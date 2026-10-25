//! Fixed-step worker independent of repaint frequency. The latest controls win;
//! state stays on the worker within a generation. Only one result snapshot is
//! retained when the window is slow or hidden (no growing animation queue).
use eframe::{egui, wgpu};
use marvelous_cloth::{
    fitting::FittingScene,
    gpu_vbd::GpuVbd,
    model::State,
    vbd::{FinalDiagnostics, FinalDiagnosticsBackend, Grab, VbdMetrics, VbdSettings, VbdSolver},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub(super) struct Work {
    pub generation: u64,
    pub scene: FittingScene,
    pub settings: VbdSettings,
    pub grab: Option<Grab>,
    pub gpu: bool,
    pub ctx: egui::Context,
    pub playing: bool,
    pub once: bool,
}
pub(super) struct Done {
    pub generation: u64,
    pub had_grab: bool,
    pub result: Result<(State, VbdMetrics), String>,
}
pub(super) type Latest = Arc<Mutex<Option<Done>>>;

/// Identity of one sample request. The generation names the document/solver
/// generation; the request id distinguishes repeated requests in the same
/// generation. Timestamps alone cannot identify either one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FinalDiagnosticsRequest {
    pub generation: u64,
    pub request_id: u64,
}

#[derive(Clone)]
pub(super) struct FinalDiagnosticsResult {
    pub request: FinalDiagnosticsRequest,
    pub sample: FinalDiagnostics,
}

#[derive(Clone)]
pub(super) struct FinalDiagnosticsFailure {
    pub request: FinalDiagnosticsRequest,
    pub message: String,
}

/// One-shot request from the Simulation inspector. The worker owns the
/// accepted state and solver cache, so the UI only asks for the next accepted
/// step to be sampled instead of constructing a second solver or reading the
/// worker's private state. A newer queued request supersedes one the worker
/// has not taken yet; an already running sample may finish and is filtered by
/// its identity in the UI.
static FINAL_DIAGNOSTICS_REQUEST: Mutex<Option<FinalDiagnosticsRequest>> = Mutex::new(None);
/// Common-path guard: the worker pays one atomic swap per accepted step and
/// only takes the request mutex when a request is pending.
static FINAL_DIAGNOSTICS_REQUESTED: AtomicBool = AtomicBool::new(false);
static NEXT_FINAL_DIAGNOSTICS_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

/// One-shot result slot. It is separate from `Latest` so a slow frame cannot
/// overwrite a completed sample with a later ordinary step before the UI reads
/// it. The application has one physical worker.
static FINAL_DIAGNOSTICS: Mutex<Option<FinalDiagnosticsResult>> = Mutex::new(None);
static FINAL_DIAGNOSTICS_ERROR: Mutex<Option<FinalDiagnosticsFailure>> = Mutex::new(None);

pub(super) fn request_final_diagnostics(generation: u64) -> FinalDiagnosticsRequest {
    let request = FinalDiagnosticsRequest {
        generation,
        request_id: NEXT_FINAL_DIAGNOSTICS_REQUEST_ID.fetch_add(1, Ordering::Relaxed),
    };
    if let Ok(mut slot) = FINAL_DIAGNOSTICS_REQUEST.lock() {
        *slot = Some(request);
    }
    FINAL_DIAGNOSTICS_REQUESTED.store(true, Ordering::Release);
    request
}

pub(super) fn final_diagnostics() -> Option<FinalDiagnosticsResult> {
    FINAL_DIAGNOSTICS
        .lock()
        .ok()
        .and_then(|sample| sample.clone())
}

pub(super) fn final_diagnostics_error() -> Option<FinalDiagnosticsFailure> {
    FINAL_DIAGNOSTICS_ERROR
        .lock()
        .ok()
        .and_then(|error| error.clone())
}

fn take_final_diagnostics_request(generation: u64) -> Option<FinalDiagnosticsRequest> {
    if !FINAL_DIAGNOSTICS_REQUESTED.swap(false, Ordering::AcqRel) {
        return None;
    }
    let mut slot = FINAL_DIAGNOSTICS_REQUEST.lock().ok()?;
    match *slot {
        Some(request) if request.generation == generation => slot.take(),
        // A request for a newer generation may have arrived just before the
        // worker accepted the matching work. Keep it queued for that step.
        Some(request) if request.generation > generation => {
            FINAL_DIAGNOSTICS_REQUESTED.store(true, Ordering::Release);
            None
        }
        // Requests from an older generation are stale after reload/reset.
        Some(_) => {
            *slot = None;
            None
        }
        None => None,
    }
}

fn clear_final_diagnostics() {
    if let Ok(mut sample) = FINAL_DIAGNOSTICS.lock() {
        *sample = None;
    }
    if let Ok(mut error) = FINAL_DIAGNOSTICS_ERROR.lock() {
        *error = None;
    }
}

fn accept(active: &mut Option<Work>, mut new: Work, deadline: &mut Instant) {
    if let Some(old) = active {
        if old.generation == new.generation {
            // Never roll back to the slightly older snapshot displayed by egui.
            std::mem::swap(&mut old.scene.state, &mut new.scene.state);
            new.once |= old.once;
            if new.playing && !old.playing {
                *deadline = Instant::now();
            }
        } else {
            clear_final_diagnostics();
            *deadline = Instant::now();
        }
    }
    *active = Some(new);
}
pub(super) fn start(
    device: wgpu::Device,
    queue: wgpu::Queue,
) -> Result<(mpsc::Sender<Work>, Latest), std::io::Error> {
    let (tx, rx) = mpsc::channel::<Work>();
    let latest: Latest = Arc::new(Mutex::new(None));
    let output = latest.clone();
    std::thread::Builder::new()
        .name("cloth-vbd".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            let gpu = GpuVbd::new(device, queue);
            let mut cache: Option<(u64, VbdSolver)> = None;
            let mut active: Option<Work> = None;
            let mut deadline = Instant::now();
            loop {
                let wait = active.as_ref().and_then(|w| {
                    if w.once {
                        Some(Duration::ZERO)
                    } else if w.playing {
                        Some(deadline.saturating_duration_since(Instant::now()))
                    } else {
                        None
                    }
                });
                let message = match wait {
                    Some(timeout) => match rx.recv_timeout(timeout) {
                        Ok(w) => Some(w),
                        Err(mpsc::RecvTimeoutError::Timeout) => None,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    },
                    None => match rx.recv() {
                        Ok(w) => Some(w),
                        Err(_) => break,
                    },
                };
                if let Some(w) = message {
                    accept(&mut active, w, &mut deadline);
                }
                while let Ok(w) = rx.try_recv() {
                    accept(&mut active, w, &mut deadline);
                }
                let Some(w) = &mut active else { continue };
                if !w.once && (!w.playing || Instant::now() < deadline) {
                    continue;
                }
                // A request from another generation is stale even if the
                // physical timestamp happens to match after a reload or reset.
                let request = take_final_diagnostics_request(w.generation);
                if request.is_some() {
                    // A new request supersedes the previous result; never let
                    // an old sample look like the answer to this request.
                    clear_final_diagnostics();
                }
                let previous = request.map(|_| w.scene.state.clone());
                w.once = false;
                let result: Result<(State, VbdMetrics), String> = (|| {
                    if cache.as_ref().is_none_or(|(g, _)| *g != w.generation) {
                        cache = Some((
                            w.generation,
                            VbdSolver::new(
                                w.scene.garment.clone(),
                                Some(&w.scene.collider.surface),
                                &w.scene.state,
                            )?,
                        ));
                        // A sample belongs to the solver generation that made it.
                        clear_final_diagnostics();
                    }
                    let solver = &mut cache.as_mut().unwrap().1;
                    let metrics = if w.gpu {
                        gpu.step(solver, &mut w.scene.state, &w.settings, w.grab.as_ref())
                    } else {
                        solver.step(&mut w.scene.state, &w.settings, w.grab.as_ref())
                    }?;
                    if let (Some(request), Some(previous)) = (request, previous) {
                        let backend = if w.gpu {
                            FinalDiagnosticsBackend::CpuAuditOfGpuState
                        } else {
                            FinalDiagnosticsBackend::Cpu
                        };
                        match solver.sample_final_diagnostics(
                            &previous,
                            &w.scene.state,
                            &w.settings,
                            w.grab.as_ref(),
                            backend,
                        ) {
                            Ok(sample) => {
                                if let Ok(mut slot) = FINAL_DIAGNOSTICS.lock() {
                                    *slot = Some(FinalDiagnosticsResult { request, sample });
                                }
                                if let Ok(mut error) = FINAL_DIAGNOSTICS_ERROR.lock() {
                                    *error = None;
                                }
                            }
                            Err(message) => {
                                if let Ok(mut slot) = FINAL_DIAGNOSTICS.lock() {
                                    *slot = None;
                                }
                                if let Ok(mut error) = FINAL_DIAGNOSTICS_ERROR.lock() {
                                    *error = Some(FinalDiagnosticsFailure { request, message });
                                }
                            }
                        }
                    }
                    Ok((w.scene.state.clone(), metrics))
                })();
                if let Err(error) = &result {
                    if let Some(request) = request {
                        if let Ok(mut sample) = FINAL_DIAGNOSTICS.lock() {
                            *sample = None;
                        }
                        if let Ok(mut slot) = FINAL_DIAGNOSTICS_ERROR.lock() {
                            *slot = Some(FinalDiagnosticsFailure {
                                request,
                                message: error.clone(),
                            });
                        }
                    }
                    w.playing = false;
                }
                let mut slot = output.lock().unwrap();
                let had_grab = w.grab.is_some()
                    || slot
                        .as_ref()
                        .is_some_and(|d| d.generation == w.generation && d.had_grab);
                *slot = Some(Done {
                    generation: w.generation,
                    had_grab,
                    result,
                });
                drop(slot);
                w.ctx.request_repaint();
                // Keep the wall-clock phase through isolated BVH rebuilds and OS
                // wakeup jitter. Bound catch-up to four steps; never change the
                // physical dt or iteration budget when the machine is overloaded.
                let dt = Duration::from_secs_f64(w.settings.dt);
                deadline = (deadline + dt).max(Instant::now() - dt * 4);
            }
        })?;
    Ok((tx, latest))
}

#[cfg(test)]
mod final_diagnostics_transport_tests {
    use super::*;
    use std::sync::Mutex;

    // These tests exercise the one physical worker's global mailbox.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn reset() {
        if let Ok(mut request) = FINAL_DIAGNOSTICS_REQUEST.lock() {
            *request = None;
        }
        FINAL_DIAGNOSTICS_REQUESTED.store(false, Ordering::Release);
        clear_final_diagnostics();
    }

    #[test]
    fn newer_request_has_a_new_identity_and_supersedes_a_queued_request() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        reset();

        let first = request_final_diagnostics(11);
        let second = request_final_diagnostics(11);
        let reloaded = request_final_diagnostics(12);

        assert_eq!(first.generation, 11);
        assert_eq!(second.generation, 11);
        assert_eq!(reloaded.generation, 12);
        assert_ne!(first.request_id, second.request_id);
        assert_ne!(second.request_id, reloaded.request_id);
        assert_eq!(take_final_diagnostics_request(11), None);
        assert_eq!(take_final_diagnostics_request(12), Some(reloaded));

        let stale = request_final_diagnostics(10);
        assert_eq!(stale.generation, 10);
        assert_eq!(take_final_diagnostics_request(11), None);
        assert_eq!(take_final_diagnostics_request(11), None);

        reset();
    }
}

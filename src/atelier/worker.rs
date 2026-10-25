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
        atomic::{AtomicBool, Ordering},
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

/// One-shot request from the Simulation inspector. The worker owns the
/// accepted state and solver cache, so the UI only asks for the next accepted
/// step to be sampled instead of constructing a second solver or reading the
/// worker's private state.
static FINAL_DIAGNOSTICS_REQUEST: AtomicBool = AtomicBool::new(false);

/// One-shot result slot. It is separate from `Latest` so a slow frame cannot
/// overwrite a completed sample with a later ordinary step before the UI reads
/// it. The application has one physical worker.
static FINAL_DIAGNOSTICS: Mutex<Option<FinalDiagnostics>> = Mutex::new(None);
static FINAL_DIAGNOSTICS_ERROR: Mutex<Option<String>> = Mutex::new(None);

pub(super) fn request_final_diagnostics() {
    FINAL_DIAGNOSTICS_REQUEST.store(true, Ordering::Release);
}

pub(super) fn final_diagnostics() -> Option<FinalDiagnostics> {
    FINAL_DIAGNOSTICS
        .lock()
        .ok()
        .and_then(|sample| sample.clone())
}

pub(super) fn final_diagnostics_error() -> Option<String> {
    FINAL_DIAGNOSTICS_ERROR
        .lock()
        .ok()
        .and_then(|error| error.clone())
}

fn take_final_diagnostics_request() -> bool {
    FINAL_DIAGNOSTICS_REQUEST.swap(false, Ordering::AcqRel)
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
                let sample_final = take_final_diagnostics_request();
                if sample_final {
                    // A new request supersedes the previous result; never let
                    // an old sample look like the answer to this request.
                    clear_final_diagnostics();
                }
                let previous = sample_final.then(|| w.scene.state.clone());
                w.once = false;
                let result = (|| {
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
                    if let Some(previous) = previous {
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
                                    *slot = Some(sample);
                                }
                                if let Ok(mut error) = FINAL_DIAGNOSTICS_ERROR.lock() {
                                    *error = None;
                                }
                            }
                            Err(error) => {
                                if let Ok(mut slot) = FINAL_DIAGNOSTICS_ERROR.lock() {
                                    *slot = Some(error);
                                }
                            }
                        }
                    }
                    Ok((w.scene.state.clone(), metrics))
                })();
                if result.is_err() {
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

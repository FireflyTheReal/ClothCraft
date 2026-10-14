//! Fixed-step worker independent of repaint frequency. The latest controls win;
//! state stays on the worker within a generation. Only one result snapshot is
//! retained when the window is slow or hidden (no growing animation queue).
use eframe::{egui, wgpu};
use marvelous_cloth::{
    fitting::FittingScene,
    gpu_vbd::GpuVbd,
    model::State,
    vbd::{Grab, VbdMetrics, VbdSettings, VbdSolver},
};
use std::{
    sync::{Arc, Mutex, mpsc},
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
                    }
                    let solver = &mut cache.as_mut().unwrap().1;
                    let metrics = if w.gpu {
                        gpu.step(solver, &mut w.scene.state, &w.settings, w.grab.as_ref())
                    } else {
                        solver.step(&mut w.scene.state, &w.settings, w.grab.as_ref())
                    }?;
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

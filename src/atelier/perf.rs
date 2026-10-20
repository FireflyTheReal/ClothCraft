//! Opt-in measurement of the actual window + worker, not an offscreen FPS claim.
use std::time::Instant;
/// Delivered physical time, including snapshots skipped by a slower renderer.
/// Never infer throughput from GPU timestamps or the repaint counter.
#[derive(Default)]
pub(super) struct Cadence {
    sample: Option<(Instant, f64, u64)>,
    pub speed: Option<f64>,
}
impl Cadence {
    pub fn update(&mut self, now: Instant, time: f64, generation: u64, playing: bool) {
        if !playing {
            self.sample = None;
            self.speed = None;
            return;
        }
        let Some((start, initial, previous_generation)) = self.sample else {
            self.sample = Some((now, time, generation));
            return;
        };
        if previous_generation != generation || time < initial {
            self.sample = Some((now, time, generation));
            self.speed = None;
            return;
        }
        let elapsed = now.duration_since(start).as_secs_f64();
        if elapsed >= 0.5 {
            self.speed = Some((time - initial) / elapsed);
            self.sample = Some((now, time, generation));
        }
    }
}

pub(super) struct PerfRun {
    boot: Instant,
    sample: Option<(Instant, f64, u64)>,
    last_frame: Option<u64>,
    previous: Instant,
    intervals: Vec<f64>,
    pub finished: bool,
}
impl PerfRun {
    pub fn new() -> Self {
        Self {
            boot: Instant::now(),
            sample: None,
            last_frame: None,
            previous: Instant::now(),
            intervals: Vec::new(),
            finished: false,
        }
    }
    pub fn expired(&self) -> bool {
        self.boot.elapsed().as_secs_f64() > 20.
    }
    pub fn tick(
        &mut self,
        time: f64,
        frame: u64,
        dt: f64,
        ready: bool,
    ) -> Option<serde_json::Value> {
        if self.finished || !ready || self.last_frame == Some(frame) {
            return None;
        }
        self.last_frame = Some(frame);
        let now = Instant::now();
        let Some((start, simulated_start, first_frame)) = self.sample else {
            if self.boot.elapsed().as_secs_f64() >= 1. {
                self.sample = Some((now, time, frame));
                self.previous = now;
            }
            return None;
        };
        self.intervals
            .push(now.duration_since(self.previous).as_secs_f64() * 1000.);
        self.previous = now;
        let elapsed = now.duration_since(start).as_secs_f64();
        if elapsed < 5. {
            return None;
        }
        self.finished = true;
        self.intervals.sort_by(f64::total_cmp);
        let fps = (frame - first_frame) as f64 / elapsed;
        let speed = (time - simulated_start) / elapsed;
        Some(serde_json::json!({"passed":speed>=0.95 && fps>=55.,
            "wall_seconds":elapsed,"simulated_seconds":time-simulated_start,
            "simulation_speed":speed,"physics_steps_per_second":speed/dt,
            "ui_frames_per_second":fps,"ui_frame_interval_p95_ms":self.intervals[(self.intervals.len()*95/100).min(self.intervals.len()-1)],
            "scope":"Actual egui frame cadence and delivered simulated time; one second warmup, five seconds measured. Acceptance: >=0.95x simulation and >=55 UI frames/s."}))
    }
}

#[cfg(test)]
mod cadence_tests {
    use super::*;
    #[test]
    fn physical_rate_counts_time_not_snapshot_or_frame_count() {
        let mut c = Cadence::default();
        let now = Instant::now();
        c.update(now, 10., 0, true);
        c.update(now + std::time::Duration::from_secs(1), 10.5, 0, true);
        assert_eq!(c.speed, Some(0.5)); // 30 physical 1/60 s steps, one snapshot.
        c.update(now + std::time::Duration::from_secs(2), 0., 1, true);
        assert_eq!(c.speed, None);
        c.update(now + std::time::Duration::from_secs(3), 0.1, 1, false);
        assert_eq!(c.speed, None);
    }
}

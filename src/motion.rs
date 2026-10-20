//! Accepted-step motion diagnostics, independent of rendering and the solver.
//! Oscillation is evidence to inspect, not proof of numerical error. This module
//! never damps, freezes, projects or otherwise modifies physical state.
use crate::{garment::CompiledGarment, model::State};
use glam::{DVec2, DVec3};
use serde::Serialize;

#[derive(Clone, Debug, Default, Serialize)]
pub struct MotionMetrics {
    pub observed_seconds: f64,
    /// EMA of |(v_new-v_old)*dt/2|², square rooted, averaged over vertices.
    pub highpass_rms_m: f64,
    pub highpass_max_m: f64,
    pub speed_rms_m_s: f64,
    pub oscillating_vertices: usize,
    /// Bounded diagnostic sample; semantic coordinates survive file export.
    pub hotspots: Vec<MotionHotspot>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MotionHotspot {
    pub panel: String,
    pub material_point_m: DVec2,
    pub highpass_rms_m: f64,
    pub reversal_fraction: f64,
}

#[derive(Clone, Default)]
struct VertexMotion {
    mean_velocity: DVec3,
    speed2: f64,
    highpass2: f64,
    reversals: f64,
    persistent_s: f64,
}

#[derive(Clone, Default)]
pub struct MotionMonitor {
    vertices: Vec<VertexMotion>,
    last_time: Option<f64>,
    dt: f64,
    elapsed: f64,
}

impl MotionMonitor {
    /// Call exactly once per accepted physical step. Reconstruct/reset this
    /// cache on external state edits, remeshing or a solver generation change.
    pub fn record(
        &mut self,
        old_velocity: &[DVec3],
        state: &State,
        dt: f64,
        garment: &CompiledGarment,
    ) -> MotionMetrics {
        if self.vertices.len() != state.v.len()
            || self.dt != dt
            || self
                .last_time
                .is_none_or(|t| (state.time - t - dt).abs() > dt * 1e-6)
        {
            self.vertices.clear();
            self.vertices.resize_with(state.v.len(), Default::default);
            self.elapsed = 0.;
        }
        self.dt = dt;
        self.last_time = Some(state.time);
        self.elapsed += dt;
        // Half a second of history; independent of rendering frequency.
        let weight = 1. - (-dt / 0.5).exp();
        let mut result = MotionMetrics {
            observed_seconds: self.elapsed,
            ..Default::default()
        };
        let mut worst: Vec<(usize, f64)> = Vec::with_capacity(12);
        let mut highpass2 = 0.;
        let mut speed2 = 0.;
        let mut maximum2 = 0.0_f64;
        for (i, ((m, &v), &old)) in self
            .vertices
            .iter_mut()
            .zip(&state.v)
            .zip(old_velocity)
            .enumerate()
        {
            let hp2 = ((v - old) * (0.5 * dt)).length_squared();
            let reversal =
                v.dot(old) < 0. && v.length_squared().min(old.length_squared()) * dt * dt > 1e-12;
            m.mean_velocity += (v - m.mean_velocity) * weight;
            m.speed2 += (v.length_squared() - m.speed2) * weight;
            m.highpass2 += (hp2 - m.highpass2) * weight;
            m.reversals += (f64::from(reversal) - m.reversals) * weight;
            // Repeated reversals with little net drift. Ordinary translation,
            // monotone acceleration and sub-20 µm numerical noise are not flags.
            let suspicious = self.elapsed >= 0.5
                && m.highpass2 >= (20e-6_f64).powi(2)
                && m.reversals >= 0.04
                && m.mean_velocity.length_squared() < 0.25 * m.speed2;
            m.persistent_s = if suspicious { m.persistent_s + dt } else { 0. };
            if m.persistent_s >= 0.25 && garment.mesh.pins[i].is_none() {
                result.oscillating_vertices += 1;
                let at = worst.partition_point(|(_, value)| *value > m.highpass2);
                if at < 12 {
                    worst.insert(at, (i, m.highpass2));
                    worst.truncate(12);
                }
            }
            highpass2 += m.highpass2;
            speed2 += m.speed2;
            maximum2 = maximum2.max(m.highpass2);
        }
        let n = state.v.len().max(1) as f64;
        result.highpass_rms_m = (highpass2 / n).sqrt();
        result.speed_rms_m_s = (speed2 / n).sqrt();
        result.highpass_max_m = maximum2.sqrt();
        result.hotspots = worst
            .into_iter()
            .filter_map(|(v, amplitude)| {
                let panel = garment.panels.iter().find(|p| p.vertices.contains(&v))?;
                Some(MotionHotspot {
                    panel: panel.id.clone(),
                    material_point_m: garment.mesh.uv[v],
                    highpass_rms_m: amplitude.sqrt(),
                    reversal_fraction: self.vertices[v].reversals,
                })
            })
            .collect();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::garment::GarmentTemplate;

    #[test]
    fn detects_persistent_oscillation_not_drift_or_acceleration_and_resets() {
        let g = GarmentTemplate::two_panel_demo().compile().unwrap();
        let dt = 1. / 60.;
        for mode in 0..3 {
            let mut monitor = MotionMonitor::default();
            let mut state = g.initial_state.clone();
            let mut metrics = MotionMetrics::default();
            for step in 0..180 {
                let old = state.v.clone();
                let v = match mode {
                    0 => DVec3::new(0.1, -0.2, 0.3),
                    1 => -DVec3::Y * step as f64 * dt,
                    _ => DVec3::X * if step % 2 == 0 { 0.03 } else { -0.03 },
                };
                state.v.fill(v);
                state.time += dt;
                metrics = monitor.record(&old, &state, dt, &g);
            }
            assert_eq!(metrics.oscillating_vertices > 0, mode == 2);
            if mode == 2 {
                assert!((metrics.highpass_rms_m - 0.0005).abs() < 2e-6);
                assert!(!metrics.hotspots.is_empty());
                assert!(metrics.hotspots.len() <= 12);
                let old = state.v.clone();
                state.time += dt * 2.;
                metrics = monitor.record(&old, &state, dt * 2., &g);
                assert_eq!(metrics.oscillating_vertices, 0);
                assert_eq!(metrics.observed_seconds, dt * 2.);
            }
        }
    }

    #[test]
    fn quiet_motion_clears_hotspots_and_subthreshold_noise_is_ignored() {
        let g = GarmentTemplate::two_panel_demo().compile().unwrap();
        let dt = 1. / 60.;
        for amplitude in [1e-6, 5e-4] {
            let mut monitor = MotionMonitor::default();
            let mut s = g.initial_state.clone();
            for step in 0..360 {
                let old = s.v.clone();
                s.v.fill(if step < 180 {
                    DVec3::Z * (amplitude / dt) * if step % 2 == 0 { 1. } else { -1. }
                } else {
                    DVec3::ZERO
                });
                s.time += dt;
                let m = monitor.record(&old, &s, dt, &g);
                if step == 179 {
                    assert_eq!(m.oscillating_vertices > 0, amplitude > 20e-6);
                }
                if step == 359 {
                    assert_eq!(m.oscillating_vertices, 0);
                    assert!(m.hotspots.is_empty());
                }
            }
        }
    }
}

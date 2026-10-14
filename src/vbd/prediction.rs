//! VBD §3.7 adaptive initialization, with backward-Euler damping accounted for.
//! Only the starting iterate changes. The inertia target retains full gravity.
use super::VbdSettings;
use crate::model::State;
use glam::DVec3;

#[derive(Clone, Default)]
pub(crate) struct Prediction {
    last: Option<State>,
    acceleration: Vec<DVec3>,
    gravity: DVec3,
    damping: f64,
}
impl Prediction {
    pub fn guess(&self, state: &State, settings: &VbdSettings) -> Vec<DVec3> {
        let valid = self.last.as_ref().is_some_and(|s| {
            s.time == state.time
                && s.x == state.x
                && s.v == state.v
                && self.gravity == settings.gravity
                && self.damping == settings.damping
        });
        let g2 = settings.gravity.length_squared();
        state
            .x
            .iter()
            .zip(&state.v)
            .enumerate()
            .map(|(i, (x, v))| {
                let factor = if valid && g2 > 1e-20 {
                    // a + damping*v equals external + elastic/contact acceleration.
                    // This gives 1 during damped free fall, 0 at supported rest.
                    (self.acceleration[i].dot(settings.gravity) / g2).clamp(0., 1.)
                } else {
                    1.
                };
                *x + (*v * settings.dt + factor * settings.gravity * settings.dt.powi(2))
                    / (1. + settings.damping * settings.dt)
            })
            .collect()
    }
    /// Called only after an accepted step; edits/reloads must not reuse history.
    pub fn record(&mut self, old_velocity: &[DVec3], state: &State, settings: &VbdSettings) {
        self.acceleration = state
            .v
            .iter()
            .zip(old_velocity)
            .map(|(v, old)| (*v - *old) / settings.dt + settings.damping * *v)
            .collect();
        self.last = Some(state.clone());
        self.gravity = settings.gravity;
        self.damping = settings.damping;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adaptive_guess_preserves_damped_fall_and_supported_rest_and_invalidates_edits() {
        let settings = VbdSettings {
            dt: 1. / 60.,
            ..Default::default()
        };
        let mut state = State {
            x: vec![DVec3::Y],
            v: vec![DVec3::ZERO],
            time: 0.,
        };
        let mut p = Prediction::default();
        for _ in 0..5 {
            let next = p.guess(&state, &settings);
            let expected = (state.v[0] * settings.dt + settings.gravity * settings.dt.powi(2))
                / (1. + settings.damping * settings.dt);
            assert!((next[0] - state.x[0] - expected).length() < 1e-12);
            let old = state.v.clone();
            state.v[0] = (next[0] - state.x[0]) / settings.dt;
            state.x = next;
            state.time += settings.dt;
            p.record(&old, &state, &settings);
        }
        state.v[0] = DVec3::ZERO;
        p.record(&[DVec3::ZERO], &state, &settings);
        assert_eq!(p.guess(&state, &settings), state.x);
        state.x[0].x += 0.01;
        assert!(p.guess(&state, &settings)[0].y < state.x[0].y);
    }
}

//! Shared f64 local minimization, including sparse precision work for the GPU.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct LocalSolveProbe {
    unshifted_spd: bool,
    backtracks: usize,
}
impl Default for LocalSolveProbe {
    fn default() -> Self {
        Self {
            unshifted_spd: false,
            backtracks: 10,
        }
    }
}

// A rotated rank-one contact Hessian can be SPD after inertia is added while
// its Gershgorin lower bound is negative. Cholesky tests the actual matrix.
// Reject poorly scaled pivots and keep the original shifted fallback.
fn solve_spd(h: DMat3, rhs: DVec3) -> Option<DVec3> {
    let a = h.to_cols_array_2d();
    let scale = a[0][0].abs().max(a[1][1].abs()).max(a[2][2].abs());
    if !h.is_finite() || !rhs.is_finite() || scale == 0. {
        return None;
    }
    let threshold = scale * 1e-12;
    let pivot = |x: f64| {
        if x.is_finite() && x > threshold {
            Some(x.sqrt())
        } else {
            None
        }
    };
    let l00 = pivot(a[0][0])?;
    let l10 = a[0][1] / l00;
    let l20 = a[0][2] / l00;
    let l11 = pivot(a[1][1] - l10 * l10)?;
    let l21 = (a[1][2] - l20 * l10) / l11;
    let l22 = pivot(a[2][2] - l20 * l20 - l21 * l21)?;
    let y0 = rhs.x / l00;
    let y1 = (rhs.y - l10 * y0) / l11;
    let y2 = (rhs.z - l20 * y0 - l21 * y1) / l22;
    let z = y2 / l22;
    let y = (y1 - l21 * z) / l11;
    let x = (y0 - l10 * y - l20 * z) / l00;
    let result = DVec3::new(x, y, z);
    result.is_finite().then_some(result)
}

impl VbdSolver {
    /// Opt-in CPU research ablations. Not a material, document or UI setting.
    /// Defaults preserve the shipped solve. GPU WGSL is unaffected; do not use
    /// this as a claimed matched GPU experiment (its f64 fallback would change).
    pub fn set_local_solve_probe(
        &mut self,
        unshifted_spd: bool,
        backtracks: usize,
    ) -> Result<(), String> {
        if !(10..=32).contains(&backtracks) {
            return Err("Probe backtracks must be in [10, 32]".into());
        }
        self.local_probe = LocalSolveProbe {
            unshifted_spd,
            backtracks,
        };
        Ok(())
    }
    pub(super) fn local_step(
        &self,
        v: usize,
        x: &[DVec3],
        contacts: &ContactSet,
        settings: &VbdSettings,
        grab: Option<&Grab>,
        (target, inertia): (DVec3, f64),
    ) -> (DVec3, bool, f64) {
        let jet = self.local(
            v,
            std::array::from_fn(|k| Jet::<3>::variable(x[v][k], k)),
            x,
            contacts,
            settings,
            grab,
        );
        let g = DVec3::from_array(jet.g) + (x[v] - target) * inertia;
        let mut h =
            DMat3::from_cols_array(&std::array::from_fn::<_, 9, _>(|i| jet.h[i % 3][i / 3]))
                + DMat3::IDENTITY * inertia;
        let unshifted = if self.local_probe.unshifted_spd {
            solve_spd(h, -g)
        } else {
            None
        };
        let diag = h.to_cols_array_2d();
        let lower = (0..3)
            .map(|i| {
                diag[i][i]
                    - (0..3)
                        .filter(|j| *j != i)
                        .map(|j| diag[i][j].abs())
                        .sum::<f64>()
            })
            .fold(f64::INFINITY, f64::min);
        // Gershgorin SPD shift changes the local search direction, not the energy.
        if lower < 1e-8 {
            h += DMat3::IDENTITY * (1e-8 - lower + 0.01 * inertia);
        }
        let delta = unshifted.unwrap_or_else(|| -(h.inverse() * g));
        let old = jet.v + 0.5 * inertia * (x[v] - target).length_squared();
        let mut truncated = false;
        for attempt in 0..self.local_probe.backtracks {
            let candidate = x[v] + delta * 0.5_f64.powi(attempt as i32);
            let (candidate, bound) = contacts.truncate_step(v, x[v], candidate);
            truncated |= bound;
            let e = self.local(v, candidate.to_array(), x, contacts, settings, grab)
                + 0.5 * inertia * (candidate - target).length_squared();
            if candidate.is_finite() && e.is_finite() && e <= old + 1e-14 {
                return (candidate, truncated, g.length_squared());
            }
        }
        (x[v], truncated, g.length_squared())
    }

    /// Dependency-ordered local minimizations of the SAME implicit objective,
    /// equivalent to sequential visitation, within the ORIGINAL detection
    /// regions (balls and, when present, their separating half-spaces).
    #[cfg(feature = "desktop")]
    pub(crate) fn refine_precision_vertices(
        &self,
        x: &mut [DVec3],
        state: &State,
        contacts: &ContactSet,
        settings: &VbdSettings,
        grab: Option<&Grab>,
        vertices: &[usize],
    ) {
        let dt = settings.dt;
        let damp = 1. + settings.damping * dt;
        let groups = super::coloring::ordered_groups(&self.graph, contacts, grab, vertices);
        // Two sparse sweeps; detection is refreshed normally at the next step.
        for _ in 0..2 {
            for group in &groups {
                let solve = |&v: &usize| {
                    let target = state.x[v] + (state.v[v] * dt + settings.gravity * dt * dt) / damp;
                    let inertia = self.garment.mesh.masses[v] * damp / (dt * dt);
                    (
                        v,
                        self.local_step(v, x, contacts, settings, grab, (target, inertia))
                            .0,
                    )
                };
                if group.len() == 1 {
                    let (v, p) = solve(&group[0]);
                    x[v] = p;
                } else {
                    let updates: Vec<_> = group.par_iter().map(solve).collect();
                    for (v, p) in updates {
                        x[v] = p;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spd_probe_preserves_tangential_response_of_rotated_contact() {
        let normal = DVec3::new(0.1, 0.9, 0.3).normalize();
        let tangent = normal.cross(DVec3::Z).normalize();
        let h = DMat3::IDENTITY * 0.3
            + DMat3::from_cols(normal * normal.x, normal * normal.y, normal * normal.z) * 1e5;
        let force = tangent * 0.03;
        let answer = solve_spd(h, force).unwrap();
        assert!((h * answer - force).length() < 1e-10);
        assert!((answer - tangent * 0.1).length() < 1e-10);
        let rotation = DMat3::from_rotation_y(0.7);
        let rotated = solve_spd(rotation * h * rotation.transpose(), rotation * force).unwrap();
        assert!((rotated - rotation * answer).length() < 1e-10);
        assert!(solve_spd(DMat3::from_diagonal(DVec3::new(1., -1., 1.)), force).is_none());
        assert!(solve_spd(DMat3::ZERO, force).is_none());
        assert!(solve_spd(DMat3::from_diagonal(DVec3::new(1., 1e-16, 1.)), force).is_none());
    }

    #[test]
    fn longer_search_resolves_contact_onset_without_relaxing_bounds() {
        let mut t = crate::garment::GarmentTemplate::two_panel_demo();
        t.panels.truncate(1);
        t.seams.clear();
        let p = &mut t.panels[0];
        p.pins.clear();
        p.pattern = crate::model::Pattern {
            width: 0.15,
            height: 0.15,
            nx: 3,
            ny: 3,
        };
        p.placement.rotation_degrees = DVec3::new(90., 0., 0.);
        p.placement.translation = DVec3::new(-0.075, 0.0030003, 0.075);
        let g = Arc::new(t.compile().unwrap());
        let state = g.initial_state.clone();
        let floor = SurfaceMesh {
            vertices: vec![
                DVec3::new(-2., 0., -2.),
                DVec3::new(2., 0., -2.),
                DVec3::new(0., 0., 2.),
            ],
            triangles: vec![[0, 1, 2]],
        };
        let mut solver = VbdSolver::new(g.clone(), Some(&floor), &state).unwrap();
        let settings = VbdSettings {
            dt: 1. / 60.,
            ..Default::default()
        };
        let x = solver.world.positions(&state.x);
        let contacts = solver
            .world
            .detect(&x, 4. * settings.contact.radius, &settings.contact)
            .unwrap();
        let v = 15;
        let damp = 1. + settings.damping * settings.dt;
        let target = x[v] + settings.gravity * settings.dt.powi(2) / damp;
        let inertia = g.mesh.masses[v] * damp / settings.dt.powi(2);
        let old = solver
            .local_step(v, &x, &contacts, &settings, None, (target, inertia))
            .0;
        assert_eq!(
            old, x[v],
            "Ten trials should reproduce the exhausted contact-onset search"
        );
        solver.set_local_solve_probe(false, 32).unwrap();
        let next = solver
            .local_step(v, &x, &contacts, &settings, None, (target, inertia))
            .0;
        assert!(next.y < x[v].y - 1e-7);
        assert!(next.y > 0.);
        assert!(contacts.contains(v, next, 1e-12));
        let objective = |p: DVec3| {
            solver.local(v, p.to_array(), &x, &contacts, &settings, None)
                + 0.5 * inertia * (p - target).length_squared()
        };
        assert!(objective(next) < objective(old));
        assert_eq!(contacts.centers[v], x[v]);
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn precision_levels_match_sequential_minimization_exactly() {
        let mut template = crate::garment::GarmentTemplate::two_panel_demo();
        template.panels.truncate(1);
        template.panels[0].pins.clear();
        template.seams.clear();
        let garment = Arc::new(template.compile().unwrap());
        let state = garment.initial_state.clone();
        let n = state.x.len();
        let mut solver = VbdSolver::new(garment, None, &state).unwrap();
        let settings = VbdSettings::default();
        let mut sequential = solver.world.positions(&state.x);
        let contacts = solver
            .world
            .detect(&sequential, settings.contact.radius * 2., &settings.contact)
            .unwrap();
        let vertices = [2, 0, 1, n - 1, n - 3, n - 2];
        let grab = Grab {
            patch: Vec::new(),
            vertices: [0, n - 1, 2],
            weights: [0.3, 0.4, 0.3],
            target: state.x[0] + DVec3::new(0.001, 0.002, 0.003),
            stiffness: 100.,
        };
        for g in [None, Some(&grab)] {
            sequential.clone_from(&solver.world.positions(&state.x));
            let mut parallel = sequential.clone();
            let dt = settings.dt;
            let damp = 1. + settings.damping * dt;
            for _ in 0..2 {
                for &v in &vertices {
                    let target = state.x[v] + (state.v[v] * dt + settings.gravity * dt * dt) / damp;
                    let inertia = solver.garment.mesh.masses[v] * damp / (dt * dt);
                    sequential[v] = solver
                        .local_step(v, &sequential, &contacts, &settings, g, (target, inertia))
                        .0;
                }
            }
            solver.refine_precision_vertices(
                &mut parallel,
                &state,
                &contacts,
                &settings,
                g,
                &vertices,
            );
            assert_eq!(parallel, sequential);
        }
    }

    #[test]
    fn local_minimization_decreases_the_objective_inside_the_original_ball() {
        let mut template = crate::garment::GarmentTemplate::two_panel_demo();
        template.panels.truncate(1);
        template.panels[0].pins.clear();
        template.seams.clear();
        let garment = Arc::new(template.compile().unwrap());
        let state = garment.initial_state.clone();
        let mut solver = VbdSolver::new(garment, None, &state).unwrap();
        let settings = VbdSettings::default();
        let mut x = solver.world.positions(&state.x);
        let mut contacts = solver
            .world
            .detect(&x, settings.contact.radius * 2., &settings.contact)
            .unwrap();
        // A tiny ball exercises the same constrained minimization as the sparse
        // precision path, without needing a GPU or changing contact exclusions.
        let v = 5;
        contacts.radii[v] = 1.919005e-6;
        let center = contacts.centers[v];
        let target = x[v] + DVec3::new(0.1, -0.2, 0.3);
        let inertia = 1000.;
        let objective = |p: DVec3, x: &[DVec3]| {
            solver.local(v, p.to_array(), x, &contacts, &settings, None)
                + 0.5 * inertia * (p - target).length_squared()
        };
        let initial = objective(x[v], &x);
        for _ in 0..4 {
            let before = objective(x[v], &x);
            let (p, _, _) = solver.local_step(v, &x, &contacts, &settings, None, (target, inertia));
            assert!(p.distance(center) <= contacts.radii[v] + 1e-14);
            assert!(objective(p, &x) <= before + 1e-13);
            x[v] = p;
        }
        assert!(x[v].distance(center) > 1e-6);
        assert!(objective(x[v], &x) < initial - 1e-6);
        assert_eq!(contacts.centers[v], center);
    }
}

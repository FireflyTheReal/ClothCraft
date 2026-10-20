use glam::DVec3;
use marvelous_cloth::{
    garment::GarmentTemplate,
    solver_lab::{Algorithm, Backend},
    vbd::{Grab, VbdSettings},
};
use std::sync::Arc;
mod common;

#[test]
fn lab_backends_preserve_rest_and_analytic_damped_free_fall() {
    for algorithm in [
        Algorithm::Xpbd,
        Algorithm::AvbdFinite,
        Algorithm::XpbdPrimal,
        Algorithm::AvbdTarget,
    ] {
        let mut t = GarmentTemplate::two_panel_demo();
        t.panels.truncate(1);
        t.seams.clear();
        t.panels[0].pins.clear();
        t.panels[0].pattern.nx = 2;
        t.panels[0].pattern.ny = 2;
        let g = Arc::new(t.compile().unwrap());
        let mut state = g.initial_state.clone();
        let mut solver = Backend::new(algorithm, g.clone(), None, &state).unwrap();
        let mut settings = VbdSettings {
            dt: 0.001,
            iterations: 4,
            gravity: DVec3::ZERO,
            ..Default::default()
        };
        solver.step(&mut state, &settings, None).unwrap();
        for (x, r) in state.x.iter().zip(&g.initial_state.x) {
            assert!(x.distance(*r) < 1e-12);
        }
        settings.gravity = DVec3::new(0., -9.81, 0.);
        let mut expected = DVec3::ZERO;
        for _ in 0..20 {
            expected =
                (expected + settings.gravity * settings.dt) / (1. + settings.damping * settings.dt);
            solver.step(&mut state, &settings, None).unwrap();
            for v in &state.v {
                assert!(
                    v.distance(expected) < 1e-8,
                    "{algorithm:?}: {v:?} != {expected:?}"
                );
            }
        }
    }
}

#[test]
fn lab_pull_release_keeps_floor_separation_and_failed_step_is_transactional() {
    for algorithm in [
        Algorithm::Xpbd,
        Algorithm::AvbdFinite,
        Algorithm::XpbdPrimal,
        Algorithm::AvbdTarget,
    ] {
        let (g, floor) = common::resting_patch(DVec3::ZERO);
        let mut state = g.initial_state.clone();
        let mut solver = Backend::new(algorithm, g.clone(), Some(&floor), &state).unwrap();
        let settings = VbdSettings {
            dt: 1. / 120.,
            iterations: 8,
            ..Default::default()
        };
        let vertices = g.mesh.triangles[0].vertices;
        let anchor = vertices.iter().map(|i| state.x[*i]).sum::<DVec3>() / 3.;
        for step in 0..90 {
            let grab = (10..40).contains(&step).then_some(Grab {
                patch: Vec::new(),
                vertices,
                weights: [1. / 3.; 3],
                target: anchor + DVec3::new(0.01, 0.01, 0.),
                stiffness: 1.,
            });
            solver.step(&mut state, &settings, grab.as_ref()).unwrap();
            assert!(state.x.iter().all(|p| p.is_finite() && p.y > 0.));
        }
        let before = serde_json::to_vec(&state).unwrap();
        let invalid = VbdSettings {
            dt: f64::NAN,
            ..settings
        };
        assert!(solver.step(&mut state, &invalid, None).is_err());
        assert_eq!(before, serde_json::to_vec(&state).unwrap());
        solver
            .reference
            .world
            .validate_start(&solver.reference.world.positions(&state.x))
            .unwrap();
    }
}

#[test]
fn full_target_convergence_and_independent_force_audit() {
    let (g, floor) = common::resting_patch(DVec3::ZERO);
    for algorithm in [Algorithm::XpbdPrimal, Algorithm::AvbdTarget] {
        let mut residuals = Vec::new();
        for iterations in [4, 64] {
            let mut state = g.initial_state.clone();
            // The inertial target crosses the floor: an unclipped prediction is
            // invalid, yet the target must remain in the equations of motion.
            state.v.fill(DVec3::new(0.03, -0.5, 0.));
            let before = state.clone();
            let mut solver = Backend::new(algorithm, g.clone(), Some(&floor), &state).unwrap();
            solver.trace_convergence(true);
            let settings = VbdSettings {
                dt: 1. / 60.,
                iterations,
                ..Default::default()
            };
            let m = solver.step(&mut state, &settings, None).unwrap();
            let x = solver.reference.world.positions(&state.x);
            let c = solver
                .reference
                .world
                .detect(&x, settings.contact.radius * 2., &settings.contact)
                .unwrap();
            let independent = solver
                .reference
                .audit_step(&before, &state, &settings, &c, None)
                .unwrap();
            assert!((m.residual_rms_n.unwrap() - independent.residual_rms_n).abs() < 1e-8);
            solver.reference.world.validate_start(&x).unwrap();
            let trace = solver.trace.last().unwrap();
            assert_eq!(trace.full_stiffness_fraction, 1.);
            residuals.push(independent.residual_rms_n);
        }
        eprintln!("{algorithm:?}: residuals {residuals:?}");
        assert!(
            residuals[1] < residuals[0] * 0.1,
            "{algorithm:?}: {residuals:?}"
        );
    }
}

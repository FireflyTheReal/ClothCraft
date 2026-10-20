use glam::DVec3;
use marvelous_cloth::{
    garment::GarmentTemplate,
    vbd::{VbdSettings, VbdSolver},
};
use std::sync::Arc;
mod common;

#[test]
fn free_fall_energy_loss_is_the_backward_euler_defect() {
    let mut t = GarmentTemplate::two_panel_demo();
    t.panels.truncate(1);
    t.seams.clear();
    t.panels[0].pins.clear();
    t.panels[0].pattern.nx = 2;
    t.panels[0].pattern.ny = 2;
    let g = Arc::new(t.compile().unwrap());
    let mut s = g.initial_state.clone();
    let mut solver = VbdSolver::new(g.clone(), None, &s).unwrap();
    let settings = VbdSettings {
        dt: 0.005,
        damping: 0.,
        iterations: 3,
        ..Default::default()
    };
    let old = s.clone();
    let contacts = solver
        .world
        .detect(&solver.world.positions(&s.x), 0.012, &settings.contact)
        .unwrap();
    solver.step(&mut s, &settings, None).unwrap();
    let snapshot = s.clone();
    let a = solver
        .audit_step(&old, &s, &settings, &contacts, None)
        .unwrap();
    let expected = 0.5
        * g.mesh.masses.iter().sum::<f64>()
        * settings.dt.powi(2)
        * settings.gravity.length_squared();
    assert!((a.delta_mechanical_j + expected).abs() < 1e-12);
    assert!((a.implicit_kinetic_loss_j - expected).abs() < 1e-12);
    assert!(a.residual_rms_n < 1e-8);
    assert!(a.balance_error_j.abs() < 1e-12);
    assert_eq!(s.x, snapshot.x);
    assert_eq!(s.v, snapshot.v);
    let vertices: Vec<_> = (0..s.x.len()).collect();
    let forces = solver
        .audit_vertex_forces(&s, &settings, &contacts, None, &vertices)
        .unwrap();
    for (v, force) in forces.iter().enumerate() {
        assert!((*force - g.mesh.masses[v] * settings.gravity).length() < 1e-8);
    }
    assert!(
        solver
            .audit_vertex_forces(&s, &settings, &contacts, None, &[s.x.len()])
            .is_err()
    );
    // Deliberately inject acceleration: the audit must expose positive work.
    let mut injected = old.clone();
    injected.time += settings.dt;
    injected.v.fill(DVec3::X);
    for p in &mut injected.x {
        *p += settings.dt * DVec3::X;
    }
    let a = solver
        .audit_step(&old, &injected, &settings, &contacts, None)
        .unwrap();
    assert!(a.delta_mechanical_j > 0.);
    assert!(a.residual_work_j > a.delta_mechanical_j);
    assert!(a.balance_error_j.abs() < 1e-12);
    injected.v[0] = DVec3::ZERO;
    assert!(
        solver
            .audit_step(&old, &injected, &settings, &contacts, None)
            .is_err()
    );
}

#[test]
fn fixed_contact_accounting_includes_stored_contact_energy_and_drag() {
    let (g, floor) = common::resting_patch(DVec3::ZERO);
    let mut s = g.initial_state.clone();
    let mut solver = VbdSolver::new(g, Some(&floor), &s).unwrap();
    let settings = VbdSettings {
        dt: 1. / 60.,
        ..Default::default()
    };
    let old = s.clone();
    let contacts = solver
        .world
        .detect(&solver.world.positions(&s.x), 0.012, &settings.contact)
        .unwrap();
    solver.step(&mut s, &settings, None).unwrap();
    let a = solver
        .audit_step(&old, &s, &settings, &contacts, None)
        .unwrap();
    assert!(a.after.contact_j > 0.);
    assert!(a.drag_loss_j >= 0.);
    assert!(a.balance_error_j.abs() < 1e-12);
    // The offline hotspot force must differentiate the complete supplied
    // potential, including contact and gravity, rather than just a local term.
    let selected = [0, 5, 15];
    let forces = solver
        .audit_vertex_forces(&s, &settings, &contacts, None, &selected)
        .unwrap();
    for (&v, force) in selected.iter().zip(forces) {
        for axis in 0..3 {
            let mut plus = s.clone();
            let mut minus = s.clone();
            plus.x[v][axis] += 1e-8;
            minus.x[v][axis] -= 1e-8;
            let a = solver
                .audit_energy(&plus, &settings, &contacts, None)
                .unwrap()
                .potential_j();
            let b = solver
                .audit_energy(&minus, &settings, &contacts, None)
                .unwrap()
                .potential_j();
            assert!((force[axis] + (a - b) / 2e-8).abs() < 1e-7);
        }
    }
}

#[test]
fn stationary_prestress_is_not_counted_as_injected_energy() {
    let mut t = GarmentTemplate::two_panel_demo();
    t.panels.truncate(1);
    t.seams.clear();
    t.panels[0].pins.clear();
    let mut g = t.compile().unwrap();
    for p in &mut g.initial_state.x {
        p.x *= 1.1;
    }
    g.mesh.pins = g.initial_state.x.iter().copied().map(Some).collect();
    let g = Arc::new(g);
    let old = g.initial_state.clone();
    let mut solver = VbdSolver::new(g, None, &old).unwrap();
    let settings = VbdSettings::default();
    let contacts = solver
        .world
        .detect(&solver.world.positions(&old.x), 0.012, &settings.contact)
        .unwrap();
    let mut next = old.clone();
    next.time += settings.dt;
    let a = solver
        .audit_step(&old, &next, &settings, &contacts, None)
        .unwrap();
    assert!(a.before.membrane_j > 0.01);
    assert_eq!(a.delta_mechanical_j, 0.);
    assert_eq!(a.drag_loss_j, 0.);
    assert_eq!(a.residual_work_j, 0.);
    assert_eq!(a.balance_error_j, 0.);
}

//! Independent CPU contracts for the opt-in final-state diagnostic sample.
//!
//! The sample is not a solver step. These tests check its accepted-state
//! contacts, free-DOF residual against a finite-difference objective, pinned
//! normalization, state preservation and the "one accepted step" contract.
use glam::DVec3;
use marvelous_cloth::{
    garment::GarmentTemplate,
    model::State,
    ogc::ContactSet,
    vbd::{FinalDiagnosticsBackend, Grab, VbdSettings, VbdSolver},
};
use std::sync::Arc;

mod common;

fn free_vertices_of(garment: &marvelous_cloth::garment::CompiledGarment) -> Vec<usize> {
    garment
        .mesh
        .pins
        .iter()
        .enumerate()
        .filter_map(|(i, pin)| pin.is_none().then_some(i))
        .collect()
}

fn free_vertices(solver: &VbdSolver) -> Vec<usize> {
    free_vertices_of(&solver.garment)
}

fn final_contacts(solver: &VbdSolver, state: &State, settings: &VbdSettings) -> ContactSet {
    let positions = solver.world.positions(&state.x);
    solver
        .world
        .clone()
        .detect(&positions, 2.0 * settings.contact.radius, &settings.contact)
        .unwrap()
}

/// Independent f64 oracle: finite differences of the implicit-Euler objective
/// (membrane, bending, sewing, contact, grab and inertia) over free vertices.
/// Gravity is already represented by the inertia target, so its potential is
/// removed from the energy accounting.
fn finite_difference_rms(
    solver: &VbdSolver,
    previous: &State,
    accepted: &State,
    settings: &VbdSettings,
    grab: Option<&Grab>,
    contacts: &ContactSet,
) -> f64 {
    let free = free_vertices(solver);
    let damp = 1.0 + settings.damping * settings.dt;
    let objective = |x: &[DVec3]| {
        let probe = State {
            x: x.to_vec(),
            v: accepted.v.clone(),
            time: accepted.time,
        };
        let energy = solver
            .audit_energy(&probe, settings, contacts, grab)
            .unwrap();
        let potential = energy.potential_j() - energy.gravity_j;
        let inertia: f64 = x
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let target = previous.x[i]
                    + (previous.v[i] * settings.dt + settings.gravity * settings.dt.powi(2)) / damp;
                0.5 * solver.garment.mesh.masses[i] * damp / settings.dt.powi(2)
                    * p.distance_squared(target)
            })
            .sum();
        potential + inertia
    };

    let h = 1e-7;
    let mut norm2 = 0.0;
    for &vertex in &free {
        for axis in 0..3 {
            let mut plus = accepted.x.clone();
            let mut minus = accepted.x.clone();
            plus[vertex][axis] += h;
            minus[vertex][axis] -= h;
            let grad = (objective(&plus) - objective(&minus)) / (2.0 * h);
            norm2 += grad * grad;
        }
    }
    (norm2 / (3 * free.len()) as f64).sqrt()
}

#[test]
fn final_sample_describes_accepted_contacts_and_preserves_state() {
    let (garment, floor) = common::resting_patch(DVec3::ZERO);
    let mut state = garment.initial_state.clone();
    let mut solver = VbdSolver::new(garment, Some(&floor), &state).unwrap();
    let settings = VbdSettings {
        dt: 1.0 / 60.0,
        iterations: 8,
        ..Default::default()
    };
    let previous = state.clone();
    solver.step(&mut state, &settings, None).unwrap();
    let accepted = state.clone();

    let sample = solver
        .sample_final_diagnostics(
            &previous,
            &state,
            &settings,
            None,
            FinalDiagnosticsBackend::Cpu,
        )
        .unwrap();

    assert_eq!(state.x, accepted.x, "the sample must not move vertices");
    assert_eq!(state.v, accepted.v, "the sample must not change velocities");
    assert_eq!(state.time, accepted.time, "the sample must not change time");
    assert!((sample.state_time_s - accepted.time).abs() < 1e-12);
    assert!((sample.step_dt_s - settings.dt).abs() < 1e-12);

    let contacts = final_contacts(&solver, &state, &settings);
    assert_eq!(sample.contacts, contacts.active_contacts);
    assert_eq!(sample.contact_terms, contacts.contacts.len());
    assert!(sample.minimum_distance_m.is_some());
    assert!((sample.minimum_distance_m.unwrap() - contacts.minimum_distance).abs() < 1e-12);
    assert_eq!(sample.free_vertices, free_vertices(&solver).len());
    assert!(sample.sample_ms >= sample.detection_ms);
    assert!(sample.detection_ms >= 0.0);
    assert!(sample.residual_ms >= 0.0);
    assert!(sample.residual_rms_n.is_some());
}

#[test]
fn free_dof_residual_matches_finite_difference_with_seam_inertia_and_grab() {
    let template = GarmentTemplate::two_panel_demo();
    let garment = Arc::new(template.compile().unwrap());
    let mut state = garment.initial_state.clone();
    let settings = VbdSettings {
        dt: 0.005,
        iterations: 8,
        damping: 0.6,
        gravity: DVec3::ZERO,
        ..Default::default()
    };
    let free = free_vertices_of(&garment);
    let anchor = free[free.len() / 2];
    let grab = Grab {
        vertices: [anchor; 3],
        weights: [1.0, 0.0, 0.0],
        target: state.x[anchor] + DVec3::new(0.0, 0.0, 0.01),
        stiffness: 120.0,
        patch: vec![],
    };
    let mut solver = VbdSolver::new(garment.clone(), None, &state).unwrap();
    let previous = state.clone();
    solver.step(&mut state, &settings, Some(&grab)).unwrap();
    let sample = solver
        .sample_final_diagnostics(
            &previous,
            &state,
            &settings,
            Some(&grab),
            FinalDiagnosticsBackend::Cpu,
        )
        .unwrap();

    assert_eq!(sample.free_vertices, free.len());
    assert_eq!(sample.minimum_distance_m, None);
    let contacts = final_contacts(&solver, &state, &settings);
    let expected = finite_difference_rms(
        &solver,
        &previous,
        &state,
        &settings,
        Some(&grab),
        &contacts,
    );
    let reported = sample.residual_rms_n.unwrap();
    assert!(
        (expected - reported).abs() < 1e-4 + 0.002 * expected,
        "finite-difference RMS {expected} differs from reported {reported}"
    );
}

#[test]
fn final_contact_residual_matches_finite_difference() {
    let (garment, floor) = common::resting_patch(DVec3::ZERO);
    let mut state = garment.initial_state.clone();
    let mut solver = VbdSolver::new(garment, Some(&floor), &state).unwrap();
    let mut settings = VbdSettings {
        dt: 1.0 / 60.0,
        iterations: 8,
        ..Default::default()
    };
    // Keep the fixture strictly inside the compact barrier. At exactly the
    // activation radius the potential is non-smooth: a tiny penetration can
    // carry a large force while a finite-difference step lies outside the
    // support, so that boundary is not a valid gradient oracle.
    settings.contact.radius = 0.02;
    let previous = state.clone();
    solver.step(&mut state, &settings, None).unwrap();
    let sample = solver
        .sample_final_diagnostics(
            &previous,
            &state,
            &settings,
            None,
            FinalDiagnosticsBackend::Cpu,
        )
        .unwrap();
    let contacts = final_contacts(&solver, &state, &settings);
    assert!(
        contacts.active_contacts > 0,
        "fixture must have active contact"
    );
    let expected = finite_difference_rms(&solver, &previous, &state, &settings, None, &contacts);
    let reported = sample.residual_rms_n.unwrap();
    assert!(reported > 1e-6, "contact residual must be non-trivial");
    assert!(
        (expected - reported).abs() < 1e-4 + 0.002 * expected,
        "contact finite-difference RMS {expected} differs from reported {reported}"
    );
}

#[test]
fn sampling_does_not_change_the_next_accepted_trajectory() {
    let (garment, floor) = common::resting_patch(DVec3::ZERO);
    let initial = garment.initial_state.clone();
    let settings = VbdSettings {
        dt: 1.0 / 60.0,
        iterations: 6,
        ..Default::default()
    };
    let mut a = VbdSolver::new(garment.clone(), Some(&floor), &initial).unwrap();
    let mut b = VbdSolver::new(garment.clone(), Some(&floor), &initial).unwrap();
    let mut state_a = initial.clone();
    let mut state_b = initial.clone();
    let previous_a = state_a.clone();
    a.step(&mut state_a, &settings, None).unwrap();
    b.step(&mut state_b, &settings, None).unwrap();
    assert_eq!(state_a.x, state_b.x);
    a.sample_final_diagnostics(
        &previous_a,
        &state_a,
        &settings,
        None,
        FinalDiagnosticsBackend::Cpu,
    )
    .unwrap();
    a.step(&mut state_a, &settings, None).unwrap();
    b.step(&mut state_b, &settings, None).unwrap();
    for (pa, pb) in state_a.x.iter().zip(&state_b.x) {
        assert!((*pa - *pb).length() < 1e-12, "sample changed contact cache");
    }
    for (va, vb) in state_a.v.iter().zip(&state_b.v) {
        assert!(
            (*va - *vb).length() < 1e-12,
            "sample changed accepted trajectory"
        );
    }
    assert!((state_a.time - state_b.time).abs() < 1e-12);
}

#[test]
fn sample_rejects_a_non_consecutive_or_inconsistent_state() {
    let (garment, floor) = common::resting_patch(DVec3::ZERO);
    let mut state = garment.initial_state.clone();
    let mut solver = VbdSolver::new(garment, Some(&floor), &state).unwrap();
    let settings = VbdSettings {
        dt: 1.0 / 60.0,
        iterations: 4,
        ..Default::default()
    };
    solver.step(&mut state, &settings, None).unwrap();
    assert!(
        solver
            .sample_final_diagnostics(
                &state,
                &state,
                &settings,
                None,
                FinalDiagnosticsBackend::Cpu,
            )
            .is_err(),
        "a zero-dt pair is not an accepted step"
    );
    let mut inconsistent = state.clone();
    inconsistent.time += 0.25 * settings.dt;
    assert!(
        solver
            .sample_final_diagnostics(
                &state,
                &inconsistent,
                &settings,
                None,
                FinalDiagnosticsBackend::Cpu,
            )
            .is_err(),
        "a partial step is not accepted-state evidence"
    );
    let mut inconsistent = state.clone();
    inconsistent.time += settings.dt;
    inconsistent.x[0] += DVec3::X * 1e-3;
    assert!(
        solver
            .sample_final_diagnostics(
                &state,
                &inconsistent,
                &settings,
                None,
                FinalDiagnosticsBackend::Cpu,
            )
            .is_err(),
        "x_new - x_old must equal dt * v_new"
    );
}

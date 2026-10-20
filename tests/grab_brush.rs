use glam::DVec3;
use marvelous_cloth::{
    garment::GarmentTemplate,
    vbd::{Grab, VbdSettings, VbdSolver, grab::surface_patch},
};
use std::sync::Arc;
mod common;

fn fixture() -> Arc<marvelous_cloth::garment::CompiledGarment> {
    let mut t = GarmentTemplate::two_panel_demo();
    t.seams.clear();
    for p in &mut t.panels {
        p.pins.clear();
        p.pattern.nx = 2;
        p.pattern.ny = 2;
    }
    Arc::new(t.compile().unwrap())
}

#[test]
fn brush_preserves_offsets_and_stays_on_picked_piece() {
    let g = fixture();
    let x = &g.initial_state.x;
    let vertices = g.mesh.triangles[0].vertices;
    let center = vertices.iter().map(|&i| x[i] / 3.).sum::<DVec3>();
    let grab = Grab {
        vertices,
        weights: [1. / 3.; 3],
        target: center,
        stiffness: 150.,
        patch: surface_patch(
            &g.mesh,
            x,
            g.panels[0].triangles.clone(),
            vertices,
            center,
            2.,
        ),
    };
    assert!(grab.patch.len() > 3);
    assert!(grab.valid_patch(x.len()));
    assert!(
        grab.patch
            .iter()
            .all(|a| g.panels[0].vertices.contains(&a.vertex))
    );
    assert!(grab.patch_energy(x) < 1e-24);
    let translated: Vec<_> = x
        .iter()
        .map(|p| *p + DVec3::new(0.01, 0.03, -0.02))
        .collect();
    let mut shifted = grab.clone();
    shifted.target += DVec3::new(0.01, 0.03, -0.02);
    assert!(shifted.patch_energy(&translated) < 1e-24);
    assert!(
        surface_patch(
            &g.mesh,
            x,
            g.panels[0].triangles.clone(),
            vertices,
            center,
            0.
        )
        .is_empty()
    );
    assert!(surface_patch(&g.mesh, x, 0..0, vertices, center, 2.).is_empty());
    assert!(
        surface_patch(
            &g.mesh,
            x,
            g.panels[1].triangles.clone(),
            vertices,
            center,
            2.
        )
        .is_empty()
    );
}

#[test]
fn patch_pull_into_surface_then_release_keeps_contact_guards() {
    let (g, floor) = common::resting_patch(DVec3::ZERO);
    let mut state = g.initial_state.clone();
    let vertices = g.mesh.triangles[0].vertices;
    let center = vertices.iter().map(|&i| state.x[i] / 3.).sum::<DVec3>();
    let grab = Grab {
        vertices,
        weights: [1. / 3.; 3],
        target: center - DVec3::Y * 0.02,
        stiffness: 600.,
        patch: surface_patch(
            &g.mesh,
            &state.x,
            0..g.mesh.triangles.len(),
            vertices,
            center,
            1.,
        ),
    };
    let mut solver = VbdSolver::new(g, Some(&floor), &state).unwrap();
    let settings = VbdSettings {
        iterations: 8,
        ..Default::default()
    };
    for step in 0..30 {
        solver
            .step(&mut state, &settings, (step < 15).then_some(&grab))
            .unwrap();
        assert!(state.x.iter().all(|p| p.is_finite() && p.y > 0.));
        let contacts = solver
            .world
            .detect(&solver.world.positions(&state.x), 0.012, &settings.contact)
            .unwrap();
        assert!(contacts.minimum_distance > 0.);
    }
}

#[test]
fn brush_force_matches_energy_and_releases_without_a_persistent_attachment() {
    let g = fixture();
    let mut state = g.initial_state.clone();
    let vertices = g.mesh.triangles[0].vertices;
    let center = vertices.iter().map(|&i| state.x[i] / 3.).sum::<DVec3>();
    let grab = Grab {
        vertices,
        weights: [1. / 3.; 3],
        target: center + DVec3::Z * 0.01,
        stiffness: 150.,
        patch: surface_patch(
            &g.mesh,
            &state.x,
            g.panels[0].triangles.clone(),
            vertices,
            center,
            2.,
        ),
    };
    let mut solver = VbdSolver::new(g.clone(), None, &state).unwrap();
    let settings = VbdSettings {
        gravity: DVec3::ZERO,
        iterations: 6,
        ..Default::default()
    };
    let contacts = solver
        .world
        .detect(&solver.world.positions(&state.x), 0.012, &settings.contact)
        .unwrap();
    let ids: Vec<_> = (0..state.x.len()).collect();
    let with = solver
        .audit_vertex_forces(&state, &settings, &contacts, Some(&grab), &ids)
        .unwrap();
    let without = solver
        .audit_vertex_forces(&state, &settings, &contacts, None, &ids)
        .unwrap();
    let extra_force: DVec3 = with.iter().zip(&without).map(|(a, b)| *a - *b).sum();
    // Total force stays at the original spring budget, regardless of radius.
    assert!((extra_force - DVec3::Z * 1.5).length() < 1e-8);
    let id = grab.patch.last().unwrap().vertex;
    let eps = 1e-7;
    let mut plus = state.clone();
    plus.x[id].z += eps;
    let mut minus = state.clone();
    minus.x[id].z -= eps;
    let derivative = (solver
        .audit_energy(&plus, &settings, &contacts, Some(&grab))
        .unwrap()
        .grab_j
        - solver
            .audit_energy(&minus, &settings, &contacts, Some(&grab))
            .unwrap()
            .grab_j)
        / (2. * eps);
    assert!((derivative + with[id].z - without[id].z).abs() < 1e-7);
    for _ in 0..6 {
        solver.step(&mut state, &settings, Some(&grab)).unwrap();
    }
    assert!(state.x[id].z > g.initial_state.x[id].z);
    for _ in 0..10 {
        solver.step(&mut state, &settings, None).unwrap();
        let c = solver
            .world
            .detect(&solver.world.positions(&state.x), 0.012, &settings.contact)
            .unwrap();
        assert_eq!(
            solver
                .audit_energy(&state, &settings, &c, None)
                .unwrap()
                .grab_j,
            0.
        );
    }
    let before = state.clone();
    let mut invalid = grab.clone();
    invalid.patch[0].weight = f64::NAN;
    assert!(solver.step(&mut state, &settings, Some(&invalid)).is_err());
    assert_eq!(before.x, state.x);
    assert_eq!(before.time, state.time);
}

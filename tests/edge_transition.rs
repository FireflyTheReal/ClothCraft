use glam::{DQuat, DVec3};
use marvelous_cloth::{
    ogc::{
        OgcSettings, OgcWorld, geometry,
        potential::{Contact, Kind},
    },
    vbd::{Grab, VbdSettings, VbdSolver},
};
mod common;

// A local star rejects endpoint contact, while its neighboring edge accepts
// interior contact. Public, separated geometry; no private garment indices.
fn fixture(h: f64) -> (Vec<DVec3>, Vec<[usize; 3]>) {
    (
        vec![
            DVec3::ZERO,
            DVec3::new(0., 0.02, 0.),
            DVec3::new(-0.01, h, 0.0002),
            DVec3::new(0.01, h, 0.0002),
            DVec3::new(0.02, 0., -0.01),
            DVec3::new(-0.02, -0.02, 0.02),
            DVec3::new(0.02, -0.02, 0.02),
            DVec3::new(-0.01, h, 0.02),
        ],
        vec![[0, 1, 4], [0, 4, 5], [0, 5, 6], [2, 3, 7]],
    )
}

#[test]
fn endpoint_entry_reproduces_finite_jump_and_experiment_keeps_bounds_exact() {
    let cfg = OgcSettings::default();
    let mut original = Vec::new();
    let mut smooth = Vec::new();
    for h in [-1e-9, 1e-9] {
        let (x, faces) = fixture(h);
        let mut world = OgcWorld::new(&x, faces, x.len(), []).unwrap();
        world.directional_bounds = true;
        world.validate_start(&x).unwrap();
        assert!(!geometry::vertex_feasible(
            DVec3::new(0., h, 0.0002),
            0,
            &x,
            &world.topology
        ));
        let old = world.detect(&x, cfg.radius * 2., &cfg).unwrap();
        world.experimental_interior_edges = true;
        let new = world.detect(&x, cfg.radius * 2., &cfg).unwrap();
        assert_eq!(old.radii, new.radii);
        assert_eq!(old.minimum_distance, new.minimum_distance);
        assert_eq!(old.directional, new.directional);
        let pair_energy = |cs: &marvelous_cloth::ogc::ContactSet| {
            cs.contacts
                .iter()
                .filter(|c| c.vertices == [0, 1, 2, 3])
                .map(|c| c.value(&x, cfg.radius, cfg.stiffness))
                .sum::<f64>()
        };
        original.push(pair_energy(&old));
        smooth.push(pair_energy(&new));
        assert!(new.contacts.iter().any(|c| c.vertices == [0, 1, 2, 3]));
    }
    assert_eq!(original[0], 0.);
    assert!(original[1] > 0.5);
    assert_eq!(smooth[0], 0.);
    assert!(smooth[1] < 1e-15);
}

#[test]
fn concave_face_entry_is_a_separate_refresh_discontinuity() {
    // Two incident faces form a valley. The second face rejects the shared
    // edge's contact, but entering the first face adds a finite plane energy.
    // This independent public fixture must not be mistaken for an EE fix.
    let cfg = OgcSettings::default();
    let geometry = |h| {
        vec![
            DVec3::new(0., h, 0.0002),
            DVec3::new(-0.01, 0., 0.),
            DVec3::new(0.01, 0., 0.),
            DVec3::new(0., 0.02, 0.),
            DVec3::new(0., -0.02, 0.01),
            DVec3::new(-0.001, h, 0.02),
            DVec3::new(0.001, h + 0.001, 0.02),
        ]
    };
    let before = geometry(-1e-9);
    let after = geometry(1e-9);
    let mut world = OgcWorld::new(&before, vec![[1, 2, 3], [2, 1, 4], [0, 5, 6]], 7, []).unwrap();
    world.directional_bounds = true;
    for x in [&before, &after] {
        world.validate_start(x).unwrap();
    }
    let edge = world
        .topology
        .edges
        .iter()
        .position(|e| *e == [1, 2])
        .unwrap();
    assert!(!geometry::edge_feasible(
        before[0],
        edge,
        &before,
        &world.topology
    ));
    let old = world.detect(&before, cfg.radius * 2., &cfg).unwrap();
    let new = world.detect(&after, cfg.radius * 2., &cfg).unwrap();
    let witness = |c: &&Contact| c.kind == Kind::PointFace && c.vertices == [0, 1, 2, 3];
    assert!(!old.contacts.iter().any(|c| witness(&c)));
    let added = new.contacts.iter().find(witness).unwrap();
    assert!(added.value(&after, cfg.radius, cfg.stiffness) > 0.5);
    let event = marvelous_cloth::ogc::diagnostics::refresh_event(
        &old,
        &new,
        &after,
        7,
        &cfg,
        1,
        new.value(&after, &cfg) - old.value(&after, &cfg),
    );
    let change = event
        .largest_feature_changes
        .iter()
        .find(|c| c.kind == "PointFace" && c.vertices == [0, 1, 2, 3])
        .unwrap();
    assert!(change.delta_j > 0.5);
    assert_eq!(event.previous_detection_positions_m, before);
    assert_eq!(event.dynamic_positions_m, after);
    // Smoothing EE alone does not cure this independent face-family switch.
    world.experimental_interior_edges = true;
    let candidate = world.detect(&after, cfg.radius * 2., &cfg).unwrap();
    assert_eq!(
        candidate
            .contacts
            .iter()
            .find(witness)
            .unwrap()
            .value(&after, cfg.radius, cfg.stiffness),
        added.value(&after, cfg.radius, cfg.stiffness)
    );
    assert_eq!(new.radii, candidate.radii);
    assert_eq!(new.directional, candidate.directional);
}

#[test]
fn experimental_energy_derivatives_objectivity_and_interior_limit() {
    let r = 0.003;
    let k = 100_000.;
    let c = Contact {
        kind: Kind::EdgeEdgeInterior,
        vertices: [0, 1, 2, 3],
        count: 4,
        mollifier_epsilon: 1.6e-10,
    };
    for h in [-1e-5, 1e-5, 0.0009, 0.0029, 0.008] {
        let (x, _) = fixture(h);
        let mut force = DVec3::ZERO;
        let mut torque = DVec3::ZERO;
        for i in 0..4 {
            let (_, g, hh) = c.block(&x, i, r, k);
            force += g;
            torque += x[i].cross(g);
            for a in 0..3 {
                let mut plus = x.clone();
                let mut minus = x.clone();
                let eps = 1e-8;
                plus[i][a] += eps;
                minus[i][a] -= eps;
                let fd = (c.value(&plus, r, k) - c.value(&minus, r, k)) / (2. * eps);
                assert!(
                    (fd - g[a]).abs() < 2e-5 * (1. + g[a].abs()),
                    "h={h} g={g:?} fd={fd}"
                );
                let hg = (c.block(&plus, i, r, k).1 - c.block(&minus, i, r, k).1) / (2. * eps);
                assert!(
                    (hg - hh.col(a)).length() < 1e-3 + 2e-5 * hh.col(a).length(),
                    "h={h} i={i} a={a}: fd={hg:?}, analytic={:?}",
                    hh.col(a)
                );
            }
        }
        assert!(force.length() < 1e-9);
        assert!(torque.length() < 1e-10);
        let rot = DQuat::from_axis_angle(DVec3::new(1., 2., -1.).normalize(), 0.7);
        let transformed: Vec<_> = x
            .iter()
            .map(|p| rot * *p + DVec3::new(0.4, -0.2, 0.1))
            .collect();
        assert!((c.value(&x, r, k) - c.value(&transformed, r, k)).abs() < 1e-10);
        if h > r {
            let old = Contact {
                kind: Kind::EdgeEdge,
                ..c.clone()
            };
            assert_eq!(old.value(&x, r, k), c.value(&x, r, k));
        }
    }
}

#[test]
fn production_vbd_pull_release_and_tangential_motion_are_not_gated() {
    let (g, floor) = common::resting_patch(DVec3::ZERO);
    let settings = VbdSettings {
        dt: 1. / 120.,
        iterations: 12,
        ..Default::default()
    };
    let vertices = g.mesh.triangles[0].vertices;
    let anchor = vertices
        .iter()
        .map(|i| g.initial_state.x[*i])
        .sum::<DVec3>()
        / 3.;
    let mut trajectories = Vec::new();
    let mut state_traces = Vec::new();
    for experiment in [false, true] {
        let mut state = g.initial_state.clone();
        state.v.fill(DVec3::X * 0.05);
        let mut solver = VbdSolver::new(g.clone(), Some(&floor), &state).unwrap();
        solver.world.experimental_interior_edges = experiment;
        if experiment {
            solver.contact_refresh_audit = Some(Vec::new());
        }
        let mut path = Vec::new();
        let mut states = Vec::new();
        for step in 0..120 {
            let grab = (30..60).contains(&step).then_some(Grab {
                patch: Vec::new(),
                vertices,
                weights: [1. / 3.; 3],
                target: anchor + DVec3::new(0.03, 0.02, 0.),
                stiffness: 10.,
            });
            solver.step(&mut state, &settings, grab.as_ref()).unwrap();
            if let Some(audit) = &solver.contact_refresh_audit {
                assert!(audit.iter().all(|delta| delta.is_finite()));
            }
            solver
                .world
                .validate_start(&solver.world.positions(&state.x))
                .unwrap();
            assert!(state.x.iter().all(|p| p.y > 0.));
            path.push(vertices.iter().map(|i| state.x[*i]).sum::<DVec3>() / 3.);
            states.push(serde_json::to_vec(&state).unwrap());
        }
        assert!(path[29].x - anchor.x > 0.008, "sliding must remain mobile");
        assert!(path[59].y - path[29].y > 0.005, "grab must lift the patch");
        assert!(path[119].y < path[59].y - 0.005, "release must fall");
        trajectories.push(path);
        state_traces.push(states);
    }
    // This fixture has no nearby EE interiors: ordinary supported movement
    // must remain exactly unchanged, not merely visually similar.
    assert_eq!(trajectories[0], trajectories[1]);
    assert_eq!(state_traces[0], state_traces[1]);
}

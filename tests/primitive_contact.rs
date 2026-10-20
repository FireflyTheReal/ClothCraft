use glam::{DQuat, DVec3};
use marvelous_cloth::{
    ogc::{
        OgcSettings, OgcWorld, geometry,
        potential::{Contact, Kind, activation},
    },
    vbd::{Grab, VbdSettings, VbdSolver},
};
mod common;

fn contact(kind: Kind) -> Contact {
    Contact {
        kind,
        vertices: [0, 1, 2, 3],
        count: 4,
        mollifier_epsilon: 1.6e-10,
    }
}

#[test]
fn finite_distances_agree_with_geometry_and_differentiate_selected_coordinates() {
    let cfg = OgcSettings::default();
    // Point/vertex, point/edge and point/face regions, not only face interiors.
    for kind in [Kind::PointTriangle, Kind::SegmentSegment] {
        let c = contact(kind);
        for a in 0..17 {
            for b in 0..13 {
                let h = -0.006 + a as f64 * 0.002;
                let z = 0.0004 + b as f64 * 0.00007;
                let x = if kind == Kind::PointTriangle {
                    vec![
                        DVec3::new(h, 0.0021, z),
                        DVec3::ZERO,
                        DVec3::new(0.02, 0., 0.),
                        DVec3::new(0., 0.02, 0.),
                    ]
                } else {
                    vec![
                        DVec3::ZERO,
                        DVec3::new(0., 0.02, 0.),
                        DVec3::new(-0.01, h, z),
                        DVec3::new(0.01, h + 0.001, z),
                    ]
                };
                let d = if kind == Kind::PointTriangle {
                    let top =
                        marvelous_cloth::ogc::topology::Topology::new(4, vec![[1, 2, 3]]).unwrap();
                    x[0].distance(geometry::closest_triangle(x[0], 0, &x, &top).0)
                } else {
                    let (s, t) = geometry::closest_segments(x[0], x[1], x[2], x[3]);
                    x[0].lerp(x[1], s).distance(x[2].lerp(x[3], t))
                };
                assert!(
                    (c.value(&x, cfg.radius, cfg.stiffness)
                        - activation(d, cfg.radius, cfg.stiffness))
                    .abs()
                        < 1e-10
                );
                // Away from switching surfaces, finite differences must include
                // closest-coordinate motion, rather than frozen barycentrics.
                if a % 4 != 1 {
                    continue;
                }
                let mut net = DVec3::ZERO;
                let mut torque = DVec3::ZERO;
                for i in 0..4 {
                    let (_, g, hess) = c.block(&x, i, cfg.radius, cfg.stiffness);
                    net += g;
                    torque += x[i].cross(g);
                    for axis in 0..3 {
                        let eps = 1e-8;
                        let mut plus = x.clone();
                        plus[i][axis] += eps;
                        let mut minus = x.clone();
                        minus[i][axis] -= eps;
                        let fd = (c.value(&plus, cfg.radius, cfg.stiffness)
                            - c.value(&minus, cfg.radius, cfg.stiffness))
                            / (2. * eps);
                        assert!(
                            (fd - g[axis]).abs() < 2e-5 * (1. + g[axis].abs()),
                            "{kind:?} {a}/{b} axis {axis}"
                        );
                        let hg = (c.block(&plus, i, cfg.radius, cfg.stiffness).1
                            - c.block(&minus, i, cfg.radius, cfg.stiffness).1)
                            / (2. * eps);
                        assert!(
                            (hg - hess.col(axis)).length() < 0.005 + 5e-5 * hess.col(axis).length()
                        );
                    }
                }
                assert!(net.length() < 1e-8 && torque.length() < 1e-9);
                let q = DQuat::from_axis_angle(DVec3::new(1., 2., 3.).normalize(), 0.7);
                let transformed: Vec<_> = x
                    .iter()
                    .map(|p| q * *p + DVec3::new(0.3, -0.7, 0.2))
                    .collect();
                assert!(
                    (c.value(&x, cfg.radius, cfg.stiffness)
                        - c.value(&transformed, cfg.radius, cfg.stiffness))
                    .abs()
                        < 1e-10
                );
            }
        }
    }
}

fn fixture(face: bool, h: f64) -> (Vec<DVec3>, Vec<[usize; 3]>) {
    if face {
        (
            vec![
                DVec3::new(0., h, 0.0002),
                DVec3::new(-0.01, 0., 0.),
                DVec3::new(0.01, 0., 0.),
                DVec3::new(0., 0.02, 0.),
                DVec3::new(0., -0.02, 0.01),
                DVec3::new(-0.001, h, 0.02),
                DVec3::new(0.001, h + 0.001, 0.02),
            ],
            vec![[1, 2, 3], [2, 1, 4], [0, 5, 6]],
        )
    } else {
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
}

#[test]
fn both_feature_transitions_keep_energy_at_refresh_and_identical_geometric_bounds() {
    let cfg = OgcSettings::default();
    for face in [false, true] {
        let (before, triangles) = fixture(face, -1e-9);
        let (after, _) = fixture(face, 1e-9);
        let mut baseline = OgcWorld::new(&before, triangles, before.len(), []).unwrap();
        baseline.directional_bounds = true;
        let mut world = baseline.clone();
        world.experimental_primitive_distances = true;
        let old = world.detect(&before, 2. * cfg.radius, &cfg).unwrap();
        let new = world.detect(&after, 2. * cfg.radius, &cfg).unwrap();
        let original = baseline.detect(&after, 2. * cfg.radius, &cfg).unwrap();
        world.validate_start(&after).unwrap();
        assert_eq!(original.radii, new.radii);
        assert_eq!(original.directional, new.directional);
        assert_eq!(original.minimum_distance, new.minimum_distance);
        assert!((old.value(&after, &cfg) - new.value(&after, &cfg)).abs() < 1e-12);
        assert!(
            after
                .iter()
                .enumerate()
                .all(|(i, p)| old.contains(i, *p, 1e-14))
        );
        // A continuous contribution need not be small: do not confuse energy
        // conservation across refresh with removal of contact stiffness.
        assert!(new.value(&after, &cfg) > 0.5);
    }
}

#[test]
fn force_halo_covers_pairs_that_enter_activation_inside_the_original_bounds() {
    let cfg = OgcSettings::default();
    let r = cfg.radius;
    let before = vec![
        DVec3::new(-0.01, 1.1 * r, 0.),
        DVec3::new(0.01, 1.1 * r, 0.),
        DVec3::new(0., 1.1 * r, 0.02),
        DVec3::new(-1., 0., -1.),
        DVec3::new(1., 0., -1.),
        DVec3::new(0., 0., 1.),
    ];
    let mut base = OgcWorld::new(&before, vec![[0, 1, 2], [3, 4, 5]], 3, []).unwrap();
    base.directional_bounds = true;
    let mut world = base.clone();
    world.experimental_primitive_distances = true;
    let original = base.detect(&before, r, &cfg).unwrap();
    let old = world.detect(&before, r, &cfg).unwrap();
    assert_eq!(original.radii, old.radii);
    assert_eq!(old.value(&before, &cfg), 0.);
    assert!(old.contacts.len() > original.contacts.len());
    let mut after = before.clone();
    for (i, p) in after.iter_mut().take(3).enumerate() {
        p.y -= 0.3 * r;
        assert!(old.contains(i, *p, 1e-14));
    }
    let new = world.detect(&after, r, &cfg).unwrap();
    assert!(old.value(&after, &cfg) > 0.);
    assert!((old.value(&after, &cfg) - new.value(&after, &cfg)).abs() < 1e-12);
    // Reuse the enlarged temporal cache while changing geometry and query size.
    let mut seed = OgcWorld::new(&before, vec![[0, 1, 2], [3, 4, 5]], 3, []).unwrap();
    seed.directional_bounds = true;
    seed.experimental_primitive_distances = true;
    for frame in 0..40 {
        after[0].x += 1e-5;
        let q = if frame < 20 { r } else { 2. * r };
        let cached = world.detect(&after, q, &cfg).unwrap();
        let fresh = seed.clone().detect(&after, q, &cfg).unwrap();
        let keys = |c: &marvelous_cloth::ogc::ContactSet| {
            c.contacts
                .iter()
                .map(|c| (c.kind, c.vertices))
                .collect::<Vec<_>>()
        };
        assert_eq!(keys(&cached), keys(&fresh));
        assert_eq!(cached.value(&after, &cfg), fresh.value(&after, &cfg));
        assert_eq!(cached.radii, fresh.radii);
    }
    world.experimental_interior_edges = true;
    assert!(world.detect(&after, r, &cfg).is_err());
}

#[test]
fn supported_sliding_pull_release_matches_production_when_the_force_laws_coincide() {
    let (g, floor) = common::resting_patch(DVec3::ZERO);
    let settings = VbdSettings {
        dt: 1. / 120.,
        iterations: 12,
        ..Default::default()
    };
    let ids = g.mesh.triangles[0].vertices;
    let anchor = ids.iter().map(|i| g.initial_state.x[*i]).sum::<DVec3>() / 3.;
    let mut traces = Vec::new();
    for experiment in [false, true] {
        let mut state = g.initial_state.clone();
        state.v.fill(DVec3::X * 0.05);
        let mut solver = VbdSolver::new(g.clone(), Some(&floor), &state).unwrap();
        solver.world.experimental_primitive_distances = experiment;
        let mut trace = Vec::new();
        let mut centers = Vec::new();
        for step in 0..120 {
            let grab = (30..60).contains(&step).then_some(Grab {
                patch: Vec::new(),
                vertices: ids,
                weights: [1. / 3.; 3],
                target: anchor + DVec3::new(0.03, 0.02, 0.),
                stiffness: 10.,
            });
            solver.step(&mut state, &settings, grab.as_ref()).unwrap();
            solver
                .world
                .validate_start(&solver.world.positions(&state.x))
                .unwrap();
            trace.push(serde_json::to_vec(&state).unwrap());
            centers.push(ids.iter().map(|i| state.x[*i]).sum::<DVec3>() / 3.);
        }
        assert!(centers[29].x - anchor.x > 0.008);
        assert!(centers[59].y - centers[29].y > 0.005);
        assert!(centers[119].y < centers[59].y - 0.005);
        traces.push(trace);
    }
    assert_eq!(traces[0], traces[1]);
}

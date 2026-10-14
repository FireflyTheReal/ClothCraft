use glam::{DMat3, DVec3};
use marvelous_cloth::{
    math::Jet,
    ogc::{
        OgcSettings, OgcWorld, geometry,
        potential::{Contact, Kind, activation},
        topology::Topology,
    },
};
fn settings(r: f64) -> OgcSettings {
    OgcSettings {
        radius: r,
        ..Default::default()
    }
}
#[test]
fn two_stage_potential_is_c2_at_join_and_has_barrier() {
    let r = 0.01;
    let k = 1200.;
    let tau = r / 2.;
    let eps = 1e-9;
    let a = activation(Jet::<1>::variable(tau - eps, 0), r, k);
    let b = activation(Jet::<1>::variable(tau + eps, 0), r, k);
    assert!((a.v - b.v).abs() < 1e-7);
    assert!((a.g[0] - b.g[0]).abs() < 1e-5);
    assert!((a.h[0][0] - b.h[0][0]).abs() < 0.001);
    assert_eq!(activation(r, r, k), 0.0);
    assert!(activation(0.0, r, k).is_infinite());
    assert!(activation(1e-12, r, k) > activation(1e-6, r, k));
    for d in [0.001, 0.004, 0.007, 0.009] {
        let j = activation(Jet::<1>::variable(d, 0), r, k);
        let fd = (activation(d + eps, r, k) - activation(d - eps, r, k)) / (2. * eps);
        assert!((fd - j.g[0]).abs() < 1e-7);
    }
}
#[test]
fn contact_block_derivatives_and_force_balance() {
    let cases = [
        (Kind::PointPoint, vec![[0.1, 0.1, 0.04], [0.1, 0.1, 0.0]], 2),
        (
            Kind::PointEdge,
            vec![[0.1, 0.04, 0.02], [-1., 0., 0.], [1., 0., 0.]],
            3,
        ),
        (
            Kind::PointFace,
            vec![
                [0.1, 0.1, 0.04],
                [-1., -1., 0.],
                [1., -1., 0.],
                [0., 1., 0.],
            ],
            4,
        ),
        (
            Kind::EdgeEdge,
            vec![[-1., 0., 0.04], [1., 0., 0.04], [0., -1., 0.], [0., 1., 0.]],
            4,
        ),
    ];
    for (kind, p, count) in cases {
        let mut x: Vec<_> = p.into_iter().map(DVec3::from_array).collect();
        while x.len() < 4 {
            x.push(DVec3::ZERO);
        }
        let c = Contact {
            kind,
            vertices: [0, 1, 2, 3],
            count,
            mollifier_epsilon: 0.01,
        };
        let eps = 1e-7;
        let mut force = DVec3::ZERO;
        for v in 0..count {
            let (_, g, h) = c.block(&x, v, 0.1, 20.);
            force += g;
            for axis in 0..3 {
                let mut a = x.clone();
                let mut b = x.clone();
                a[v][axis] += eps;
                b[v][axis] -= eps;
                let fd = (c.value(&a, 0.1, 20.) - c.value(&b, 0.1, 20.)) / (2. * eps);
                assert!((fd - g[axis]).abs() < 1e-6, "{kind:?} gradient");
                let dg = (c.block(&a, v, 0.1, 20.).1 - c.block(&b, v, 0.1, 20.).1) / (2. * eps);
                assert!(
                    (dg - h.col(axis)).length() < 1e-4,
                    "{kind:?} Hessian {dg:?} {:?}",
                    h.col(axis)
                );
            }
        }
        assert!(force.length() < 1e-9, "{kind:?} action/reaction");
        let rotated: Vec<_> = x
            .iter()
            .map(|p| DMat3::from_rotation_y(0.64) * p + DVec3::splat(2.))
            .collect();
        assert!((c.value(&x, 0.1, 20.) - c.value(&rotated, 0.1, 20.)).abs() < 1e-10);
    }
}
#[test]
fn folded_surface_preserves_multiple_layers_and_excludes_false_edge_contact() {
    let x = vec![
        DVec3::new(0., 0.1, 0.1),
        DVec3::new(-1., 0., 0.),
        DVec3::new(1., 0., 0.),
        DVec3::new(0., 1., 0.),
        DVec3::new(0., 0., 1.),
    ];
    let top = Topology::new(x.len(), vec![[1, 2, 3], [2, 1, 4]]).unwrap();
    let e = top.edges.iter().position(|e| *e == [1, 2]).unwrap();
    assert!(!geometry::edge_feasible(x[0], e, &x, &top));
    let mut w = OgcWorld::new(&x, top.triangles, 1, []).unwrap();
    let s = w.detect(&x, 0.25, &settings(0.2)).unwrap();
    assert_eq!(
        s.contacts
            .iter()
            .filter(|c| c.kind == Kind::PointFace)
            .count(),
        2
    );
}
#[test]
fn shared_feature_contact_is_not_duplicated() {
    let x = vec![
        DVec3::new(0., -0.03, 0.04),
        DVec3::new(-1., 0., 0.),
        DVec3::new(1., 0., 0.),
        DVec3::new(0., 1., 0.),
        DVec3::new(0., 0., -1.),
    ];
    let mut w = OgcWorld::new(&x, vec![[1, 2, 3], [2, 1, 4]], 1, []).unwrap();
    let s = w.detect(&x, 0.2, &settings(0.1)).unwrap();
    assert_eq!(s.contacts.len(), 1);
    assert_eq!(s.contacts[0].kind, Kind::PointEdge);
}
#[test]
fn reverse_vertex_face_and_edge_edge_bounds_protect_face_interiors() {
    // Static point projects INSIDE moving triangle, far from its moving vertices.
    let x = vec![
        DVec3::new(-1., -1., 0.1),
        DVec3::new(1., -1., 0.1),
        DVec3::new(0., 1., 0.1),
        DVec3::ZERO,
        DVec3::new(0.01, 0., -1.),
        DVec3::new(0., 0.01, -1.),
    ];
    let mut w = OgcWorld::new(&x, vec![[0, 1, 2], [3, 4, 5]], 3, []).unwrap();
    let s = w.detect(&x, 0.2, &settings(0.15)).unwrap();
    for r in &s.radii {
        assert!(*r <= 0.045 + 1e-12);
    }
    for (i, position) in x.iter().enumerate().take(3) {
        let (p, b) = s.truncate(i, *position - DVec3::Z * 10.);
        assert!(b);
        assert!(p.z >= 0.055 - 1e-12);
    }
    // Crossing long edges: all endpoint distances are large; EE must bound them.
    let x = vec![
        DVec3::new(-1., 0., 0.1),
        DVec3::new(1., 0., 0.1),
        DVec3::new(-1., -1., 0.1),
        DVec3::new(0., -1., 0.),
        DVec3::new(0., 1., 0.),
        DVec3::new(1., 1., 0.),
    ];
    let mut w = OgcWorld::new(&x, vec![[0, 1, 2], [3, 4, 5]], 6, []).unwrap();
    let s = w.detect(&x, 0.2, &settings(0.15)).unwrap();
    assert!(s.contacts.iter().any(|c| c.kind == Kind::EdgeEdge));
    assert!(s.radii[0] <= 0.045 + 1e-12 && s.radii[1] <= 0.045 + 1e-12);
    let moved: Vec<_> = (0..6)
        .map(|i| {
            s.truncate(i, x[i] + if i < 3 { -DVec3::Z } else { DVec3::Z } * 10.)
                .0
        })
        .collect();
    w.validate_start(&moved).unwrap();
}
#[test]
fn bounds_are_cumulative_until_detection_and_settings_fail_closed() {
    let x = vec![
        DVec3::new(0., 0., 0.1),
        DVec3::new(-1., -1., 0.),
        DVec3::new(1., -1., 0.),
        DVec3::new(0., 1., 0.),
    ];
    let mut w = OgcWorld::new(&x, vec![[1, 2, 3]], 1, []).unwrap();
    let s = w.detect(&x, 0.2, &settings(0.15)).unwrap();
    let p = s.truncate(0, x[0] - DVec3::Z * 0.04).0;
    let q = s.truncate(0, p - DVec3::Z * 0.04).0;
    assert!((q.z - 0.055).abs() < 1e-12);
    assert!(w.detect(&x, 0.01, &settings(0.15)).is_err());
    assert!(
        OgcSettings {
            relaxation: 0.5,
            ..Default::default()
        }
        .validate()
        .is_err()
    );
}
#[test]
fn initial_intersection_is_rejected_instead_of_claiming_a_guarantee() {
    let x = vec![
        DVec3::new(0., 0., -1.),
        DVec3::new(0., 0., 1.),
        DVec3::new(2., 0., 0.),
        DVec3::new(-1., -1., 0.),
        DVec3::new(1., -1., 0.),
        DVec3::new(0., 1., 0.),
    ];
    let mut w = OgcWorld::new(&x, vec![[0, 1, 2], [3, 4, 5]], 3, []).unwrap();
    assert!(w.validate_start(&x).is_err());
}

#[test]
fn query_radius_changes_bounds_without_activating_distant_forces() {
    let x = vec![
        DVec3::new(0., 0., 0.02),
        DVec3::new(-1., -1., 0.),
        DVec3::new(1., -1., 0.),
        DVec3::new(0., 1., 0.),
    ];
    let mut w = OgcWorld::new(&x, vec![[1, 2, 3]], 1, []).unwrap();
    let s = w.detect(&x, 0.03, &settings(0.01)).unwrap();
    assert_eq!(s.contacts.len(), 1);
    assert_eq!(s.active_contacts, 0);
    assert_eq!(s.value(&x, &settings(0.01)), 0.);
    assert!((s.radii[0] - 0.009).abs() < 1e-12);
}

#[test]
fn dormant_contact_activates_inside_the_same_original_trust_region() {
    let mut x = vec![
        DVec3::new(0., 0., 0.00305),
        DVec3::new(-1., -1., 0.),
        DVec3::new(1., -1., 0.),
        DVec3::new(0., 1., 0.),
    ];
    let mut w = OgcWorld::new(&x, vec![[1, 2, 3]], 1, []).unwrap();
    let settings = settings(0.003);
    let contacts = w.detect(&x, 0.006, &settings).unwrap();
    assert_eq!(contacts.active_contacts, 0);
    assert_eq!(contacts.value(&x, &settings), 0.);
    let (p, clipped) = contacts.truncate(0, DVec3::new(0., 0., 0.0025));
    assert!(!clipped);
    x[0] = p;
    let expected = 0.5 * settings.stiffness * 0.0005_f64.powi(2);
    assert!((contacts.value(&x, &settings) - expected).abs() < 1e-12);
    assert!(
        (contacts.value(&x, &settings)
            - w.detect(&x, 0.006, &settings).unwrap().value(&x, &settings))
        .abs()
            < 1e-12
    );
}

#[test]
fn static_edges_before_moving_edges_are_not_missed() {
    let x = vec![
        DVec3::new(-1., 0., 0.1),
        DVec3::new(1., 0., 0.1),
        DVec3::new(-1., -1., 0.1),
        DVec3::new(0., -1., 0.),
        DVec3::new(0., 1., 0.),
        DVec3::new(1., 1., 0.),
    ];
    let mut a = OgcWorld::new(&x, vec![[0, 1, 2], [3, 4, 5]], 3, []).unwrap();
    let mut b = OgcWorld::new(&x, vec![[3, 4, 5], [0, 1, 2]], 3, []).unwrap();
    let sa = a.detect(&x, 0.2, &settings(0.15)).unwrap();
    let sb = b.detect(&x, 0.2, &settings(0.15)).unwrap();
    assert_eq!(sa.radii, sb.radii);
    assert_eq!(sa.edge_edge_candidates, sb.edge_edge_candidates);
    assert_eq!(sa.contacts.len(), sb.contacts.len());
}

#[test]
fn shared_vertex_near_parallel_edges_do_not_create_false_initial_intersections() {
    // Regression from a GPU-draped shirt. An incident edge produced t≈1-1e-10
    // at the COMMON endpoint due to a near-zero ray/plane determinant.
    let x = vec![
        [0.194131180644, 0.943517088890, 0.091018132865],
        [0.183311969042, 0.970439314842, 0.101929388940],
        [0.197149559855, 0.954595983028, 0.076061360538],
        [0.195560514927, 0.972076416016, 0.068995960057],
        [0.188853010535, 0.980777919292, 0.085684590042],
    ]
    .into_iter()
    .map(|p: [f64; 3]| DVec3::from_array(p.map(|v| v as f32 as f64)))
    .collect::<Vec<_>>();
    let mut w = OgcWorld::new(&x, vec![[0, 1, 2], [3, 2, 4]], 5, []).unwrap();
    w.validate_start(&x).unwrap();
}

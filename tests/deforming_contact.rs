use glam::{DQuat, DVec3};
use marvelous_cloth::{
    ogc::{feature_hierarchy::Pair, topology::Topology},
    solver_lab::deforming_contact::{self, Certificate},
};
fn fixture() -> (Vec<DVec3>, Topology, Pair, Pair) {
    let x = vec![
        DVec3::new(-0.5, 0., 1.),
        DVec3::new(0.5, 0., 1.),
        DVec3::new(0.1, 0.3, 1.),
        DVec3::new(0., -0.5, 0.),
        DVec3::new(0., 0.5, 0.),
        DVec3::new(1., 0., 0.),
    ];
    let top = Topology::new(x.len(), vec![[0, 1, 2], [3, 4, 5]]).unwrap();
    let edge = |a, b| top.edges.iter().position(|e| *e == [a, b]).unwrap();
    let ee = Pair::EdgeEdge([edge(0, 1), edge(3, 4)]);
    (x, top, Pair::VertexFace([2, 1]), ee)
}
#[test]
fn all_moving_contact_gradients_match_finite_differences_and_internal_balance() {
    let (x, top, vf, ee) = fixture();
    let mut outside = x.clone();
    outside[2].x = 1.2;
    for p in &mut outside[3..] {
        p.x += 0.6;
    }
    for (x, pair) in [(&x, vf), (&x, ee), (&outside, vf), (&outside, ee)] {
        let row = deforming_contact::linearize(pair, &top, x, 0.01).unwrap();
        let gradient: Vec<_> = row
            .jacobian
            .as_chunks::<3>()
            .0
            .iter()
            .map(|p| DVec3::from_array(*p))
            .collect();
        assert!(gradient.iter().copied().sum::<DVec3>().length() < 1e-12);
        assert!(
            x.iter()
                .zip(&gradient)
                .map(|(p, g)| p.cross(*g))
                .sum::<DVec3>()
                .length()
                < 1e-12
        );
        let rotation = DQuat::from_rotation_y(0.7) * DQuat::from_rotation_x(-0.3);
        let transformed: Vec<_> = x
            .iter()
            .map(|p| rotation * *p + DVec3::new(0.8, -0.4, 0.3))
            .collect();
        let rotated = deforming_contact::linearize(pair, &top, &transformed, 0.01).unwrap();
        assert!((rotated.offset - row.offset).abs() < 1e-12);
        for (before, after) in gradient
            .iter()
            .zip(rotated.jacobian.as_chunks::<3>().0.iter())
        {
            assert!((rotation * *before - DVec3::from_slice(after)).length() < 1e-10);
        }
        for k in 0..x.len() * 3 {
            let mut a = x.clone();
            let mut b = x.clone();
            a[k / 3][k % 3] += 1e-6;
            b[k / 3][k % 3] -= 1e-6;
            let fd = (pair.distance(&top, &a) - pair.distance(&top, &b)) / 2e-6;
            assert!((fd - row.jacobian[k]).abs() < 1e-8, "{pair:?} dof {k}");
        }
    }
}
#[test]
fn safe_endpoints_do_not_hide_vf_or_ee_tunneling() {
    let (x, top, vf, ee) = fixture();
    let mut end = x.clone();
    for p in &mut end[..3] {
        p.z -= 2.;
    }
    for pair in [vf, ee] {
        assert!(pair.distance(&top, &x) > 0.9 && pair.distance(&top, &end) > 0.9);
        assert!(matches!(
            deforming_contact::certify(pair, &top, &x, &end, 0.01, 1e-10, 10).unwrap(),
            Certificate::Violated { .. }
        ));
    }
}
#[test]
fn common_motion_and_tangential_sliding_keep_separation_certificates() {
    let (x, top, vf, ee) = fixture();
    let end: Vec<_> = x
        .iter()
        .enumerate()
        .map(|(i, p)| {
            *p + DVec3::new(2., 3., 4.)
                + if i < 3 {
                    DVec3::new(0.01, 0., 0.)
                } else {
                    DVec3::ZERO
                }
        })
        .collect();
    for pair in [vf, ee] {
        assert!(matches!(
            deforming_contact::certify(pair, &top, &x, &end, 1., 1e-10, 8).unwrap(),
            Certificate::Safe { intervals: 1 }
        ));
    }
}
#[test]
fn rotating_deforming_edges_need_subdivision_and_budget_exhaustion_is_not_safe() {
    let (_, top, _, ee) = fixture();
    let make = |axis: DVec3, offset: DVec3| {
        vec![
            offset - 0.5 * axis,
            offset + 0.5 * axis,
            offset + DVec3::Z,
            -0.5 * axis,
            0.5 * axis,
            DVec3::Z,
        ]
    };
    let a = make(DVec3::Y, 0.1 * DVec3::X);
    let b = make(DVec3::X, -0.1 * DVec3::Y);
    assert!(matches!(
        deforming_contact::certify(ee, &top, &a, &b, 0.06, 1e-10, 0).unwrap(),
        Certificate::Unresolved { .. }
    ));
    assert!(
        matches!(deforming_contact::certify(ee,&top,&a,&b,0.06,1e-10,8).unwrap(),Certificate::Safe{intervals} if intervals>1)
    );
    for i in 0..101 {
        let x: Vec<_> = a
            .iter()
            .zip(&b)
            .map(|(p, q)| p.lerp(*q, i as f64 / 100.))
            .collect();
        assert!(ee.distance(&top, &x) > 0.0707);
    }
}

#[test]
fn affine_separator_certifies_corotating_close_primitives_without_loosening_gap() {
    let (_, top, vf, ee) = fixture();
    for pair in [vf, ee] {
        for angle in [0.005_f64, 0.02, 0.08, 0.2] {
            // Under linear interpolation of identity/rotation, the perpendicular
            // separation reaches gap*cos(angle/2) at the interval midpoint.
            let gap = 0.001 / (angle * 0.5).cos() * 1.0001;
            let (mut x, _, _, _) = fixture();
            for p in &mut x[..3] {
                p.z = gap;
            }
            let rotation = DQuat::from_rotation_y(angle);
            let end: Vec<_> = x
                .iter()
                .map(|p| rotation * *p + DVec3::new(0.03, -0.02, 0.01))
                .collect();
            let certificate =
                deforming_contact::certify_moving_planes(pair, &top, &x, &end, 0.001, 1e-10, 10)
                    .unwrap();
            assert!(
                matches!(certificate, Certificate::Safe { .. }),
                "{pair:?}, {angle}, {certificate:?}"
            );
            for i in 0..1001 {
                let at: Vec<_> = x
                    .iter()
                    .zip(&end)
                    .map(|(p, q)| p.lerp(*q, i as f64 / 1000.))
                    .collect();
                assert!(pair.distance(&top, &at) >= 0.001 - 1e-12);
            }
            if angle == 0.02 {
                assert!(matches!(
                    deforming_contact::certify(pair, &top, &x, &end, 0.001, 1e-10, 10).unwrap(),
                    Certificate::Unresolved { .. }
                ));
            }
        }
    }
}

#[test]
fn affine_separator_rejects_interior_clearance_loss_and_tunneling() {
    let (mut x, top, vf, ee) = fixture();
    for p in &mut x[..3] {
        p.z = 0.00100001;
    }
    let rotation = DQuat::from_rotation_y(0.02);
    let end: Vec<_> = x.iter().map(|p| rotation * *p).collect();
    for pair in [vf, ee] {
        assert!(pair.distance(&top, &x) > 0.001 && pair.distance(&top, &end) > 0.001);
        assert!(matches!(
            deforming_contact::certify_moving_planes(pair, &top, &x, &end, 0.001, 1e-10, 10)
                .unwrap(),
            Certificate::Violated { .. }
        ));
    }
    let (x, top, vf, ee) = fixture();
    let mut end = x.clone();
    for p in &mut end[..3] {
        p.z -= 2.;
    }
    for pair in [vf, ee] {
        assert!(matches!(
            deforming_contact::certify_moving_planes(pair, &top, &x, &end, 0.01, 1e-10, 10)
                .unwrap(),
            Certificate::Violated { .. }
        ));
    }
}

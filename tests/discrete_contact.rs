use glam::{DQuat, DVec3};
use marvelous_cloth::{
    ogc::{feature_hierarchy::Pair, topology::Topology},
    solver_lab::{
        deforming_contact::{self, Certificate},
        discrete_contact,
    },
};
fn fixture() -> (Vec<DVec3>, Topology, Pair, Pair) {
    let x = vec![
        DVec3::new(-0.5, 0., 0.02),
        DVec3::new(0.5, 0., 0.02),
        DVec3::new(0., -0.5, 0.),
        DVec3::new(0., 0.5, 0.),
        DVec3::new(0.7, 0.6, 0.),
    ];
    let top = Topology::new(5, vec![[0, 1, 4], [2, 3, 4]]).unwrap();
    let edge = |a, b| top.edges.iter().position(|e| *e == [a, b]).unwrap();
    let ee = Pair::EdgeEdge([edge(0, 1), edge(2, 3)]);
    (x, top, Pair::VertexFace([0, 1]), ee)
}
#[test]
fn discrete_rows_obey_chain_rule_internal_balance_and_distance_oracle() {
    let (base, top, vf, ee) = fixture();
    let mut seed = 415u64;
    let mut random = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 32) as f64 / u32::MAX as f64 - 0.5
    };
    for _ in 0..128 {
        let mut old = base.clone();
        let mut new = base.clone();
        for p in &mut old {
            *p += 0.02 * DVec3::new(random(), random(), random());
        }
        for p in &mut new {
            *p += 0.02 * DVec3::new(random(), random(), random());
        }
        for pair in [vf, ee] {
            let r = discrete_contact::row(pair, &top, &old, &new).unwrap();
            assert!((r.distance_before - pair.distance(&top, &old)).abs() < 1e-12);
            assert!((r.distance_after - pair.distance(&top, &new)).abs() < 1e-12);
            assert!(r.chain_error_m.abs() < 1e-12, "{}", r.chain_error_m);
            assert!(r.gradient.iter().copied().sum::<DVec3>().length() < 1e-12);
            let origin = 0.5 * (old[r.vertices[0]] + new[r.vertices[0]]);
            let torque: DVec3 = r
                .vertices
                .iter()
                .zip(&r.gradient)
                .map(|(&i, f)| (0.5 * (old[i] + new[i]) - origin).cross(*f))
                .sum();
            assert!(torque.length() < 1e-12);
        }
    }
}
#[test]
fn static_limit_matches_current_geometry_and_is_objective_and_symmetric() {
    let (old, top, vf, ee) = fixture();
    let mut new = old.clone();
    new[0] += DVec3::new(0.08, 0.03, 0.01);
    new[3].z -= 0.02;
    let rotation = DQuat::from_rotation_y(0.4) * DQuat::from_rotation_z(-0.2);
    for pair in [vf, ee] {
        let current = deforming_contact::linearize(pair, &top, &old, 0.001).unwrap();
        let stationary = discrete_contact::row(pair, &top, &old, &old).unwrap();
        for (&id, f) in stationary.vertices.iter().zip(&stationary.gradient) {
            assert!(f.distance(DVec3::from_slice(&current.jacobian[3 * id..3 * id + 3])) < 1e-11);
        }
        let forward = discrete_contact::row(pair, &top, &old, &new).unwrap();
        let reverse = discrete_contact::row(pair, &top, &new, &old).unwrap();
        for i in 0..4 {
            assert!(forward.gradient[i].distance(reverse.gradient[i]) < 1e-12);
        }
        for scale in [0.001, 1., 1000.] {
            let transform = |x: &[DVec3]| {
                x.iter()
                    .map(|p| scale * (rotation * *p + DVec3::new(0.8, -0.2, 0.3)))
                    .collect::<Vec<_>>()
            };
            let transformed =
                discrete_contact::row(pair, &top, &transform(&old), &transform(&new)).unwrap();
            for i in 0..4 {
                assert!(
                    (transformed.gradient[i] - rotation * forward.gradient[i]).length() < 1e-10
                );
            }
            assert!(transformed.chain_error_m.abs() < 1e-12 * scale);
        }
    }
}
#[test]
fn frictionless_face_and_edge_sliding_reject_frozen_witness_drag() {
    let top = Topology::new(4, vec![[1, 2, 3]]).unwrap();
    let mut old = vec![
        DVec3::new(-0.2, 0., 0.001),
        DVec3::new(-1., -1., 0.),
        DVec3::new(1., -1., 0.),
        DVec3::new(0., 1., 0.),
    ];
    let mut new = old.clone();
    new[0].x += 0.4;
    let row = discrete_contact::row(Pair::VertexFace([0, 0]), &top, &old, &new).unwrap();
    assert!(row.gradient[0].x.abs() < 1e-10 && row.gradient[0].y.abs() < 1e-10);
    assert!(row.gradient[0].z > 0.);
    assert!(row.frozen_witness_gradient[0].x < -0.9);
    assert!(row.frozen_witness_old_distance > 0.39);
    assert!(row.chain_error_m.abs() < 1e-12);
    let (base, top, _, ee) = fixture();
    old = base.clone();
    new = base;
    new[0].x += 0.2;
    new[1].x += 0.2;
    let row = discrete_contact::row(ee, &top, &old, &new).unwrap();
    assert!((row.gradient[0] + row.gradient[1]).x.abs() < 1e-11);
    assert!((row.frozen_witness_gradient[0] + row.frozen_witness_gradient[1]).x < -0.8);
}
#[test]
fn work_identity_is_not_a_swept_collision_certificate() {
    let old = vec![
        DVec3::new(0., 0., 0.01),
        DVec3::new(-1., -1., 0.),
        DVec3::new(1., -1., 0.),
        DVec3::new(0., 1., 0.),
    ];
    let mut new = old.clone();
    new[0].z = -0.01;
    let top = Topology::new(4, vec![[1, 2, 3]]).unwrap();
    let pair = Pair::VertexFace([0, 0]);
    let row = discrete_contact::row(pair, &top, &old, &new).unwrap();
    assert!(row.chain_error_m.abs() < 1e-12);
    assert!(matches!(
        deforming_contact::certify(pair, &top, &old, &new, 0.001, 1e-10, 10).unwrap(),
        Certificate::Violated { .. }
    ));
}
#[test]
fn tiny_changes_rigid_motion_and_invalid_inputs_are_explicit() {
    let (old, top, vf, ee) = fixture();
    let rotation = DQuat::from_rotation_z(0.7);
    let rigid: Vec<_> = old
        .iter()
        .map(|p| rotation * *p + DVec3::new(0.4, 0.1, -0.2))
        .collect();
    for pair in [vf, ee] {
        let r = discrete_contact::row(pair, &top, &old, &rigid).unwrap();
        assert!(r.near_invariant_coincidence);
        assert!(r.chain_error_m.abs() < 1e-12);
        for epsilon in [1e-5, 1e-8, 1e-12, 1e-15] {
            let mut new = old.clone();
            new[0].z += epsilon;
            let r = discrete_contact::row(pair, &top, &old, &new).unwrap();
            assert!(r.chain_error_m.abs() < 1e-12);
            assert!(r.gradient.iter().all(|g| g.length() < 10.));
        }
    }
    assert!(discrete_contact::row(vf, &top, &old[..3], &old).is_err());
    let mut bad = old.clone();
    bad[0].x = f64::NAN;
    assert!(discrete_contact::row(vf, &top, &old, &bad).is_err());
    let huge: Vec<_> = old.iter().map(|p| *p * 1e90).collect();
    let mut moved = huge.clone();
    moved[0].z += 1e89;
    assert!(discrete_contact::row(ee, &top, &huge, &moved).is_err());
}

#[test]
fn force_rows_approach_the_geometric_normal_as_motion_shrinks() {
    let (old, top, vf, ee) = fixture();
    for pair in [vf, ee] {
        let mut errors = vec![];
        for amount in [1e-2, 1e-3, 1e-4, 1e-5, 1e-6] {
            let mut new = old.clone();
            new[0] += amount * DVec3::new(0.2, 0.1, 0.3);
            new[3] += amount * DVec3::new(-0.1, 0.2, -0.1);
            let discrete = discrete_contact::row(pair, &top, &old, &new).unwrap();
            let current = deforming_contact::linearize(pair, &top, &new, 0.001).unwrap();
            let error = discrete
                .vertices
                .iter()
                .zip(&discrete.gradient)
                .map(|(&i, g)| {
                    g.distance_squared(DVec3::from_slice(&current.jacobian[3 * i..3 * i + 3]))
                })
                .sum::<f64>()
                .sqrt();
            errors.push(error);
        }
        assert!(errors[4] < 1e-4, "{errors:?}");
        assert!(errors[4] < 0.002 * errors[0], "{errors:?}");
    }
}

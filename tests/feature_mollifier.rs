use glam::{DQuat, DVec3};
use marvelous_cloth::{
    math::{Jet, Scalar},
    ogc::{
        feature_mollifier as m,
        potential::activation,
        smooth_forest::{Link, evaluate},
    },
};

#[test]
fn support_and_parallel_limits_match_feature_hierarchy() {
    let edge = [[0., 0.001, 0.], [0., 0., 0.], [0.02, 0., 0.]];
    assert_eq!(m::vertex_edge(edge, 0.01).unwrap(), 0.);
    let mut inside = edge;
    inside[0][0] = 0.001;
    assert_eq!(m::vertex_edge(inside, 0.01).unwrap(), 1.);
    let face = [
        [0., 0.001, 0.],
        [-0.02, 0., 0.],
        [0.02, 0., 0.],
        [0., 0., 0.02],
    ];
    assert_eq!(m::vertex_face(face, 0.01).unwrap(), 0.);
    let mut inside = face;
    inside[0][2] = 0.001;
    assert_eq!(m::vertex_face(inside, 0.01).unwrap(), 1.);
    let crossing = [
        [-0.01, 0.001, 0.],
        [0.01, 0.001, 0.],
        [0., 0., -0.02],
        [0., 0., 0.02],
    ];
    assert_eq!(m::edge_edge(crossing, 0.01).unwrap(), 1.);
    let parallel = [
        [-0.01, 0.001, 0.],
        [0.01, 0.001, 0.],
        [-0.02, 0., 0.],
        [0.02, 0., 0.],
    ];
    assert_eq!(m::edge_edge(parallel, 0.01).unwrap(), 0.);
    assert!(m::vertex_edge(edge, 0.).is_err());
    assert!(m::vertex_edge([edge[0], edge[1], edge[1]], 0.01).is_err());
    let mut intersection = crossing;
    intersection[0][1] = 0.;
    intersection[1][1] = 0.;
    assert!(m::edge_edge(intersection, 0.01).is_err());
}

#[test]
fn point_feature_weights_differentiate_target_geometry_too() {
    for face in [false, true] {
        let base = [
            [0.00008, 0.001, 0.00005],
            [0., 0., 0.],
            [0.02, 0., 0.],
            [0., 0., 0.02],
        ];
        let count = if face { 4 } else { 3 };
        for moving in 0..count {
            let sample = |points: [[f64; 3]; 4]| {
                let x = std::array::from_fn(|i| {
                    std::array::from_fn(|a| {
                        if i == moving {
                            Jet::<3>::variable(points[i][a], a)
                        } else {
                            Jet::c(points[i][a])
                        }
                    })
                });
                if face {
                    m::vertex_face(x, 0.01).unwrap()
                } else {
                    m::vertex_edge([x[0], x[1], x[2]], 0.01).unwrap()
                }
            };
            let j = sample(base);
            assert!(j.v > 0. && j.v < 1.);
            for a in 0..3 {
                let eps = 1e-9;
                let mut plus = base;
                let mut minus = base;
                plus[moving][a] += eps;
                minus[moving][a] -= eps;
                let jp = sample(plus);
                let jm = sample(minus);
                let fd = (jp.v - jm.v) / (2. * eps);
                assert!((fd - j.g[a]).abs() < 2e-3 + 2e-5 * fd.abs());
                for b in 0..3 {
                    let fd = (jp.g[b] - jm.g[b]) / (2. * eps);
                    assert!((fd - j.h[b][a]).abs() < 5. + 5e-4 * fd.abs());
                }
            }
        }
    }
}

#[test]
fn boundary_and_parallel_approaches_have_vanishing_first_derivatives() {
    let c = Jet::<1>::c;
    let ve = |x| {
        m::vertex_edge(
            [
                [Jet::variable(x, 0), c(0.001), c(0.)],
                [c(0.); 3],
                [c(0.02), c(0.), c(0.)],
            ],
            0.01,
        )
        .unwrap()
    };
    assert_eq!(ve(0.).g[0], 0.);
    assert_eq!(ve(-0.0001).v, 0.);
    let mut previous = f64::INFINITY;
    for x in [1e-6, 1e-7, 1e-8, 1e-9] {
        let j = ve(x);
        assert!(j.g[0].abs() < previous * 0.12);
        previous = j.g[0].abs();
    }
    let ee = |slope| {
        let t = Jet::variable(slope, 0);
        m::edge_edge(
            [
                [c(-0.01), c(0.001), c(-0.01) * t],
                [c(0.01), c(0.001), c(0.01) * t],
                [c(-0.02), c(0.), c(0.)],
                [c(0.02), c(0.), c(0.)],
            ],
            0.01,
        )
        .unwrap()
    };
    assert_eq!(ee(0.).v, 0.);
    assert_eq!(ee(0.).g[0], 0.);
    let mut previous = f64::INFINITY;
    for t in [1e-3, 1e-4, 1e-5, 1e-6] {
        let j = ee(t);
        assert!(j.g[0].abs() < previous * 0.01);
        assert!((j.v - ee(-t).v).abs() < 1e-12);
        previous = j.g[0].abs();
    }
}

#[test]
fn edge_weight_derivatives_include_all_vertices_and_are_objective() {
    let base = [
        DVec3::new(-0.01, 0.001, -0.00008),
        DVec3::new(0.01, 0.001, 0.00008),
        DVec3::new(-0.02, 0., 0.),
        DVec3::new(0.02, 0., 0.),
    ];
    let mut force = DVec3::ZERO;
    let mut torque = DVec3::ZERO;
    for moving in 0..4 {
        let sample = |points: [DVec3; 4]| {
            let x = std::array::from_fn(|i| {
                std::array::from_fn(|a| {
                    if i == moving {
                        Jet::<3>::variable(points[i][a], a)
                    } else {
                        Jet::c(points[i][a])
                    }
                })
            });
            m::edge_edge(x, 0.01).unwrap()
        };
        let j = sample(base);
        let g = DVec3::from_array(j.g);
        force += g;
        torque += base[moving].cross(g);
        for a in 0..3 {
            let eps = 1e-9;
            let mut plus = base;
            let mut minus = base;
            plus[moving][a] += eps;
            minus[moving][a] -= eps;
            let jp = sample(plus);
            let jm = sample(minus);
            let fd = (jp.v - jm.v) / (2. * eps);
            assert!((fd - j.g[a]).abs() < 2e-3 + 2e-5 * fd.abs());
            for b in 0..3 {
                let fd = (jp.g[b] - jm.g[b]) / (2. * eps);
                assert!((fd - j.h[b][a]).abs() < 5. + 5e-4 * fd.abs());
            }
        }
    }
    assert!(force.length() < 1e-7 && torque.length() < 1e-9);
    let rotation = DQuat::from_axis_angle(DVec3::new(1., 2., 3.).normalize(), 0.73);
    let reference = m::edge_edge(base.map(|p| p.to_array()), 0.01).unwrap();
    for scale in [0.1, 1., 10.] {
        let x = base.map(|p| (rotation * p * scale + DVec3::new(0.2, -0.3, 0.1)).to_array());
        assert!((m::edge_edge(x, 0.01).unwrap() - reference).abs() < 1e-9);
    }
}

#[test]
fn product_weighted_forest_is_positive_but_fails_duplicate_force_gate() {
    // Two incident collinear targets at their common feature have A=B=C.
    // Both are weighted by m. A consistent representative would be m*b;
    // product-link weighting tends instead to (2m-m²)*b as tau tends to zero.
    let b = activation(Jet::<1>::variable(0.001, 0), 0.003, 100_000.);
    for weight in [0.25, 0.5, 0.75] {
        let c = Jet::c;
        let actual = evaluate(
            &[c(weight) * b; 2],
            &[Link {
                nodes: [0, 1],
                barrier: c(weight * weight) * b,
            }],
            1e-11,
        )
        .unwrap()
        .energy;
        let expected = c(2. * weight - weight * weight) * b;
        assert!(actual.v >= weight * b.v);
        assert!((actual.v - expected.v).abs() < 1e-10);
        assert!((actual.g[0] - expected.g[0]).abs() < 1e-7);
        assert!(-actual.g[0] > 1.2 * (-weight * b.g[0]));
    }
}

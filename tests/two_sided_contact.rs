use glam::{DQuat, DVec3};
use marvelous_cloth::{
    math::Jet,
    ogc::{
        feature_hierarchy::Pair,
        topology::Topology,
        two_sided_contact::{Feature, MinimumContact, Settings, smooth_minimum},
    },
};

fn settings() -> Settings {
    Settings {
        radius: 0.003,
        stiffness: 100_000.,
        width: 0.01,
        alpha: 0.5,
    }
}
fn fixture() -> ([[f64; 3]; 4], Topology, Pair) {
    let x = [
        [0.005, 0.001, 0.0002],
        [0., 0., 0.],
        [0.02, 0., 0.],
        [0.003, 0., 0.02],
    ];
    let top = Topology::new(4, vec![[1, 2, 3]]).unwrap();
    let e = top.edges.iter().position(|e| *e == [1, 2]).unwrap();
    (x, top, Pair::VertexEdge([0, e]))
}
fn jet(x: [[f64; 3]; 4], top: &Topology, pair: Pair) -> Jet<12> {
    MinimumContact::new(top)
        .energy(
            pair,
            &std::array::from_fn::<_, 4, _>(|i| {
                std::array::from_fn(|a| Jet::variable(x[i][a], 3 * i + a))
            }),
            1.,
            settings(),
        )
        .unwrap()
}
#[test]
fn cumulative_cubic_is_monotone_and_c2_at_all_knots() {
    let alpha = 0.6;
    let mut last = 0.;
    for i in 0..1001 {
        let v = smooth_minimum(-1. + i as f64 * 0.0015, alpha);
        assert!((-1e-14..=1. + 1e-14).contains(&v) && v >= last - 1e-14);
        last = v;
    }
    for phi in [-alpha, -2. * alpha / 3., -alpha / 3., 0.] {
        let l = smooth_minimum(Jet::<1>::variable(phi - 1e-8, 0), alpha);
        let r = smooth_minimum(Jet::<1>::variable(phi + 1e-8, 0), alpha);
        assert!(
            (l.v - r.v).abs() < 1e-6
                && (l.g[0] - r.g[0]).abs() < 1e-5
                && (l.h[0][0] - r.h[0][0]).abs() < 1e-4
        );
    }
}
#[test]
fn directional_filter_accepts_both_sides_and_rejects_in_plane_neighbors() {
    let x = [[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]];
    let top = Topology::new(3, vec![[0, 1, 2]]).unwrap();
    let model = MinimumContact::new(&top);
    for sign in [-1., 1.] {
        assert_eq!(
            model
                .minimum_factor(Feature::Vertex(0), &x, [0., sign, 0.], 0.5)
                .unwrap(),
            1.
        );
        assert_eq!(
            model
                .minimum_factor(Feature::Edge(0), &x, [0., sign, 0.], 0.5)
                .unwrap(),
            1.
        );
    }
    assert_eq!(
        model
            .minimum_factor(Feature::Vertex(0), &x, [-1., 0., -1.], 0.5)
            .unwrap(),
        0.
    );
    assert_eq!(
        model
            .minimum_factor(Feature::Edge(0), &x, [0., 0., -1.], 0.5)
            .unwrap(),
        0.
    );
}
#[test]
fn full_ring_gradient_hessian_include_vertices_outside_pair() {
    let (x, top, pair) = fixture();
    let j = jet(x, &top, pair);
    // Vertex 3 does not belong to the vertex/edge pair, but tilts its tangent.
    assert!(j.g[10].abs() > 1e-3);
    let model = MinimumContact::new(&top);
    for k in 0..12 {
        let mut a = x;
        let mut b = x;
        let eps = 2e-8;
        a[k / 3][k % 3] -= eps;
        b[k / 3][k % 3] += eps;
        let fd = (model.energy(pair, &b, 1., settings()).unwrap()
            - model.energy(pair, &a, 1., settings()).unwrap())
            / (2. * eps);
        assert!(
            (fd - j.g[k]).abs() < 2e-5 * (1. + j.g[k].abs()),
            "gradient {k}: {fd} {}",
            j.g[k]
        );
        let ja = jet(a, &top, pair);
        let jb = jet(b, &top, pair);
        for r in 0..12 {
            let h = (jb.g[r] - ja.g[r]) / (2. * eps);
            assert!(
                (h - j.h[r][k]).abs() < 2e-4 * (1. + j.h[r][k].abs()),
                "H {r},{k}: {h} {}",
                j.h[r][k]
            );
        }
    }
    let net = (0..4)
        .map(|i| DVec3::from_array(std::array::from_fn(|a| j.g[3 * i + a])))
        .sum::<DVec3>();
    let torque = (0..4)
        .map(|i| {
            DVec3::from_array(x[i])
                .cross(DVec3::from_array(std::array::from_fn(|a| j.g[3 * i + a])))
        })
        .sum::<DVec3>();
    assert!(net.length() < 1e-9 && torque.length() < 1e-10);
}
#[test]
fn potential_is_objective_two_sided_and_face_orientation_independent() {
    let (x, top, pair) = fixture();
    let model = MinimumContact::new(&top);
    let e = model.energy(pair, &x, 1., settings()).unwrap();
    let q = DQuat::from_rotation_y(0.71) * DQuat::from_rotation_x(-0.3);
    let transformed = x.map(|v| (q * DVec3::from_array(v) + DVec3::new(0.3, -0.2, 0.1)).to_array());
    assert!((e - model.energy(pair, &transformed, 1., settings()).unwrap()).abs() < 1e-11);
    let reflected = x.map(|[a, b, c]| [a, -b, c]);
    assert!((e - model.energy(pair, &reflected, 1., settings()).unwrap()).abs() < 1e-12);
    let reversed = Topology::new(4, vec![[3, 2, 1]]).unwrap();
    let edge = reversed.edges.iter().position(|e| *e == [1, 2]).unwrap();
    assert!(
        (e - MinimumContact::new(&reversed)
            .energy(Pair::VertexEdge([0, edge]), &x, 1., settings())
            .unwrap())
        .abs()
            < 1e-12
    );
}
#[test]
fn active_vertex_face_edge_edge_and_vertex_vertex_terms_are_finite() {
    let x = vec![
        [-0.01, 0.001, 0.],
        [0.01, 0.001, 0.],
        [0., 0.02, 0.01],
        [0., 0., -0.01],
        [0., 0., 0.01],
        [0.02, -0.02, 0.],
    ];
    let top = Topology::new(6, vec![[0, 1, 2], [3, 4, 5]]).unwrap();
    let model = MinimumContact::new(&top);
    let a = top.edges.iter().position(|e| *e == [0, 1]).unwrap();
    let b = top.edges.iter().position(|e| *e == [3, 4]).unwrap();
    let energy = model
        .energy(Pair::EdgeEdge([a, b]), &x, 1., settings())
        .unwrap();
    assert!(energy > 0. && energy.is_finite());
    assert_eq!(
        energy,
        model
            .energy(Pair::EdgeEdge([b, a]), &x, 1., settings())
            .unwrap()
    );
    let mut x = x;
    x[0] = [0.002, 0.001, 0.];
    let vf = model
        .energy(Pair::VertexFace([0, 1]), &x, 1., settings())
        .unwrap();
    assert!(vf.is_finite() && vf >= 0.);
    let empty = Topology::new(2, vec![]).unwrap();
    let m = MinimumContact::new(&empty);
    assert!(
        m.energy(
            Pair::VertexVertex([0, 1]),
            &[[0., 0., 0.], [0., 0.001, 0.]],
            1.,
            settings()
        )
        .unwrap()
            > 0.
    );
    assert!(
        m.energy(Pair::VertexVertex([0, 1]), &[[0.; 3]; 2], 1., settings())
            .is_err()
    );
}

#[test]
fn edge_pair_derivatives_include_both_opposite_vertices() {
    let x = [
        [-0.01, 0.001, 0.],
        [0.01, 0.001, 0.],
        [0., -0.003, 0.02],
        [0., 0., -0.01],
        [0., 0., 0.01],
        [0.02, 0.004, 0.],
    ];
    let top = Topology::new(6, vec![[0, 1, 2], [3, 4, 5]]).unwrap();
    let a = top.edges.iter().position(|e| *e == [0, 1]).unwrap();
    let b = top.edges.iter().position(|e| *e == [3, 4]).unwrap();
    let pair = Pair::EdgeEdge([a, b]);
    let model = MinimumContact::new(&top);
    let evaluate = |x: [[f64; 3]; 6]| {
        model
            .energy(
                pair,
                &std::array::from_fn::<_, 6, _>(|i| {
                    std::array::from_fn(|a| Jet::<18>::variable(x[i][a], 3 * i + a))
                }),
                1.,
                settings(),
            )
            .unwrap()
    };
    let j = evaluate(x);
    assert!(j.g[7].abs() > 1e-3 && j.g[16].abs() > 1e-3);
    for k in 0..18 {
        let mut a = x;
        let mut b = x;
        let eps = 1e-8;
        a[k / 3][k % 3] -= eps;
        b[k / 3][k % 3] += eps;
        let ja = evaluate(a);
        let jb = evaluate(b);
        let fd = (jb.v - ja.v) / (2. * eps);
        assert!(
            (fd - j.g[k]).abs() < 2e-5 * (1. + j.g[k].abs()),
            "gradient {k}"
        );
        for r in 0..18 {
            let h = (jb.g[r] - ja.g[r]) / (2. * eps);
            assert!(
                (h - j.h[r][k]).abs() < 2e-4 * (1. + j.h[r][k].abs()),
                "H {r},{k}: {h} {}",
                j.h[r][k]
            );
        }
    }
}

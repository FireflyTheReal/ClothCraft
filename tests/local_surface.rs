use glam::{DQuat, DVec3};
use marvelous_cloth::{
    math::{Jet, Scalar},
    ogc::local_surface::{self, Sample},
};

#[test]
fn coplanar_field_reproduces_plane_with_unequal_samples_and_support_changes() {
    let samples = [
        Sample::oriented([-0.033, 0., 0.033], [0., 1., 0.], 0.003).unwrap(),
        Sample::oriented([0.033, 0., -0.033], [0., 1., 0.], 7.).unwrap(),
        Sample::oriented([0., 0., 0.], [0., 1., 0.], 1.).unwrap(),
    ];
    // Query derivatives, with a fresh generic sample representation.
    for support in [0.05, 0.08, 0.2] {
        for i in 0..81 {
            let p = [-0.04 + i as f64 * 0.001, 0.001, 0.];
            let f = local_surface::evaluate(p, &samples, support).unwrap();
            assert_eq!(f.value, 0.001);
            let js = [
                Sample::oriented(
                    [-0.033, 0., 0.033].map(Jet::c),
                    [0., 1., 0.].map(Jet::c),
                    0.003,
                )
                .unwrap(),
                Sample::oriented(
                    [0.033, 0., -0.033].map(Jet::c),
                    [0., 1., 0.].map(Jet::c),
                    7.,
                )
                .unwrap(),
                Sample::oriented([0., 0., 0.].map(Jet::c), [0., 1., 0.].map(Jet::c), 1.).unwrap(),
            ];
            let j = local_surface::evaluate(
                std::array::from_fn(|a| Jet::<3>::variable(p[a], a)),
                &js,
                support,
            )
            .unwrap()
            .value;
            assert_eq!(j.g, [0., 1., 0.]);
            assert_eq!(j.h, [[0.; 3]; 3]);
        }
    }
}

fn curved_energy<S: Scalar>(x: [[S; 3]; 5]) -> S {
    let samples = [
        Sample::triangle([x[1], x[2], x[3]], 0.7).unwrap(),
        Sample::triangle([x[2], x[1], x[4]], 1.3).unwrap(),
    ];
    local_surface::energy(x[0], &samples, 0.1, 1., 0.03, 1000.).unwrap()
}
#[test]
fn all_geometry_derivatives_are_consistent_and_objective() {
    let x = [
        [0.004, 0.015, 0.002],
        [-0.03, 0., -0.03],
        [0.03, 0., 0.03],
        [0.03, 0.002, -0.03],
        [-0.03, -0.001, 0.03],
    ];
    let jet = |x: [[f64; 3]; 5]| {
        curved_energy(std::array::from_fn(|i| {
            std::array::from_fn(|a| Jet::<15>::variable(x[i][a], 3 * i + a))
        }))
    };
    let j = jet(x);
    for k in 0..15 {
        let mut a = x;
        let mut b = x;
        let eps = 2e-8;
        a[k / 3][k % 3] -= eps;
        b[k / 3][k % 3] += eps;
        let ja = jet(a);
        let jb = jet(b);
        assert!(((jb.v - ja.v) / (2. * eps) - j.g[k]).abs() < 1e-5 * (1. + j.g[k].abs()));
        for r in 0..15 {
            let h = (jb.g[r] - ja.g[r]) / (2. * eps);
            assert!(
                (h - j.h[r][k]).abs() < 1e-4 * (1. + j.h[r][k].abs()),
                "H {r} {k}"
            );
        }
    }
    let net = (0..5)
        .map(|i| DVec3::from_array(std::array::from_fn(|a| j.g[3 * i + a])))
        .sum::<DVec3>();
    let torque = (0..5)
        .map(|i| {
            DVec3::from_array(x[i])
                .cross(DVec3::from_array(std::array::from_fn(|a| j.g[3 * i + a])))
        })
        .sum::<DVec3>();
    assert!(net.length() < 1e-10 && torque.length() < 1e-10);
    let q = DQuat::from_rotation_y(0.43) * DQuat::from_rotation_x(-0.9);
    let y = x.map(|p| (q * DVec3::from_array(p) + DVec3::new(0.3, -0.2, 0.4)).to_array());
    assert!((curved_energy(x) - curved_energy(y)).abs() < 1e-12);
}

#[test]
fn changing_support_is_c2_while_another_sample_remains() {
    let r = 0.1;
    let samples = [
        Sample::oriented([0., 0., 0.].map(Jet::c), [0., 1., 0.].map(Jet::c), 1.).unwrap(),
        Sample::oriented([r, 0., 0.].map(Jet::c), [0.2, 1., 0.].map(Jet::c), 1.).unwrap(),
    ];
    let mut previous = f64::INFINITY;
    for eps in [1e-4, 1e-5, 1e-6, 1e-7] {
        let a = local_surface::evaluate(
            [Jet::<1>::variable(-eps, 0), Jet::c(0.), Jet::c(0.)],
            &samples,
            r,
        )
        .unwrap()
        .value;
        let b = local_surface::evaluate(
            [Jet::<1>::variable(eps, 0), Jet::c(0.), Jet::c(0.)],
            &samples,
            r,
        )
        .unwrap()
        .value;
        let err = (a.v - b.v).abs() + (a.g[0] - b.g[0]).abs() + (a.h[0][0] - b.h[0][0]).abs();
        assert!(err < previous * 0.2);
        previous = err;
    }
}
#[test]
fn missing_support_and_crossed_side_are_explicit_errors() {
    let s = [Sample::oriented([0., 0., 0.], [0., 1., 0.], 1.).unwrap()];
    assert!(local_surface::evaluate([0., 0., 0.], &s, 0.).is_err());
    assert!(local_surface::evaluate([1., 0., 0.], &s, 0.1).is_err());
    assert!(local_surface::evaluate([0., 0., 0.], &s, 1e-300).is_err());
    assert!(local_surface::evaluate([0., 0., 0.], &[], 0.1).is_err());
    assert!(local_surface::energy([0., -0.001, 0.], &s, 0.1, 1., 0.003, 100_000.).is_err());
    assert!(local_surface::energy([0., 0.001, 0.], &s, 0.1, 0., 0.003, 100_000.).is_err());
    assert!(Sample::oriented([0., 0., 0.], [0., 0., 0.], 1.).is_err());
}
#[test]
fn aligned_layers_invent_a_mid_surface_even_with_perfect_normal_coherence() {
    let samples = [
        Sample::oriented([0., 0., 0.], [0., 1., 0.], 1.).unwrap(),
        Sample::oriented([0., 0.002, 0.], [0., 1., 0.], 1.).unwrap(),
    ];
    let middle = local_surface::evaluate([0., 0.001, 0.], &samples, 0.1).unwrap();
    assert!(middle.value.abs() < 1e-15 && (middle.normal_coherence - 1.).abs() < 1e-15);
    let on_real_surface = local_surface::evaluate([0., 0., 0.], &samples, 0.1).unwrap();
    assert!(on_real_surface.value.abs() > 0.0009);
    // The reconstructed barrier is finite on a real triangle's plane:
    // it cannot replace the original mesh's collision guard.
    assert!(
        local_surface::energy([0., 0., 0.], &samples, 0.1, -1., 0.003, 100_000.)
            .unwrap()
            .is_finite()
    );
}
#[test]
fn opposite_layers_can_cancel_the_contact_gradient() {
    let samples = [
        Sample::oriented([0., 0., 0.].map(Jet::c), [0., 1., 0.].map(Jet::c), 1.).unwrap(),
        Sample::oriented([0., 0.002, 0.].map(Jet::c), [0., -1., 0.].map(Jet::c), 1.).unwrap(),
    ];
    let f = local_surface::evaluate(
        [
            Jet::<3>::variable(0., 0),
            Jet::variable(0.001, 1),
            Jet::variable(0., 2),
        ],
        &samples,
        0.1,
    )
    .unwrap();
    assert!((f.value.v - 0.001).abs() < 1e-15);
    assert!(f.normal_coherence < 1e-15);
    assert!(f.value.g.iter().all(|g| g.abs() < 1e-12));
}
#[test]
fn known_separate_layers_match_the_sum_of_two_plane_barriers() {
    use marvelous_cloth::ogc::potential::activation;
    let lower = [Sample::oriented([0., 0., 0.].map(Jet::c), [0., 1., 0.].map(Jet::c), 1.).unwrap()];
    let upper =
        [Sample::oriented([0., 0.002, 0.].map(Jet::c), [0., 1., 0.].map(Jet::c), 1.).unwrap()];
    for h in [0.0002, 0.0007, 0.001, 0.0018] {
        let y = Jet::<1>::variable(h, 0);
        let p = [Jet::c(0.), y, Jet::c(0.)];
        let e = local_surface::energy(p, &lower, 0.1, 1., 0.003, 100_000.).unwrap()
            + local_surface::energy(p, &upper, 0.1, -1., 0.003, 100_000.).unwrap();
        let exact = activation(y, 0.003, 100_000.) + activation(Jet::c(0.002) - y, 0.003, 100_000.);
        assert!(
            (e.v - exact.v).abs() < 1e-12
                && (e.g[0] - exact.g[0]).abs() < 1e-9
                && (e.h[0][0] - exact.h[0][0]).abs() < 1e-5
        );
    }
}

#[test]
fn common_quadrature_scaling_does_not_corrupt_derivatives() {
    let field = |scale| {
        let samples = [
            Sample::oriented([0., 0., 0.].map(Jet::c), [0., 1., 0.].map(Jet::c), scale).unwrap(),
            Sample::oriented(
                [0.01, 0.001, 0.].map(Jet::c),
                [0.2, 1., 0.].map(Jet::c),
                2. * scale,
            )
            .unwrap(),
        ];
        local_surface::evaluate(
            [
                Jet::<3>::variable(0.002, 0),
                Jet::variable(0.003, 1),
                Jet::variable(0.001, 2),
            ],
            &samples,
            0.05,
        )
        .unwrap()
        .value
    };
    let reference = field(1.);
    for scale in [1e-150, 1e-30, 1e30, 1e150] {
        let j = field(scale);
        assert!((j.v - reference.v).abs() < 1e-14);
        for i in 0..3 {
            assert!(j.g[i].is_finite() && (j.g[i] - reference.g[i]).abs() < 1e-10);
            for k in 0..3 {
                assert!(j.h[i][k].is_finite() && (j.h[i][k] - reference.h[i][k]).abs() < 1e-8);
            }
        }
    }
}

#[test]
fn concave_patch_field_does_not_preserve_the_triangle_zero_set() {
    let samples = [
        Sample::oriented([-0.05, 0.01, 0.], [0.2, 1., 0.], 1.).unwrap(),
        Sample::oriented([0.05, 0.01, 0.], [-0.2, 1., 0.], 1.).unwrap(),
    ];
    let p = [0.01, 0.002, 0.];
    let f = local_surface::evaluate(p, &samples, 0.2).unwrap();
    // p lies exactly on the right-hand plane y=0.2*x. A field barrier
    // alone stays finite there, so it cannot protect that original geometry.
    assert!(f.value > 0.001 && f.normal_coherence > 0.9);
    assert!(
        local_surface::energy(p, &samples, 0.2, 1., 0.003, 100_000.)
            .unwrap()
            .is_finite()
    );
}

#[test]
fn snapshot_patch_eligibility_preserves_original_seam_exclusions() {
    use marvelous_cloth::ogc::OgcWorld;
    let x = [
        DVec3::Y * 0.001,
        DVec3::ZERO,
        DVec3::X * 0.02,
        DVec3::Z * 0.02,
        DVec3::Y * 0.01,
    ];
    let world = OgcWorld::new(&x, vec![[1, 2, 3]], 1, []).unwrap();
    assert!(world.vertex_face_eligible(0, 0).unwrap());
    assert!(!world.vertex_face_eligible(1, 0).unwrap());
    assert!(!world.vertex_face_eligible(4, 0).unwrap());
    assert!(world.vertex_face_eligible(0, 1).is_err());
    let excluded = OgcWorld::new(&x, vec![[1, 2, 3]], 1, [[0, 3]]).unwrap();
    assert!(!excluded.vertex_face_eligible(0, 0).unwrap());
}

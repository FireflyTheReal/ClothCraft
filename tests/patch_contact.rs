use glam::{DQuat, DVec3};
use marvelous_cloth::{
    math::{Jet, Scalar},
    ogc::{
        patch_contact::{face_pair, wire_pair},
        potential::{Contact, Kind, activation},
    },
};

fn energy<S: Scalar>(wire: bool, x: [[S; 3]; 5]) -> S {
    if wire {
        wire_pair(x, 0.003, 100_000.)
    } else {
        face_pair(x, 0.003, 100_000.)
    }
}
fn jet(wire: bool, x: [DVec3; 5]) -> Jet<15> {
    energy(
        wire,
        std::array::from_fn(|i| std::array::from_fn(|a| Jet::variable(x[i][a], 3 * i + a))),
    )
}
fn fixture(wire: bool, t: f64, fold: f64) -> [DVec3; 5] {
    if wire {
        [
            DVec3::new(t, 0.001, -0.01),
            DVec3::new(t, 0.001, 0.01),
            DVec3::new(-0.02, 0., 0.),
            DVec3::ZERO,
            DVec3::new(0.02, fold, 0.),
        ]
    } else {
        [
            DVec3::new(t, 0.001, 0.0003),
            DVec3::new(-0.02, 0., -0.02),
            DVec3::new(0.02, 0., 0.02),
            DVec3::new(0.02, 0., -0.02),
            DVec3::new(-0.02, fold, 0.02),
        ]
    }
}

#[test]
fn patch_energy_has_no_planar_seam_force_for_faces_or_transverse_wire() {
    let reference = activation(0.001, 0.003, 100_000.);
    for wire in [false, true] {
        for i in 0..161 {
            let x = fixture(wire, -0.004 + i as f64 * 0.00005, 0.);
            let j = jet(wire, x);
            assert!((j.v - reference).abs() < 1e-12);
            // Translate both source endpoints for the wire.
            let tangential = if wire { j.g[0] + j.g[3] } else { j.g[0] };
            assert!(tangential.abs() < 1e-9, "{wire} {i}: {tangential}");
        }
    }
}

#[test]
fn exact_shared_feature_does_not_create_tangential_curvature_on_a_flat_patch() {
    for wire in [false, true] {
        // Centre of the shared feature, exactly representable.
        let mut x = fixture(wire, 0., 0.);
        if !wire {
            x[0].z = 0.;
        }
        let j = jet(wire, x);
        let curvature = if wire {
            j.h[0][0] + j.h[0][3] + j.h[3][0] + j.h[3][3]
        } else {
            j.h[0][0]
        };
        assert!(curvature.abs() < 1e-7, "{wire}: {curvature}");
    }
}

#[test]
fn patch_derivatives_include_all_five_vertices_and_are_objective() {
    let q = DQuat::from_axis_angle(DVec3::new(1., 2., 3.).normalize(), 0.7);
    for wire in [false, true] {
        for fold in [-0.004, 0., 0.004] {
            for t in [-0.00131, 0.00047, 0.00143] {
                let x = fixture(wire, t, fold);
                let j = jet(wire, x);
                let mut net = DVec3::ZERO;
                let mut torque = DVec3::ZERO;
                for i in 0..5 {
                    let g = DVec3::new(j.g[3 * i], j.g[3 * i + 1], j.g[3 * i + 2]);
                    net += g;
                    torque += x[i].cross(g);
                    for a in 0..3 {
                        let index = 3 * i + a;
                        let eps = 1e-8;
                        let mut plus = x;
                        let mut minus = x;
                        plus[i][a] += eps;
                        minus[i][a] -= eps;
                        let jp = jet(wire, plus);
                        let jm = jet(wire, minus);
                        let fd = (jp.v - jm.v) / (2. * eps);
                        assert!((fd - j.g[index]).abs() < 1e-5 * (1. + fd.abs()));
                        for b in 0..15 {
                            let fd = (jp.g[b] - jm.g[b]) / (2. * eps);
                            assert!(
                                (fd - j.h[b][index]).abs() < 0.02 + 2e-5 * fd.abs(),
                                "{wire} {fold} {t} ({b},{index}): {fd} vs {}",
                                j.h[b][index]
                            );
                        }
                    }
                }
                assert!(net.length() < 1e-8 && torque.length() < 1e-9);
                let rotated = x.map(|p| q * p + DVec3::new(0.3, -0.2, 0.1));
                let jr = jet(wire, rotated);
                assert!((jr.v - j.v).abs() < 1e-10);
                for i in 0..5 {
                    let g = DVec3::new(j.g[3 * i], j.g[3 * i + 1], j.g[3 * i + 2]);
                    let gr = DVec3::new(jr.g[3 * i], jr.g[3 * i + 1], jr.g[3 * i + 2]);
                    assert!((gr - q * g).length() < 1e-7);
                }
            }
        }
    }
}

#[test]
fn each_patch_dominates_both_primitive_barriers_without_clamping() {
    for wire in [false, true] {
        for f in -10..=10 {
            for i in 0..81 {
                let fold = f as f64 * 0.001;
                let mut x = fixture(wire, -0.004 + i as f64 * 0.0001, fold);
                // Keep the probe strictly above the entire folded patch.
                // Intersections make the barrier infinite, not a finite test.
                x[0].y += fold.max(0.);
                if wire {
                    x[1].y += fold.max(0.);
                }
                let e = energy(wire, x.map(|p| p.to_array()));
                let stencils = if wire {
                    [[0, 1, 2, 3], [0, 1, 3, 4]]
                } else {
                    [[0, 1, 2, 3], [0, 2, 1, 4]]
                };
                let mut raw_sum = 0.;
                for vertices in stencils {
                    let c = Contact {
                        kind: if wire {
                            Kind::SegmentSegment
                        } else {
                            Kind::PointTriangle
                        },
                        vertices,
                        count: 4,
                        // Tiny threshold: all transverse fixture edges have
                        // unit mollification, matching this unmollified kernel.
                        mollifier_epsilon: 1e-30,
                    };
                    let primitive = c.value(&x, 0.003, 100_000.);
                    raw_sum += primitive;
                    assert!(e + 1e-9 >= primitive);
                }
                let (p, a, b) = if wire {
                    (x[3], x[0], x[1])
                } else {
                    (x[0], x[1], x[2])
                };
                let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0., 1.);
                let shared = activation(p.distance(a.lerp(b, t)), 0.003, 100_000.);
                assert!((e - (raw_sum - shared)).abs() < 1e-10);
                assert!(e >= -1e-12 && e.is_finite());
            }
        }
    }
}

#[test]
fn edge_endpoint_crossing_is_continuous_in_value_and_first_derivative() {
    // Compare one-sided limits, not Hessians at a feature switch.
    for wire in [false, true] {
        let crossing = if wire { 0. } else { 0.0003 };
        let mut previous = f64::INFINITY;
        for eps in [1e-5, 1e-6, 1e-7, 1e-8] {
            let left = jet(wire, fixture(wire, crossing - eps, -0.003));
            let right = jet(wire, fixture(wire, crossing + eps, -0.003));
            let error = (left.v - right.v).abs()
                + left
                    .g
                    .iter()
                    .zip(right.g)
                    .map(|(a, b)| (a - b).abs())
                    .sum::<f64>();
            assert!(
                error < previous * 0.2,
                "{wire} {eps}: {error} >= {previous}"
            );
            previous = error;
        }
    }
}

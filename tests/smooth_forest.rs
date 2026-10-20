use glam::{DQuat, DVec3};
use marvelous_cloth::{
    math::{Jet, Scalar},
    ogc::smooth_forest::{Link, evaluate, wire},
};

// Independent exhaustive rooted-forest oracle; no matrix formulas.
fn enumerate(nodes: &[f64], links: &[Link<f64>], tau: f64) -> (f64, Vec<f64>) {
    let mut z = 0.;
    let mut total = 0.;
    let mut probabilities = vec![0.; links.len()];
    for mask in 0..1usize << links.len() {
        let mut labels: Vec<_> = (0..nodes.len()).collect();
        let mut weight = 1.;
        let mut correction = 0.;
        let mut cycle = false;
        for (i, link) in links.iter().enumerate() {
            if mask & (1 << i) == 0 {
                continue;
            }
            let a = labels[link.nodes[0]];
            let b = labels[link.nodes[1]];
            if a == b {
                cycle = true;
                break;
            }
            for label in &mut labels {
                if *label == b {
                    *label = a;
                }
            }
            weight *= link.barrier / tau;
            correction += link.barrier;
        }
        if cycle {
            continue;
        }
        for i in 0..nodes.len() {
            let size = labels.iter().filter(|l| **l == i).count();
            if size > 0 {
                weight *= size as f64;
            }
        }
        z += weight;
        total += weight * (nodes.iter().sum::<f64>() - correction);
        for (i, p) in probabilities.iter_mut().enumerate() {
            if mask & (1 << i) != 0 {
                *p += weight;
            }
        }
    }
    (
        total / z,
        probabilities.into_iter().map(|p| p / z).collect(),
    )
}

#[test]
fn matrix_probabilities_match_rooted_forest_enumeration() {
    for n in 2..=5 {
        for sparse in [false, true] {
            let nodes = vec![0.3; n];
            let mut links = Vec::new();
            for i in 0..n {
                for j in i + 1..n {
                    links.push(Link {
                        nodes: [i, j],
                        barrier: if sparse && (i + j) % 2 == 1 {
                            0.
                        } else {
                            0.05 * (1 + (i + 2 * j) % 3) as f64
                        },
                    });
                }
            }
            for tau in [0.5, 0.01, 1e-5, 1e-10] {
                let expected = enumerate(&nodes, &links, tau);
                let actual = evaluate(&nodes, &links, tau).unwrap();
                assert!(
                    (actual.energy - expected.0).abs() < 1e-8,
                    "{n} {sparse} {tau}: {} vs {}",
                    actual.energy,
                    expected.0
                );
                assert!(actual.energy >= 0.3 - 1e-8);
                for (a, b) in actual.inclusion_probabilities.iter().zip(expected.1) {
                    assert!((*a - b).abs() < 1e-8, "{n} {sparse} {tau}: {a} vs {b}");
                }
            }
        }
    }
}

fn parameter_energy<S: Scalar>(x: [S; 3]) -> S {
    let links = [
        Link {
            nodes: [0, 1],
            barrier: S::c(0.1) + S::c(0.01) * x[0],
        },
        Link {
            nodes: [1, 2],
            barrier: S::c(0.12) + S::c(0.02) * x[1],
        },
        Link {
            nodes: [0, 2],
            barrier: S::c(0.13) + S::c(0.03) * x[2],
        },
        Link {
            nodes: [2, 3],
            barrier: S::c(0.04) + S::c(0.001) * x[0] * x[1],
        },
    ];
    evaluate(&[S::c(0.5); 4], &links, 0.001).unwrap().energy
}

#[test]
fn differentiates_probabilities_and_cholesky_including_zero_links() {
    let x = [-0.1, 0.2, 0.3];
    let j = parameter_energy(std::array::from_fn(|i| Jet::<3>::variable(x[i], i)));
    for a in 0..3 {
        let eps = 1e-4;
        let mut plus = x;
        let mut minus = x;
        plus[a] += eps;
        minus[a] -= eps;
        let jp = parameter_energy(std::array::from_fn(|i| Jet::<3>::variable(plus[i], i)));
        let jm = parameter_energy(std::array::from_fn(|i| Jet::<3>::variable(minus[i], i)));
        assert!(((jp.v - jm.v) / (2. * eps) - j.g[a]).abs() < 1e-8);
        for b in 0..3 {
            assert!(((jp.g[b] - jm.g[b]) / (2. * eps) - j.h[b][a]).abs() < 1e-8);
        }
    }
    let tau = 0.01;
    let zero = evaluate(
        &[Jet::<1>::c(0.3); 2],
        &[Link {
            nodes: [0, 1],
            barrier: Jet::variable(0., 0),
        }],
        tau,
    )
    .unwrap()
    .energy;
    assert!((zero.v - 0.6).abs() < 1e-12 && zero.g[0].abs() < 1e-12);
    assert!((zero.h[0][0] + 4. / tau).abs() < 1e-8);
}

fn fixture(shift: f64) -> [DVec3; 5] {
    [
        DVec3::new(shift, 0.001, -0.00025),
        DVec3::new(shift, 0.001, -0.00015),
        DVec3::new(-0.001, 0., 0.),
        DVec3::new(0.001, 0., 0.),
        DVec3::new(0., 0., 0.0002),
    ]
}
fn geometry_energy<S: Scalar>(x: &[[S; 3]]) -> S {
    wire(x, [0, 1], &[[2, 3], [3, 4], [4, 2]], 0.003, 100_000., 1e-5)
        .unwrap()
        .energy
}

#[test]
fn full_geometry_derivatives_are_objective_and_force_cusp_is_removed() {
    let x = fixture(0.00013);
    let mut net = DVec3::ZERO;
    let mut torque = DVec3::ZERO;
    for i in 0..5 {
        let block = |points: [DVec3; 5]| {
            geometry_energy(&std::array::from_fn::<_, 5, _>(|v| {
                std::array::from_fn(|a| {
                    if v == i {
                        Jet::<3>::variable(points[v][a], a)
                    } else {
                        Jet::c(points[v][a])
                    }
                })
            }))
        };
        let j = block(x);
        let g = DVec3::from_array(j.g);
        net += g;
        torque += x[i].cross(g);
        for a in 0..3 {
            let eps = 1e-9;
            let mut plus = x;
            let mut minus = x;
            plus[i][a] += eps;
            minus[i][a] -= eps;
            let jp = block(plus);
            let jm = block(minus);
            let fd = (jp.v - jm.v) / (2. * eps);
            assert!((fd - j.g[a]).abs() < 1e-4 + 1e-5 * fd.abs());
            for b in 0..3 {
                let fd = (jp.g[b] - jm.g[b]) / (2. * eps);
                assert!((fd - j.h[b][a]).abs() < 0.1 + 1e-5 * fd.abs());
            }
        }
    }
    assert!(net.length() < 1e-7 && torque.length() < 1e-9);
    let rotation = DQuat::from_axis_angle(DVec3::new(1., 2., 3.).normalize(), 0.7);
    let transformed = x.map(|p| (rotation * p + DVec3::new(0.3, -0.2, 0.1)).to_array());
    assert!(
        (geometry_energy(&transformed) - geometry_energy(&x.map(|p| p.to_array()))).abs() < 1e-10
    );
    let middle = geometry_energy(&fixture(0.).map(|p| p.to_array()));
    let mut previous = f64::INFINITY;
    for eps in [1e-7, 1e-8, 1e-9] {
        let left = geometry_energy(&fixture(-eps).map(|p| p.to_array()));
        let right = geometry_energy(&fixture(eps).map(|p| p.to_array()));
        let jump = ((middle - left) - (right - middle)) / eps;
        eprintln!("smooth forest: h={eps:e} m; one-sided slope difference={jump:.9} N");
        assert!(jump.abs() < previous * 0.2 && jump.abs() < 0.1);
        previous = jump.abs();
    }
}

#[test]
fn inactive_nodes_and_zero_links_preserve_value_without_clamping() {
    let nodes = [0.2, 0.3, 0.4];
    let links = [
        Link {
            nodes: [0, 1],
            barrier: 0.1,
        },
        Link {
            nodes: [1, 2],
            barrier: 0.15,
        },
    ];
    for tau in [0.1, 1e-5, 1e-10] {
        let a = evaluate(&nodes, &links, tau).unwrap();
        let mut padded = nodes.to_vec();
        padded.push(0.);
        let mut extra = links.to_vec();
        extra.push(Link {
            nodes: [2, 3],
            barrier: 0.,
        });
        let b = evaluate(&padded, &extra, tau).unwrap();
        assert!((a.energy - b.energy).abs() < 1e-8);
        assert!(b.inclusion_probabilities.last().unwrap().abs() < 1e-12);
    }
    assert!(evaluate(&nodes, &links, 0.).is_err());
    assert!(
        evaluate(
            &nodes,
            &[Link {
                nodes: [0, 1],
                barrier: 0.8
            }],
            0.1
        )
        .is_err()
    );
    assert!(
        evaluate(
            &nodes,
            &[Link {
                nodes: [0, 8],
                barrier: 0.1
            }],
            0.1
        )
        .is_err()
    );
}

#[test]
fn flat_junction_cancels_shared_feature_before_second_derivatives() {
    use marvelous_cloth::ogc::potential::activation;
    for tau in [1e-3, 1e-5, 1e-7, 1e-9, 1e-11] {
        let sample = |offset: f64| {
            let p = [
                Jet::<3>::variable(offset, 0),
                Jet::variable(0.001, 1),
                Jet::variable(0., 2),
            ];
            let x = [
                [p[0], p[1], p[2] - Jet::c(0.01)],
                [p[0], p[1], p[2] + Jet::c(0.01)],
                [Jet::c(-0.1), Jet::c(0.), Jet::c(0.)],
                [Jet::c(0.), Jet::c(0.), Jet::c(0.)],
                [Jet::c(0.1), Jet::c(0.), Jet::c(0.)],
            ];
            let actual = wire(&x, [0, 1], &[[2, 3], [3, 4]], 0.003, 100_000., tau)
                .unwrap()
                .energy;
            // On this straight line the exact union barrier is independent
            // of x; only the explicit smooth-forest residual varies.
            let common = activation((p[0] * p[0] + p[1] * p[1]).sqrt(), 0.003, 100_000.);
            let exact = activation(p[1], 0.003, 100_000.)
                + common * Jet::c(tau) / (Jet::c(tau) + Jet::c(2.) * common);
            (actual, exact)
        };
        for offset in [-0.0029, -0.0001, 0., 0.0001, 0.0029] {
            let (actual, expected) = sample(offset);
            assert!((actual.v - expected.v).abs() < 1e-12);
            for a in 0..3 {
                assert!((actual.g[a] - expected.g[a]).abs() < 1e-8);
                for b in 0..3 {
                    assert!(
                        (actual.h[a][b] - expected.h[a][b]).abs() < 1e-6,
                        "tau={tau}, x={offset}, H[{a},{b}]={} expected {}",
                        actual.h[a][b],
                        expected.h[a][b]
                    );
                }
            }
        }
        let center = sample(0.).0;
        let eps = 1e-8;
        let fd = (sample(eps).0.g[0] - sample(-eps).0.g[0]) / (2. * eps);
        assert!((center.h[0][0] - fd).abs() < 1e-6);
    }
}

#[test]
fn dense_cycle_repels_and_is_independent_of_target_edge_order() {
    for tau in [1e-3, 1e-7, 1e-11] {
        for height in [0.0001, 0.001, 0.0029, 0.0031] {
            let sample = |height: f64, reverse: bool| {
                let y = Jet::<1>::variable(height, 0);
                let c = Jet::c;
                let x = [
                    [c(-0.01), y, c(0.)],
                    [c(0.01), y, c(0.0001)],
                    [c(-0.00001), c(0.), c(0.)],
                    [c(0.00001), c(0.), c(0.)],
                    [c(0.), c(0.00001), c(0.)],
                    [c(0.), c(0.), c(0.00001)],
                ];
                let mut targets = [[2, 3], [2, 4], [2, 5], [3, 4], [3, 5], [4, 5]];
                if reverse {
                    targets.reverse();
                    for edge in &mut targets {
                        edge.reverse();
                    }
                }
                wire(&x, [0, 1], &targets, 0.003, 100_000., tau)
                    .unwrap()
                    .energy
            };
            let j = sample(height, false);
            let permuted = sample(height, true);
            assert!(j.v >= 0. && j.g[0] <= 0.);
            assert!((j.v - permuted.v).abs() < 1e-10);
            assert!((j.g[0] - permuted.g[0]).abs() < 1e-6);
            let eps = 1e-9;
            let plus = sample(height + eps, false);
            let minus = sample(height - eps, false);
            let fd_g = (plus.v - minus.v) / (2. * eps);
            let fd_h = (plus.g[0] - minus.g[0]) / (2. * eps);
            assert!((fd_g - j.g[0]).abs() < 1e-3);
            assert!((fd_h - j.h[0][0]).abs() < 1. + 1e-5 * fd_h.abs());
        }
    }
}

#[test]
fn inactive_geometry_preserves_value_force_and_tie_hessian() {
    let c = Jet::<3>::c;
    let p = [
        Jet::variable(0., 0),
        Jet::variable(0.001, 1),
        Jet::variable(0., 2),
    ];
    let positions = [
        [p[0], p[1], p[2] - c(0.01)],
        [p[0], p[1], p[2] + c(0.01)],
        [c(-0.1), c(0.), c(0.)],
        [c(0.), c(0.), c(0.)],
        [c(0.1), c(0.), c(0.)],
        [c(0.2), c(0.), c(0.)],
        [c(0.3), c(0.), c(0.)],
    ];
    for tau in [1e-3, 1e-7, 1e-11] {
        let reference = wire(&positions, [0, 1], &[[2, 3], [3, 4]], 0.003, 100_000., tau)
            .unwrap()
            .energy;
        for targets in [
            [[2, 3], [3, 4], [5, 6]],
            [[6, 5], [4, 3], [3, 2]],
            [[3, 4], [5, 6], [2, 3]],
            [[2, 3], [3, 4], [4, 5]],
        ] {
            let padded = wire(&positions, [0, 1], &targets, 0.003, 100_000., tau)
                .unwrap()
                .energy;
            assert!((reference.v - padded.v).abs() < 1e-12);
            for a in 0..3 {
                assert!((reference.g[a] - padded.g[a]).abs() < 1e-8);
                for b in 0..3 {
                    assert!(
                        (reference.h[a][b] - padded.h[a][b]).abs() < 1e-6,
                        "tau={tau} {targets:?} H[{a},{b}]: {} vs {}",
                        reference.h[a][b],
                        padded.h[a][b]
                    );
                }
            }
        }
    }
}

#[test]
fn single_endpoint_boundary_hessian_matches_symmetric_gradient_differences() {
    let base = [
        [0., 0.001, -0.01],
        [0., 0.001, 0.01],
        [0., 0., 0.],
        [0.1, 0., 0.],
    ];
    for (source, target) in [([0, 1], [2, 3]), ([1, 0], [3, 2]), ([2, 3], [0, 1])] {
        for moving in 0..4 {
            let sample = |positions: [[f64; 3]; 4]| {
                let x = std::array::from_fn::<_, 4, _>(|i| {
                    std::array::from_fn(|a| {
                        if i == moving {
                            Jet::<3>::variable(positions[i][a], a)
                        } else {
                            Jet::c(positions[i][a])
                        }
                    })
                });
                wire(&x, source, &[target], 0.003, 100_000., 1e-7)
                    .unwrap()
                    .energy
            };
            let center = sample(base);
            for a in 0..3 {
                let eps = 1e-10;
                let mut plus = base;
                let mut minus = base;
                plus[moving][a] += eps;
                minus[moving][a] -= eps;
                let jp = sample(plus);
                let jm = sample(minus);
                assert!(((jp.v - jm.v) / (2. * eps) - center.g[a]).abs() < 1e-4);
                for b in 0..3 {
                    let fd = (jp.g[b] - jm.g[b]) / (2. * eps);
                    assert!(
                        (fd - center.h[b][a]).abs() < 0.1 + 1e-5 * fd.abs(),
                        "{source:?} {target:?} vertex {moving} H[{b},{a}]: {} vs {fd}",
                        center.h[b][a]
                    );
                }
            }
        }
    }
}

#[test]
fn structural_factorization_preserves_large_disconnected_graph_derivatives() {
    let n = 300;
    let nodes = vec![Jet::<1>::c(0.3); n];
    let mut links = Vec::new();
    let mut expected = Jet::c(n as f64 * 0.3);
    let tau = 1e-5;
    for i in 0..n / 2 {
        let c = Jet::variable(0.1 + i as f64 * 0.0001, 0);
        links.push(Link {
            nodes: [i, n - 1 - i],
            barrier: c,
        });
        expected = expected - Jet::c(2.) * c * c / (Jet::c(tau) + Jet::c(2.) * c);
    }
    let actual = evaluate(&nodes, &links, tau).unwrap();
    assert!((actual.energy.v - expected.v).abs() < 1e-10);
    assert!((actual.energy.g[0] - expected.g[0]).abs() < 1e-8);
    assert!((actual.energy.h[0][0] - expected.h[0][0]).abs() < 1e-7);
    // Zero-valued links remain structural. Dropping them to evade the dense
    // limit would silently discard possible derivatives.
    let connected: Vec<_> = (0..128)
        .map(|i| Link {
            nodes: [i, i + 1],
            barrier: Jet::<1>::variable(0., 0),
        })
        .collect();
    assert!(evaluate(&nodes[..129], &connected, tau).is_err());
}

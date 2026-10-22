use glam::{DMat3, DVec3};
use marvelous_cloth::{
    energy,
    garment::GarmentTemplate,
    solver_lab::{
        contact_qp, coupled_patch,
        dense_contact::{self, Row},
    },
};

#[test]
fn boundary_pivots_resolve_captured_sewn_contact_qp() {
    let capture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/sewn-contact-qp.json")).unwrap();
    let h: Vec<f64> = serde_json::from_value(capture["metric"].clone()).unwrap();
    let g: Vec<f64> = serde_json::from_value(capture["gradient"].clone()).unwrap();
    let rows: Vec<_> = capture["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Row {
            jacobian: serde_json::from_value(r["jacobian"].clone()).unwrap(),
            offset: r["offset"].as_f64().unwrap(),
        })
        .collect();
    let cfg = contact_qp::Settings {
        max_sweeps: 8192,
        feasibility_m: 1e-11,
        stationarity_n: 1e-7,
        complementarity_j: 1e-12,
    };
    let slow = dense_contact::solve_with_method(
        &h,
        &g,
        &rows,
        cfg,
        dense_contact::DualMethod::IndependentBlock,
    )
    .unwrap();
    let fast = dense_contact::solve_with_method(
        &h,
        &g,
        &rows,
        cfg,
        dense_contact::DualMethod::IndependentPivotBlock,
    )
    .unwrap();
    assert!(fast.converged, "{fast:?}");
    assert!(fast.sweeps < slow.sweeps);
    assert!(fast.boundary_pivots > 0);
    assert!(fast.multipliers.iter().all(|l| *l >= 0.));
    for (r, l) in rows.iter().zip(&fast.multipliers) {
        let gap = r
            .jacobian
            .iter()
            .zip(&fast.displacement)
            .map(|(a, x)| a * x)
            .sum::<f64>()
            - r.offset;
        assert!(gap >= -1e-11 && (l * gap).abs() <= 1e-12);
    }
}
#[test]
fn independent_blocks_keep_duplicate_reactions_and_stronger_parallel_rows() {
    let h = [1001., -1000., -1000., 1001.];
    let g = [1., 1.];
    let rows: Vec<_> = (0..16)
        .flat_map(|i| {
            let scale = (i + 1) as f64;
            [
                Row {
                    jacobian: vec![scale, 0.],
                    offset: 0.,
                },
                Row {
                    jacobian: vec![0., scale],
                    offset: 0.,
                },
                // This dependent direction is violated transiently after the
                // first two coordinate updates, so rank deficiency is real,
                // not dependent on roundoff activating an exact duplicate.
                Row {
                    jacobian: vec![-scale, scale],
                    offset: 0.,
                },
            ]
        })
        .collect();
    let cfg = contact_qp::Settings {
        max_sweeps: 20000,
        ..Default::default()
    };
    let control = dense_contact::solve_with_method(
        &h,
        &g,
        &rows,
        cfg,
        dense_contact::DualMethod::ActiveBlock,
    )
    .unwrap();
    let trial = dense_contact::solve_with_method(
        &h,
        &g,
        &rows,
        cfg,
        dense_contact::DualMethod::IndependentBlock,
    )
    .unwrap();
    assert!(control.converged && trial.converged);
    assert!(trial.reduced_active_blocks > 0);
    assert!(
        trial.sweeps < control.sweeps,
        "{} / {}",
        trial.sweeps,
        control.sweeps
    );
    assert!(trial.displacement.iter().all(|v| v.abs() < 1e-9));
    let mut reaction = [0.; 2];
    for (r, l) in rows.iter().zip(&trial.multipliers) {
        for (f, j) in reaction.iter_mut().zip(&r.jacobian) {
            *f += l * j;
        }
    }
    assert!((reaction[0] - 1.).abs() < 1e-7 && (reaction[1] - 1.).abs() < 1e-7);
    // Same normal, different offset: selecting a basis must not drop the
    // stronger inequality or falsely accept the weak boundary.
    let r = dense_contact::solve_with_method(
        &[1.],
        &[1.],
        &[
            Row {
                jacobian: vec![1.],
                offset: 0.,
            },
            Row {
                jacobian: vec![2.],
                offset: 0.02,
            },
        ],
        cfg,
        dense_contact::DualMethod::IndependentBlock,
    )
    .unwrap();
    assert!(r.converged && (r.displacement[0] - 0.01).abs() < 1e-10);
}
#[test]
fn dependent_and_nearly_dependent_infeasibility_is_never_ignored() {
    let cfg = contact_qp::Settings {
        max_sweeps: 8,
        ..Default::default()
    };
    for rows in [
        vec![
            Row {
                jacobian: vec![1., 0.],
                offset: 1.,
            },
            Row {
                jacobian: vec![-1., 0.],
                offset: 1.,
            },
        ],
        vec![
            Row {
                jacobian: vec![1., 0.],
                offset: 0.,
            },
            Row {
                jacobian: vec![-1., 1e-7],
                offset: 1e-7,
            },
        ],
    ] {
        for method in [
            dense_contact::DualMethod::IndependentBlock,
            dense_contact::DualMethod::IndependentPivotBlock,
        ] {
            let r =
                dense_contact::solve_with_method(&[1., 0., 0., 1.], &[0., 0.], &rows, cfg, method)
                    .unwrap();
            assert!(!r.converged);
            assert!(r.feasibility_m > cfg.feasibility_m);
        }
    }
}
#[test]
fn active_block_accelerates_coupled_reactions_without_changing_the_qp() {
    use dense_contact::DualMethod;
    let h = [1001., -1000., -1000., 1001.];
    let g = [1., 1.];
    let base = [
        Row {
            jacobian: vec![1., 0.],
            offset: 0.,
        },
        Row {
            jacobian: vec![0., 1.],
            offset: 0.,
        },
    ];
    let cfg = contact_qp::Settings {
        max_sweeps: 20000,
        ..Default::default()
    };
    let slow = dense_contact::solve(&h, &g, &base, cfg).unwrap();
    let fast =
        dense_contact::solve_with_method(&h, &g, &base, cfg, DualMethod::ActiveBlock).unwrap();
    assert!(slow.converged && fast.converged);
    assert!(fast.sweeps < slow.sweeps / 10);
    assert!(fast.active_block_steps > 0);
    for (a, b) in fast.displacement.iter().zip(&slow.displacement) {
        assert!((a - b).abs() < 1e-9);
    }
    for (a, b) in fast.multipliers.iter().zip(&slow.multipliers) {
        assert!((a - b).abs() < 1e-7);
    }
    let duplicated = [
        base[0].clone(),
        base[1].clone(),
        base[0].clone(),
        base[1].clone(),
    ];
    let duplicate =
        dense_contact::solve_with_method(&h, &g, &duplicated, cfg, DualMethod::ActiveBlock)
            .unwrap();
    assert!(duplicate.converged);
    assert!(duplicate.displacement.iter().all(|v| v.abs() < 1e-9));
}
#[test]
fn schur_memory_budget_falls_back_without_losing_constraints() {
    let rows: Vec<_> = (0..600)
        .map(|i| Row {
            jacobian: vec![if i % 2 == 0 { 1. } else { -1. }],
            offset: 1.,
        })
        .collect();
    let cfg = contact_qp::Settings {
        max_sweeps: 4,
        ..Default::default()
    };
    let cached = dense_contact::solve_with_method(
        &[1.],
        &[0.],
        &rows,
        cfg,
        dense_contact::DualMethod::IndependentBlock,
    )
    .unwrap();
    let direct = dense_contact::solve_with_method(
        &[1.],
        &[0.],
        &rows,
        cfg,
        dense_contact::DualMethod::IndependentBlockUncached,
    )
    .unwrap();
    assert_eq!(cached.schur_cache_bytes, 0);
    assert!(!cached.converged);
    assert_eq!(cached.displacement, direct.displacement);
    assert_eq!(cached.multipliers, direct.multipliers);
    assert!(cached.feasibility_m > 1.);
}
#[test]
fn active_set_changes_match_the_independent_three_dof_solver() {
    let mut seed = 823u64;
    let mut random = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 32) as f64 / u32::MAX as f64 * 2. - 1.
    };
    for _ in 0..32 {
        let u = DVec3::new(random(), random(), random());
        let h = DMat3::from_diagonal(DVec3::new(1., 2., 3.))
            + DMat3::from_cols(u * u.x, u * u.y, u * u.z);
        let g = DVec3::new(random(), random(), random()) * 4.;
        let rows: Vec<_> = (0..5)
            .map(|_| contact_qp::Row {
                normal: DVec3::new(random(), random(), random()).normalize(),
                offset: -0.05 - 0.2 * random().abs(),
            })
            .collect();
        let cfg = contact_qp::Settings {
            max_sweeps: 20000,
            ..Default::default()
        };
        let exact = contact_qp::solve(h, g, &rows, None, cfg).unwrap();
        let coupled = dense_contact::solve_with_method(
            &h.to_cols_array(),
            &g.to_array(),
            &rows
                .iter()
                .map(|r| Row {
                    jacobian: r.normal.to_array().to_vec(),
                    offset: r.offset,
                })
                .collect::<Vec<_>>(),
            cfg,
            dense_contact::DualMethod::ActiveBlock,
        )
        .unwrap();
        assert!(exact.converged && coupled.converged);
        assert!(DVec3::from_slice(&coupled.displacement).distance(exact.displacement) < 1e-7);
        assert!(coupled.multipliers.iter().all(|v| *v >= 0.));
        let redundant: Vec<_> = rows
            .iter()
            .flat_map(|r| {
                [1., 2., 0.5].map(|scale| Row {
                    jacobian: (scale * r.normal).to_array().to_vec(),
                    offset: scale * r.offset,
                })
            })
            .collect();
        let independent = dense_contact::solve_with_method(
            &h.to_cols_array(),
            &g.to_array(),
            &redundant,
            cfg,
            dense_contact::DualMethod::IndependentBlock,
        )
        .unwrap();
        assert!(independent.converged);
        assert!(DVec3::from_slice(&independent.displacement).distance(exact.displacement) < 1e-7);
        let direct = dense_contact::solve_with_method(
            &h.to_cols_array(),
            &g.to_array(),
            &redundant,
            cfg,
            dense_contact::DualMethod::IndependentBlockUncached,
        )
        .unwrap();
        assert_eq!(independent.displacement, direct.displacement);
        assert_eq!(independent.multipliers, direct.multipliers);
        assert_eq!(independent.sweeps, direct.sweeps);
        let pivot = dense_contact::solve_with_method(
            &h.to_cols_array(),
            &g.to_array(),
            &redundant,
            cfg,
            dense_contact::DualMethod::IndependentPivotBlock,
        )
        .unwrap();
        assert!(pivot.converged);
        assert!(DVec3::from_slice(&pivot.displacement).distance(exact.displacement) < 1e-7);
    }
}

#[test]
fn coupled_contact_moves_both_sides_without_block_lock() {
    // Two touching coordinates, equal masses, load only on the left.
    // Independent feasible point updates cannot move the left until the right
    // moves; the joint solve must find their shared displacement immediately.
    let h = [1., 0., 0., 1.];
    let g = [-2., 0.];
    for copies in [1, 8] {
        let rows = vec![
            Row {
                jacobian: vec![-1., 1.],
                offset: 0.
            };
            copies
        ];
        let r = dense_contact::solve(&h, &g, &rows, contact_qp::Settings::default()).unwrap();
        assert!(r.converged);
        assert!((r.displacement[0] - 1.).abs() < 1e-10);
        assert!((r.displacement[1] - 1.).abs() < 1e-10);
        assert!((r.multipliers.iter().sum::<f64>() - 1.).abs() < 1e-10);
    }
}
#[test]
fn dense_kernel_matches_three_dof_oracle_and_reports_infeasibility() {
    let h = DMat3::from_cols(
        DVec3::new(4., 1., 0.),
        DVec3::new(1., 3., 0.5),
        DVec3::new(0., 0.5, 2.),
    );
    let g = DVec3::new(2., 5., -3.);
    let rows = [
        contact_qp::Row {
            normal: DVec3::Y,
            offset: 0.,
        },
        contact_qp::Row {
            normal: DVec3::X,
            offset: 0.,
        },
    ];
    let a = contact_qp::solve(h, g, &rows, None, Default::default()).unwrap();
    let b = dense_contact::solve(
        &h.to_cols_array(),
        &g.to_array(),
        &rows
            .iter()
            .map(|r| Row {
                jacobian: r.normal.to_array().to_vec(),
                offset: r.offset,
            })
            .collect::<Vec<_>>(),
        Default::default(),
    )
    .unwrap();
    assert!(a.converged && b.converged);
    assert!(DVec3::from_slice(&b.displacement).distance(a.displacement) < 1e-10);
    let bad = dense_contact::solve(
        &[1.],
        &[0.],
        &[
            Row {
                jacobian: vec![1.],
                offset: 1.,
            },
            Row {
                jacobian: vec![-1.],
                offset: 1.,
            },
        ],
        contact_qp::Settings {
            max_sweeps: 8,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!bad.converged && bad.feasibility_m > 1.);
    assert!(dense_contact::solve(&[-1.], &[0.], &[], Default::default()).is_err());
}
#[test]
fn patch_assembly_matches_production_energy_and_finite_difference_forces() {
    let mut template = GarmentTemplate::two_panel_demo();
    for panel in &mut template.panels {
        panel.pattern.nx = 1;
        panel.pattern.ny = 2;
        panel.pins.clear();
    }
    let g = template.compile().unwrap();
    let mut x = g.initial_state.x.clone();
    for (i, p) in x.iter_mut().enumerate() {
        *p += DVec3::new(0.001 * (i as f64).cos(), 0.002 * (i as f64).sin(), 0.003);
    }
    let target = g.initial_state.x.clone();
    let inertia = vec![3.; x.len()];
    let s = coupled_patch::assemble(&g, &x, &target, &inertia).unwrap();
    assert_eq!(
        s.energy,
        coupled_patch::energy(&g, &x, &target, &inertia).unwrap()
    );
    let expected = g
        .mesh
        .triangles
        .iter()
        .map(|t| {
            energy::membrane(
                t.vertices.map(|i| x[i].to_array()),
                t,
                &g.materials[t.material],
            )
        })
        .sum::<f64>()
        + g.mesh
            .hinges
            .iter()
            .map(|h| {
                energy::bending(
                    h.vertices.map(|i| x[i].to_array()),
                    h,
                    &g.materials[h.material],
                )
            })
            .sum::<f64>()
        + g.seams.iter().map(|s| s.energy(&x)).sum::<f64>()
        + x.iter()
            .zip(&target)
            .map(|(p, q)| 1.5 * p.distance_squared(*q))
            .sum::<f64>();
    assert!((s.energy - expected).abs() < 1e-10 * expected.max(1.));
    let internal: Vec<_> = s
        .gradient
        .as_chunks::<3>()
        .0
        .iter()
        .enumerate()
        .map(|(i, v)| DVec3::from_slice(v) - inertia[i] * (x[i] - target[i]))
        .collect();
    assert!(internal.iter().copied().sum::<DVec3>().length() < 1e-8);
    assert!(
        x.iter()
            .zip(&internal)
            .map(|(p, f)| p.cross(*f))
            .sum::<DVec3>()
            .length()
            < 1e-8
    );
    for k in 0..x.len() * 3 {
        let mut plus = x.clone();
        let mut minus = x.clone();
        plus[k / 3][k % 3] += 1e-6;
        minus[k / 3][k % 3] -= 1e-6;
        for p in [&plus, &minus] {
            assert_eq!(
                coupled_patch::energy(&g, p, &target, &inertia).unwrap(),
                coupled_patch::assemble(&g, p, &target, &inertia)
                    .unwrap()
                    .energy
            );
        }
        let fd = (coupled_patch::assemble(&g, &plus, &target, &inertia)
            .unwrap()
            .energy
            - coupled_patch::assemble(&g, &minus, &target, &inertia)
                .unwrap()
                .energy)
            / 2e-6;
        assert!((fd - s.gradient[k]).abs() < 1e-5 * fd.abs().max(1.));
    }
    let q = dense_contact::solve(&s.metric, &s.gradient, &[], Default::default()).unwrap();
    assert!(q.converged);
    assert!(
        q.displacement
            .iter()
            .zip(&s.gradient)
            .map(|(a, b)| a * b)
            .sum::<f64>()
            < 0.
    );
}

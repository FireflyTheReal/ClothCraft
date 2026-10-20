use glam::{DMat3, DVec3};
use marvelous_cloth::solver_lab::contact_qp::{self, Row, Settings};

#[test]
fn duplicated_scaled_and_reordered_contacts_preserve_primal_and_reaction() {
    let h = DMat3::from_diagonal(DVec3::new(2., 3., 5.));
    let g = DVec3::new(-4., 6., -5.);
    for count in [1, 2, 7, 64] {
        let rows = (0..count)
            .map(|i| Row {
                normal: DVec3::Y * (i + 1) as f64,
                offset: 0.,
            })
            .collect::<Vec<_>>();
        for warm in [vec![0.; count], vec![11.; count]] {
            let r = contact_qp::solve(h, g, &rows, Some(&warm), Settings::default()).unwrap();
            assert!(r.converged);
            assert!((r.displacement - DVec3::new(2., 0., 1.)).length() < 1e-10);
            assert!((r.reaction - DVec3::Y * 6.).length() < 1e-9);
        }
    }
}
#[test]
fn separated_contact_releases_warm_started_reaction() {
    let row = Row {
        normal: DVec3::Y,
        offset: 0.,
    };
    let r = contact_qp::solve(
        DMat3::IDENTITY,
        -DVec3::Y,
        &[row],
        Some(&[100.]),
        Settings::default(),
    )
    .unwrap();
    assert!(r.converged);
    assert_eq!(r.lambda, [0.]);
    assert_eq!(r.displacement, DVec3::Y);
}
#[test]
fn two_sided_gap_and_corner_keep_both_constraints() {
    let rows = [
        Row {
            normal: DVec3::Y,
            offset: 0.001,
        },
        Row {
            normal: -DVec3::Y,
            offset: -0.002,
        },
        Row {
            normal: DVec3::X,
            offset: 0.,
        },
    ];
    for (g, y) in [
        (DVec3::new(1., 1., 0.), 0.001),
        (DVec3::new(1., -1., 0.), 0.002),
    ] {
        let r = contact_qp::solve(DMat3::IDENTITY, g, &rows, None, Settings::default()).unwrap();
        assert!(r.converged);
        assert!((r.displacement - DVec3::new(0., y, 0.)).length() < 1e-10);
        assert!(r.lambda.iter().all(|v| *v >= 0.));
    }
}
#[test]
fn incompatible_constraints_and_iteration_limits_are_not_false_success() {
    let impossible = [
        Row {
            normal: DVec3::X,
            offset: 1.,
        },
        Row {
            normal: -DVec3::X,
            offset: 1.,
        },
    ];
    let r = contact_qp::solve(
        DMat3::IDENTITY,
        DVec3::ZERO,
        &impossible,
        None,
        Settings {
            max_sweeps: 16,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!r.converged && r.feasibility_m > 0.5);
    let narrow = [
        Row {
            normal: DVec3::new(1., 0.01, 0.),
            offset: 0.,
        },
        Row {
            normal: DVec3::new(-1., 0.01, 0.),
            offset: 0.,
        },
    ];
    let r = contact_qp::solve(
        DMat3::IDENTITY,
        DVec3::Y,
        &narrow,
        None,
        Settings {
            max_sweeps: 4,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!r.converged);
}
#[test]
fn invalid_metrics_rows_and_warm_starts_are_rejected() {
    let cfg = Settings::default();
    assert!(contact_qp::solve(DMat3::ZERO, DVec3::ZERO, &[], None, cfg).is_err());
    assert!(
        contact_qp::solve(
            DMat3::from_diagonal(DVec3::new(1., -1., 1.)),
            DVec3::ZERO,
            &[],
            None,
            cfg
        )
        .is_err()
    );
    assert!(
        contact_qp::solve(
            DMat3::IDENTITY,
            DVec3::ZERO,
            &[Row {
                normal: DVec3::ZERO,
                offset: 0.
            }],
            None,
            cfg
        )
        .is_err()
    );
    assert!(contact_qp::solve(DMat3::IDENTITY, DVec3::ZERO, &[], Some(&[1.]), cfg).is_err());
}

// Independent tiny dense active-set oracle: enumerate <=3 binding rows and
// solve the augmented KKT system with pivoted Gaussian elimination.
fn linear(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for k in 0..n {
        let p = (k..n).max_by(|i, j| a[*i][k].abs().total_cmp(&a[*j][k].abs()))?;
        if a[p][k].abs() < 1e-12 {
            return None;
        }
        a.swap(k, p);
        b.swap(k, p);
        let pivot = a[k][k];
        for value in &mut a[k][k..] {
            *value /= pivot;
        }
        b[k] /= pivot;
        let pivot_row = a[k].clone();
        for i in 0..n {
            if i == k {
                continue;
            }
            let t = a[i][k];
            for (value, pivot_value) in a[i][k..].iter_mut().zip(&pivot_row[k..]) {
                *value -= t * pivot_value;
            }
            b[i] -= t * b[k];
        }
    }
    Some(b)
}
fn oracle(h: DMat3, g: DVec3, rows: &[Row]) -> DVec3 {
    let mut best = None;
    for mask in 0..(1usize << rows.len()) {
        let ids = (0..rows.len())
            .filter(|i| mask & (1 << i) != 0)
            .collect::<Vec<_>>();
        if ids.len() > 3 {
            continue;
        }
        let mut a = vec![vec![0.; 3 + ids.len()]; 3 + ids.len()];
        let mut b = vec![0.; 3 + ids.len()];
        for i in 0..3 {
            b[i] = -g[i];
            for (j, value) in a[i][..3].iter_mut().enumerate() {
                *value = h.col(j)[i];
            }
        }
        for (k, &id) in ids.iter().enumerate() {
            b[3 + k] = rows[id].offset;
            for (j, value) in rows[id].normal.to_array().into_iter().enumerate() {
                a[j][3 + k] = -value;
                a[3 + k][j] = value;
            }
        }
        let Some(solution) = linear(a, b) else {
            continue;
        };
        let d = DVec3::from_slice(&solution[..3]);
        if solution[3..].iter().any(|v| *v < -1e-8)
            || rows.iter().any(|r| r.normal.dot(d) < r.offset - 1e-8)
        {
            continue;
        }
        let e = 0.5 * d.dot(h * d) + g.dot(d);
        if best.is_none_or(|(value, _)| e < value) {
            best = Some((e, d));
        }
    }
    best.unwrap().1
}
#[test]
fn coupled_metric_matches_independent_dense_active_set_oracle() {
    let mut seed = 137u64;
    let mut random = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 32) as f64 / u32::MAX as f64 * 2. - 1.
    };
    for _ in 0..40 {
        let u = DVec3::new(random(), random(), random());
        let h = DMat3::from_diagonal(DVec3::new(0.4, 2., 5.))
            + DMat3::from_cols(u * u.x, u * u.y, u * u.z);
        let g = DVec3::new(random(), random(), random()) * 3.;
        let witness = DVec3::new(random(), random(), random());
        let rows = (0..7)
            .map(|_| {
                let n = DVec3::new(random(), random(), random()).normalize();
                Row {
                    normal: n,
                    offset: n.dot(witness) - 0.2 - 0.5 * random().abs(),
                }
            })
            .collect::<Vec<_>>();
        let expected = oracle(h, g, &rows);
        let cfg = Settings {
            max_sweeps: 8192,
            ..Default::default()
        };
        let a = contact_qp::solve(h, g, &rows, None, cfg).unwrap();
        assert!(a.converged, "{a:?}");
        assert!((a.displacement - expected).length() < 1e-7);
        let reverse = rows.into_iter().rev().collect::<Vec<_>>();
        let b = contact_qp::solve(h, g, &reverse, None, cfg).unwrap();
        assert!(b.converged && (b.displacement - expected).length() < 1e-7);
    }
}

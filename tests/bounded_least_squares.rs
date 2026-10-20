use marvelous_cloth::solver_lab::bounded_least_squares::solve;

#[test]
fn bound_release_and_inconsistent_residual_are_explicit() {
    // x is free; lambda >= 0. Exact unconstrained solution lambda=-1
    // is inadmissible. The constrained minimizer is x=.5, lambda=0.
    let r = solve(&[1., -1., 1., 0.], &[1., 0.], 2, 1, &[0., 1.], 64).unwrap();
    assert!(r.converged);
    assert!((r.solution[0] - 0.5).abs() < 1e-12);
    assert_eq!(r.solution[1], 0.);
    assert!((r.residual_norm - 0.5_f64.sqrt()).abs() < 1e-12);
    // A variable initially fixed at zero must be released when its
    // reduced gradient indicates an improvement into the feasible cone.
    let r = solve(&[1., 0., 0., 1.], &[2., 3.], 2, 1, &[0., 0.], 64).unwrap();
    assert!(r.converged);
    assert!((r.solution[0] - 2.).abs() < 1e-12 && (r.solution[1] - 3.).abs() < 1e-12);
}

#[test]
fn rank_deficiency_zero_columns_and_invalid_inputs_are_not_hidden() {
    let r = solve(
        &[1., 1., 0., 2., 2., 0.],
        &[1., 2.],
        3,
        1,
        &[0., 0., 0.],
        64,
    )
    .unwrap();
    assert!(r.converged && r.residual_norm < 1e-12);
    assert!(r.solution[1] >= 0.);
    assert!(solve(&[1.], &[1.], 1, 0, &[-1.], 8).is_err());
    assert!(solve(&[f64::NAN], &[1.], 1, 0, &[0.], 8).is_err());
    assert!(solve(&[1.], &[1.], 1, 0, &[0.], 1).is_err());
    assert!(solve(&[0., 0.], &[f64::MAX, f64::MAX], 1, 0, &[0.], 8).is_err());
}

#[test]
fn seeded_systems_match_independent_scipy_bvls_oracle() {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/bounded-ls-oracle.json")).unwrap();
    for case in data["cases"].as_array().unwrap() {
        let read = |key: &str| {
            case[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect::<Vec<_>>()
        };
        let a = read("a");
        let b = read("b");
        let expected = read("solution");
        let initial = read("initial");
        let n = expected.len();
        let report = solve(&a, &b, n, 3, &initial, 256).unwrap();
        assert!(report.converged, "{report:?}");
        for (&x, &y) in report.solution.iter().zip(&expected) {
            assert!((x - y).abs() <= 1e-7 * (1. + y.abs()), "{x} vs {y}");
        }
        assert!(report.solution[3..].iter().all(|x| *x >= 0.));
        let reference = case["residual_norm"].as_f64().unwrap();
        assert!((report.residual_norm - reference).abs() < 1e-8 * (1. + reference));
    }
}

#[test]
fn captured_optimum_matches_oracle_but_is_not_a_collision_certificate() {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/bounded-contact-ls.json")).unwrap();
    let read = |key: &str| {
        data[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect::<Vec<_>>()
    };
    let n = data["columns"].as_u64().unwrap() as usize;
    let free = data["free_prefix"].as_u64().unwrap() as usize;
    let r = solve(&read("a"), &read("b"), n, free, &read("initial"), 512).unwrap();
    assert!(r.converged);
    assert!(r.solution[free..].iter().all(|v| *v >= 0.));
    let reference = data["reference_residual_norm"].as_f64().unwrap();
    assert!((r.residual_norm - reference).abs() < 1e-7);
    let min_gap = data["geometry"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row["jacobian"]
                .as_array()
                .unwrap()
                .iter()
                .zip(&r.solution[..free])
                .map(|(j, d)| j.as_f64().unwrap() * d)
                .sum::<f64>()
                - row["offset"].as_f64().unwrap()
        })
        .fold(f64::INFINITY, f64::min);
    // Numerical optimization converged; the proposed path is still inadmissible.
    // This negative control must never be converted into a physical pass.
    assert!(min_gap < -1e-10);
}

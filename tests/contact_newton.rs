use marvelous_cloth::solver_lab::contact_newton::{least_squares, solve};

#[test]
fn nonsymmetric_force_geometry_blocks_recover_known_solution() {
    // SPD material block, unequal force/geometry blocks, indefinite KKT.
    let a = vec![4., 1., -0.2, 1., 3., -1., 0., 1., 0.];
    let expected = [0.3, -0.1, 2.];
    let b: Vec<_> = a
        .chunks_exact(3)
        .map(|r| r.iter().zip(expected).map(|(a, x)| a * x).sum::<f64>())
        .collect();
    for scales in [[1., 1., 1.], [1e-6, 1e3, 1e8]] {
        let scaled_a = a
            .chunks_exact(3)
            .zip(scales)
            .flat_map(|(r, s)| r.iter().map(move |v| v * s))
            .collect();
        let scaled_b = b.iter().zip(scales).map(|(v, s)| v * s).collect();
        let result = solve(scaled_a, scaled_b).unwrap();
        assert!(
            result
                .iter()
                .zip(expected)
                .all(|(a, b)| (a - b).abs() < 1e-12)
        );
    }
}

#[test]
fn singular_and_invalid_systems_are_not_regularized() {
    assert!(solve(vec![1., 2., 2., 4.], vec![1., 2.]).is_err());
    assert!(solve(vec![1., f64::NAN, 0., 1.], vec![1., 2.]).is_err());
    assert!(solve(vec![1.], vec![1., 2.]).is_err());
    assert!(solve(vec![], vec![]).is_err());
}

#[test]
fn rank_deficient_and_inconsistent_rows_are_reported_without_being_hidden() {
    for scale in [1e-120, 1., 1e120] {
        let a: Vec<_> = [1., 2., 2., 4.].iter().map(|x| x * scale).collect();
        let r = least_squares(&a, &[scale, 2. * scale], 2, 1e-12).unwrap();
        assert_eq!(r.rank, 1);
        assert!((r.solution[0] - 0.2).abs() < 1e-12);
        assert!((r.solution[1] - 0.4).abs() < 1e-12);
        assert!(r.residual_norm / scale < 1e-12);
        let r = least_squares(&a, &[scale, 3. * scale], 2, 1e-12).unwrap();
        assert_eq!(r.rank, 1);
        assert!((r.solution[0] - 0.28).abs() < 1e-12);
        assert!((r.solution[1] - 0.56).abs() < 1e-12);
        assert!((r.residual_norm / scale - 0.2_f64.sqrt()).abs() < 1e-12);
    }
    let r = least_squares(&[1., 1., 1.], &[3.], 3, 1e-12).unwrap();
    assert_eq!(r.rank, 1);
    assert!(r.solution.iter().all(|x| (x - 1.).abs() < 1e-12));
    let r = least_squares(&[0.; 6], &[1., 2.], 3, 1e-12).unwrap();
    assert_eq!(r.rank, 0);
    assert!((r.residual_norm - 5_f64.sqrt()).abs() < 1e-12);
    assert!(least_squares(&[f64::NAN], &[0.], 1, 1e-12).is_err());
    assert!(least_squares(&[1.], &[0.], 1, 0.).is_err());
    assert!(least_squares(&[0.; 4], &[f64::MAX; 2], 2, 1e-12).is_err());
}

#[test]
fn minimum_norm_solution_survives_mixed_columns_and_rectangular_rank() {
    for m in [4, 7] {
        let n = 6;
        let mut a = vec![0.; m * n];
        let mut expected = vec![0.; n];
        for i in 0..3 {
            a[i * n + i] = (i + 1) as f64;
            expected[i] = 0.1 * (i + 1) as f64;
        }
        for k in 0..20 {
            let i = k % n;
            let j = (i + 1 + k % (n - 1)) % n;
            let (s, c) = (0.37 * (k + 1) as f64).sin_cos();
            for row in a.chunks_exact_mut(n) {
                let (u, v) = (row[i], row[j]);
                row[i] = c * u + s * v;
                row[j] = -s * u + c * v;
            }
            let (u, v) = (expected[i], expected[j]);
            expected[i] = c * u + s * v;
            expected[j] = -s * u + c * v;
        }
        let b: Vec<_> = a
            .chunks_exact(n)
            .map(|r| r.iter().zip(&expected).map(|(a, x)| a * x).sum::<f64>())
            .collect();
        let r = least_squares(&a, &b, n, 1e-12).unwrap();
        assert_eq!(r.rank, 3);
        assert!(
            r.solution
                .iter()
                .zip(&expected)
                .all(|(x, y)| (x - y).abs() < 1e-12)
        );
        assert!(r.residual_norm < 1e-12);
    }
}

#[test]
fn captured_contact_rank_and_solution_match_independent_svd_oracle() {
    let c: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/discrete-contact-newton.json")).unwrap();
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/discrete-contact-newton-oracle.json")).unwrap();
    let vector = |value: &serde_json::Value| {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect::<Vec<_>>()
    };
    let mut a = vector(&c["matrix"]);
    let mut b = vector(&c["rhs"]);
    let n = b.len();
    let dofs = c["dofs"].as_u64().unwrap() as usize;
    for i in 0..n {
        let scale = if i < dofs { 1e-7 } else { 1e-10 };
        for j in 0..n {
            a[i * n + j] /= scale;
        }
        b[i] /= scale;
    }
    let r = least_squares(&a, &b, n, 1e-12).unwrap();
    assert_eq!(r.rank, oracle["rank"].as_u64().unwrap() as usize);
    let expected = vector(&oracle["solution"]);
    let difference = r
        .solution
        .iter()
        .zip(&expected)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        .sqrt();
    assert!(difference < 1e-10, "{difference:e}");
    assert!(r.residual_norm < 1e-4, "{:e}", r.residual_norm);
}

use marvelous_cloth::solver_lab::{constrained_least_squares::solve, dense_contact::Row};

#[test]
fn manufactured_convex_kkt_optima_cover_working_set_changes() {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/constrained-ls-kkt.json")).unwrap();
    for case in data["cases"].as_array().unwrap() {
        let read = |key: &str| {
            case[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect::<Vec<_>>()
        };
        let expected = read("solution");
        let rows: Vec<_> = case["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| Row {
                jacobian: r["jacobian"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_f64().unwrap())
                    .collect(),
                offset: r["offset"].as_f64().unwrap(),
            })
            .collect();
        let report = solve(
            &read("a"),
            &read("b"),
            expected.len(),
            &rows,
            &read("initial"),
            512,
        )
        .unwrap();
        assert!(report.converged, "{report:?}");
        for (x, y) in report.solution.iter().zip(&expected) {
            assert!((x - y).abs() < 1e-7 * (1. + y.abs()), "{x} versus {y}");
        }
    }
}

#[test]
fn constrained_minimum_enters_and_releases_bounds() {
    let rows = [
        Row {
            jacobian: vec![1., 1.],
            offset: 1.,
        },
        Row {
            jacobian: vec![1., 0.],
            offset: 0.,
        },
    ];
    for initial in [[-2., -3.], [5., -3.], [0., 1.]] {
        let r = solve(&[1., 0., 0., 1.], &[-1., -1.], 2, &rows, &initial, 128).unwrap();
        assert!(r.converged, "{r:?}");
        assert!((r.solution[0] - 0.5).abs() < 1e-10 && (r.solution[1] - 0.5).abs() < 1e-10);
        assert!(r.normalized_feasibility <= 1e-11);
    }
    let r = solve(&[1., 0., 0., 1.], &[2., 3.], 2, &rows, &[0., 1.], 128).unwrap();
    assert!(r.converged && r.residual_norm < 1e-10);
}

#[test]
fn duplicate_constraints_and_infeasibility_remain_explicit() {
    let rows = [
        Row {
            jacobian: vec![1.],
            offset: 1.,
        },
        Row {
            jacobian: vec![2.],
            offset: 2.,
        },
    ];
    let r = solve(&[1.], &[0.], 1, &rows, &[0.], 128).unwrap();
    assert!(r.converged && (r.solution[0] - 1.).abs() < 1e-10);
    let impossible = [
        rows[0].clone(),
        Row {
            jacobian: vec![-1.],
            offset: 0.,
        },
    ];
    assert!(solve(&[1.], &[0.], 1, &impossible, &[0.], 128).is_err());
    assert!(solve(&[f64::NAN], &[0.], 1, &rows, &[0.], 128).is_err());
    assert!(solve(&[1.], &[0.], 1, &rows, &[0.], 0).is_err());
}

#[test]
fn captured_contact_search_retains_geometric_and_reaction_inequalities() {
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
    let mut rows: Vec<_> = data["geometry"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut j = r["jacobian"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect::<Vec<_>>();
            j.resize(n, 0.);
            Row {
                jacobian: j,
                offset: r["offset"].as_f64().unwrap(),
            }
        })
        .collect();
    for i in free..n {
        let mut j = vec![0.; n];
        j[i] = 1.;
        rows.push(Row {
            jacobian: j,
            offset: 0.,
        });
    }
    let r = solve(&read("a"), &read("b"), n, &rows, &read("initial"), 512).unwrap();
    assert!(r.converged, "{r:?}");
    assert!(r.normalized_feasibility <= 1e-11);
    assert!(r.solution[free..].iter().all(|v| *v >= -1e-12));
    assert!(r.residual_norm >= data["reference_residual_norm"].as_f64().unwrap() - 1e-7);
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/constrained-contact-ls-oracle.json")).unwrap();
    assert!((r.residual_norm - oracle["residual_norm"].as_f64().unwrap()).abs() < 1e-7);
}

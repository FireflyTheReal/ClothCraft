use marvelous_cloth::solver_lab::{
    dense_contact::Row,
    unilateral_newton::{Problem, solve},
};

#[test]
fn path_search_constraints_do_not_become_physical_reactions() {
    use marvelous_cloth::solver_lab::unilateral_newton::{
        constrained_direction, constrained_direction_with_search_rows,
    };
    let make = || Problem {
        derivative: &[1.],
        gradient: &[-1.],
        forces: &[],
        rows: &[],
    };
    let free = constrained_direction(make(), &[]).unwrap();
    assert!((free.solution[0] - 1.).abs() < 1e-12);
    // A search-only cap on dx cannot supply an artificial physical force.
    let bounded = constrained_direction_with_search_rows(
        make(),
        &[],
        &[Row {
            jacobian: vec![-1.],
            offset: -0.25,
        }],
    )
    .unwrap();
    assert!(bounded.converged);
    assert_eq!(bounded.solution.len(), 1);
    assert!((bounded.solution[0] - 0.25).abs() < 1e-12);
    assert!(bounded.residual_norm > 1e6);
    assert!(
        constrained_direction_with_search_rows(
            make(),
            &[],
            &[Row {
                jacobian: vec![0., 1.],
                offset: 0.
            }]
        )
        .is_err()
    );
}

#[test]
fn natural_direction_linearizes_the_current_merit_branch() {
    use marvelous_cloth::solver_lab::unilateral_newton::natural_direction;
    for gap in [0.01, 0.00001] {
        let rows = [Row {
            jacobian: vec![1.],
            offset: -gap,
        }];
        let forces = [vec![2.]];
        let history = 1.;
        let (d, l) = natural_direction(
            Problem {
                derivative: &[3.],
                gradient: &[0.5],
                forces: &forces,
                rows: &rows,
            },
            &[history],
        )
        .unwrap();
        // Exact directional derivative of the force residual.
        let old_force = 0.5 - 2. * history;
        let change = 3. * d[0] - 2. * (l[0] - history);
        assert!((change + old_force).abs() < 1e-10);
        let a = 1e-6;
        let before = (history / 1e-7).min(gap / 1e-10);
        let after = ((history + a * (l[0] - history)) / 1e-7).min((gap + a * d[0]) / 1e-10);
        assert!(((after - before) / a + before).abs() < 0.01);
        if gap == 0.01 {
            assert_eq!(l[0], 0.);
        }
    }
}

#[test]
fn contact_can_release_and_activate_without_attraction() {
    let rows = [Row {
        jacobian: vec![1.],
        offset: 0.,
    }];
    let forces = [vec![1.]];
    for (g, initial, dx, lambda) in [(-1., true, 1., 0.), (1., false, 0., 1.)] {
        let r = solve(
            Problem {
                derivative: &[1.],
                gradient: &[g],
                forces: &forces,
                rows: &rows,
            },
            &[initial],
            8,
        )
        .unwrap();
        assert!(r.converged);
        assert!((r.displacement[0] - dx).abs() < 1e-12);
        assert!((r.multipliers[0] - lambda).abs() < 1e-12);
        assert_eq!(r.releases, usize::from(initial));
        assert_eq!(r.activations, usize::from(!initial));
    }
}

#[test]
fn distinct_force_geometry_and_duplicate_contacts_preserve_all_residuals() {
    let rows = vec![
        Row {
            jacobian: vec![0., 1.],
            offset: 0.
        };
        2
    ];
    let forces = vec![vec![0.2, 1.]; 2];
    let r = solve(
        Problem {
            derivative: &[2., 0., 0., 1.],
            gradient: &[0., 1.],
            forces: &forces,
            rows: &rows,
        },
        &[true, true],
        8,
    )
    .unwrap();
    assert!(r.converged);
    assert!((r.displacement[0] - 0.1).abs() < 1e-12);
    assert!(r.displacement[1].abs() < 1e-12);
    assert!((r.multipliers.iter().sum::<f64>() - 1.).abs() < 1e-12);
    assert!(r.multipliers.iter().all(|l| *l >= 0.));
    assert_eq!(r.rank, 3);
}

#[test]
fn incompatible_equalities_and_nonmonotone_cycles_are_not_success() {
    let rows = [
        Row {
            jacobian: vec![1.],
            offset: 0.,
        },
        Row {
            jacobian: vec![1.],
            offset: 0.001,
        },
    ];
    let forces = [vec![1.], vec![1.]];
    let r = solve(
        Problem {
            derivative: &[1.],
            gradient: &[1.],
            forces: &forces,
            rows: &rows,
        },
        &[true, true],
        8,
    )
    .unwrap();
    assert!(!r.converged);
    assert!(r.gap_violation_m > 1e-5);
    // No complementarity solution: inactive gives penetration, active gives
    // a negative reaction. A cycle must be reported rather than hidden.
    assert!(
        solve(
            Problem {
                derivative: &[-1.],
                gradient: &[-1.],
                forces: &forces[..1],
                rows: &rows[..1]
            },
            &[false],
            8
        )
        .is_err()
    );
    assert!(
        solve(
            Problem {
                derivative: &[1.],
                gradient: &[1.],
                forces: &forces[..1],
                rows: &rows[..1]
            },
            &[false],
            1
        )
        .is_err()
    );
}

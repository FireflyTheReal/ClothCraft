use glam::DVec3;
use marvelous_cloth::solver_lab::{contact_newton, dense_contact::Row};

#[derive(Clone, Copy)]
pub enum Method {
    Strict,
    Rank,
    Unilateral,
    Natural,
    Bounded,
    Constrained,
}

/// Finite differences are an expensive small-fixture oracle, not a scalable
/// derivative implementation. Select the physical QP's positive reaction set;
/// never include path-repair rows in this force solve.
pub fn direction(
    x: &[DVec3],
    history: &[f64],
    target: &[f64],
    rows: &[(usize, Row)],
    method: Method,
    capture: &mut Option<serde_json::Value>,
    residual: impl Fn(&[DVec3], &[f64]) -> Result<Vec<f64>, String>,
) -> Result<(Vec<DVec3>, Vec<f64>), String> {
    direction_with_search_rows(x, history, target, rows, method, capture, &[], residual)
}

#[allow(clippy::too_many_arguments)]
pub fn direction_with_search_rows(
    x: &[DVec3],
    history: &[f64],
    target: &[f64],
    rows: &[(usize, Row)],
    method: Method,
    capture: &mut Option<serde_json::Value>,
    search_rows: &[Row],
    residual: impl Fn(&[DVec3], &[f64]) -> Result<Vec<f64>, String>,
) -> Result<(Vec<DVec3>, Vec<f64>), String> {
    if !search_rows.is_empty() && !matches!(method, Method::Constrained) {
        return Err("Search-path rows require constrained Newton".into());
    }
    if matches!(
        method,
        Method::Unilateral | Method::Natural | Method::Bounded | Method::Constrained
    ) {
        return unilateral_direction(
            x,
            history,
            target,
            rows,
            capture,
            residual,
            method,
            search_rows,
        );
    }
    let active: Vec<_> = rows.iter().filter(|(id, _)| target[*id] > 0.).collect();
    let dofs = 3 * x.len();
    let dim = dofs + active.len();
    if dim > 384 {
        return Err("Newton fixture size exceeded".into());
    }
    let mut base = vec![0.; history.len()];
    for (id, _) in &active {
        base[*id] = history[*id];
    }
    let balance = residual(x, &base)?;
    let mut rhs: Vec<_> = balance.iter().map(|v| -v).collect();
    rhs.extend(active.iter().map(|(_, r)| r.offset));
    let mut matrix = vec![0.; dim * dim];
    for k in 0..dofs {
        let h = 1e-7 * x[k / 3][k % 3].abs().max(1.);
        let mut plus = x.to_vec();
        let mut minus = x.to_vec();
        plus[k / 3][k % 3] += h;
        minus[k / 3][k % 3] -= h;
        let rp = residual(&plus, &base)?;
        let rm = residual(&minus, &base)?;
        for i in 0..dofs {
            matrix[i * dim + k] = (rp[i] - rm[i]) / (2. * h);
        }
    }
    for (j, (id, row)) in active.iter().enumerate() {
        let mut unit = base.clone();
        unit[*id] += 1.;
        let perturbed = residual(x, &unit)?;
        for i in 0..dofs {
            matrix[i * dim + dofs + j] = perturbed[i] - balance[i];
            matrix[(dofs + j) * dim + i] = row.jacobian[i];
        }
    }
    let delta = if matches!(method, Method::Rank) {
        // Match the force/gap residual scales used by globalization. These
        // numerical scales do not change final physical convergence gates.
        let mut scaled = matrix.clone();
        let mut scaled_rhs = rhs.clone();
        for i in 0..dim {
            let scale = if i < dofs { 1e-7 } else { 1e-10 };
            for j in 0..dim {
                scaled[i * dim + j] /= scale;
            }
            scaled_rhs[i] /= scale;
        }
        let report = contact_newton::least_squares(&scaled, &scaled_rhs, dim, 1e-12)?;
        if capture.is_none() && report.rank < dim {
            *capture = Some(serde_json::json!({"matrix":matrix,"rhs":rhs,"dofs":dofs,
                "active_ids":active.iter().map(|(id,_)|id).collect::<Vec<_>>(),
                "base_multipliers":base,"x":x,"reason":"Rank-deficient least-squares proposal",
                "linear_report":report}));
        }
        report.solution
    } else {
        match contact_newton::solve(matrix.clone(), rhs.clone()) {
            Ok(delta) => delta,
            Err(error) => {
                if capture.is_none() {
                    *capture = Some(serde_json::json!({"matrix":matrix,"rhs":rhs,"dofs":dofs,
                        "active_ids":active.iter().map(|(id,_)|id).collect::<Vec<_>>(),
                        "base_multipliers":base,"x":x,"reason":error}));
                }
                return Err(error);
            }
        }
    };
    let mut lambda = base;
    for (j, (id, _)) in active.iter().enumerate() {
        lambda[*id] += delta[dofs + j];
        if lambda[*id] < 0. {
            return Err("Newton reaction leaves nonnegative cone".into());
        }
    }
    Ok((
        delta[..dofs]
            .chunks_exact(3)
            .map(DVec3::from_slice)
            .collect(),
        lambda,
    ))
}

#[allow(clippy::too_many_arguments)]
fn unilateral_direction(
    x: &[DVec3],
    history: &[f64],
    target: &[f64],
    rows: &[(usize, Row)],
    capture: &mut Option<serde_json::Value>,
    residual: impl Fn(&[DVec3], &[f64]) -> Result<Vec<f64>, String>,
    method: Method,
    search_rows: &[Row],
) -> Result<(Vec<DVec3>, Vec<f64>), String> {
    use marvelous_cloth::solver_lab::unilateral_newton::{Problem, solve};
    let n = 3 * x.len();
    let balance = residual(x, history)?;
    let gradient = residual(x, &vec![0.; history.len()])?;
    let mut derivative = vec![0.; n * n];
    for k in 0..n {
        let h = 1e-7 * x[k / 3][k % 3].abs().max(1.);
        let mut plus = x.to_vec();
        let mut minus = x.to_vec();
        plus[k / 3][k % 3] += h;
        minus[k / 3][k % 3] -= h;
        let rp = residual(&plus, history)?;
        let rm = residual(&minus, history)?;
        for i in 0..n {
            derivative[i * n + k] = (rp[i] - rm[i]) / (2. * h);
        }
    }
    let mut forces = vec![];
    for (id, _) in rows {
        let mut unit = history.to_vec();
        unit[*id] += 1.;
        let perturbed = residual(x, &unit)?;
        forces.push(balance.iter().zip(perturbed).map(|(b, r)| b - r).collect());
    }
    let geometry: Vec<_> = rows.iter().map(|(_, r)| r.clone()).collect();
    if matches!(
        method,
        Method::Natural | Method::Bounded | Method::Constrained
    ) {
        let selected: Vec<_> = rows.iter().map(|(id, _)| history[*id]).collect();
        let problem = Problem {
            derivative: &derivative,
            gradient: &gradient,
            forces: &forces,
            rows: &geometry,
        };
        let (dx, reactions) = if matches!(method, Method::Constrained) {
            let report = marvelous_cloth::solver_lab::unilateral_newton::constrained_direction_with_search_rows(
                problem, &selected, search_rows,
            )?;
            *capture = Some(
                serde_json::json!({"reason":"Geometrically constrained residual search",
                "derivative":derivative,"gradient":gradient,"forces":forces,"history":selected,
                "rows":geometry.iter().map(|r|serde_json::json!({"jacobian":r.jacobian,"offset":r.offset})).collect::<Vec<_>>(),
                "search_rows":search_rows.iter().map(|r|serde_json::json!({"jacobian":r.jacobian,"offset":r.offset})).collect::<Vec<_>>(),
                "report":report}),
            );
            if report.solution[n..].iter().any(|v| *v < -1e-11) {
                return Err("Constrained proposal violates reaction cone".into());
            }
            // Remove numerical negative bound slack in search reactions only.
            // Actual force, merit and swept geometry are re-evaluated afterward.
            (
                report.solution[..n].to_vec(),
                report.solution[n..].iter().map(|v| v.max(0.)).collect(),
            )
        } else if matches!(method, Method::Bounded) {
            let report = marvelous_cloth::solver_lab::unilateral_newton::bounded_direction(
                problem, &selected,
            )?;
            // Keep the current bounded linearization so a failed nonlinear
            // iteration can be checked independently, not only the first one.
            *capture = Some(
                serde_json::json!({"reason":"Bounded natural-residual search",
                    "derivative":derivative,"gradient":gradient,"forces":forces,"history":selected,
                    "rows":geometry.iter().map(|r|serde_json::json!({"jacobian":r.jacobian,"offset":r.offset})).collect::<Vec<_>>(),
                    "report":report}),
            );
            (report.solution[..n].to_vec(), report.solution[n..].to_vec())
        } else {
            marvelous_cloth::solver_lab::unilateral_newton::natural_direction(problem, &selected)?
        };
        let mut lambda = vec![0.; history.len()];
        for ((id, _), l) in rows.iter().zip(reactions) {
            lambda[*id] = l;
        }
        return Ok((dx.chunks_exact(3).map(DVec3::from_slice).collect(), lambda));
    }
    let initial: Vec<_> = rows.iter().map(|(id, _)| target[*id] > 0.).collect();
    let report = solve(
        Problem {
            derivative: &derivative,
            gradient: &gradient,
            forces: &forces,
            rows: &geometry,
        },
        &initial,
        128,
    );
    if capture.is_none()
        && report
            .as_ref()
            .map_or(true, |r| r.releases > 0 || !r.converged)
    {
        *capture = Some(
            serde_json::json!({"reason":"Unilateral working-set diagnostic",
            "derivative":derivative,"gradient":gradient,"forces":forces,"initial":initial,
            "rows":geometry.iter().map(|r|serde_json::json!({"jacobian":r.jacobian,"offset":r.offset})).collect::<Vec<_>>(),
            "report":report.as_ref().ok(),"error":report.as_ref().err()}),
        );
    }
    let report = report?;
    let mut lambda = vec![0.; history.len()];
    for ((id, _), l) in rows.iter().zip(report.multipliers) {
        lambda[*id] = l;
    }
    Ok((
        report
            .displacement
            .chunks_exact(3)
            .map(DVec3::from_slice)
            .collect(),
        lambda,
    ))
}

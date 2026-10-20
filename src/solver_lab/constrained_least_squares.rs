//! Dense research oracle for min ||A x-b|| with C x >= d.
//! Feasible primal working sets, null-space least squares, no normal equations.
use super::{
    contact_newton,
    contact_qp::Settings,
    dense_contact::{self, DualMethod, Row},
};

#[derive(Debug, serde::Serialize)]
pub struct Report {
    pub solution: Vec<f64>,
    pub residual_norm: f64,
    pub normalized_feasibility: f64,
    pub scaled_stationarity: f64,
    pub iterations: usize,
    pub phase_one_sweeps: usize,
    pub converged: bool,
}
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}
fn norm(a: &[f64]) -> f64 {
    a.iter().copied().fold(0., f64::hypot)
}
fn orthogonalize(v: &mut [f64], basis: &[Vec<f64>]) {
    for _ in 0..2 {
        for q in basis {
            let c = dot(v, q);
            for (v, q) in v.iter_mut().zip(q) {
                *v -= c * q;
            }
        }
    }
}
fn equality_minimum(
    a: &[f64],
    b: &[f64],
    n: usize,
    rows: &[Row],
    active: &[usize],
) -> Result<Vec<f64>, String> {
    let mut basis: Vec<Vec<f64>> = vec![];
    let mut particular = vec![0.; n];
    for &id in active {
        let row = &rows[id];
        let mut v = row.jacobian.clone();
        orthogonalize(&mut v, &basis);
        let length = norm(&v);
        let rhs = row.offset - dot(&row.jacobian, &particular);
        if length < 1e-11 {
            if rhs.abs() > 1e-11 {
                return Err("Inconsistent constrained working set".into());
            }
            continue;
        }
        for v in &mut v {
            *v /= length;
        }
        for (x, v) in particular.iter_mut().zip(&v) {
            *x += rhs / length * v;
        }
        basis.push(v);
    }
    let rank = basis.len();
    let mut null = vec![];
    for i in 0..n {
        let mut v = vec![0.; n];
        v[i] = 1.;
        orthogonalize(&mut v, &basis);
        let length = norm(&v);
        if length > 1e-10 {
            for v in &mut v {
                *v /= length;
            }
            basis.push(v.clone());
            null.push(v);
        }
    }
    if rank + null.len() != n {
        return Err("Incomplete constrained null space".into());
    }
    if !null.is_empty() {
        let k = null.len();
        let reduced: Vec<_> = a
            .chunks_exact(n)
            .flat_map(|row| null.iter().map(move |q| dot(row, q)))
            .collect();
        let rhs: Vec<_> = a
            .chunks_exact(n)
            .zip(b)
            .map(|(row, b)| b - dot(row, &particular))
            .collect();
        let ls = contact_newton::least_squares(&reduced, &rhs, k, 1e-12)?;
        for (q, y) in null.iter().zip(ls.solution) {
            for (x, q) in particular.iter_mut().zip(q) {
                *x += y * q;
            }
        }
    }
    if active
        .iter()
        .any(|&i| (dot(&rows[i].jacobian, &particular) - rows[i].offset).abs() > 1e-10)
    {
        return Err("Constrained equality residual too large".into());
    }
    Ok(particular)
}
pub fn solve(
    a: &[f64],
    b: &[f64],
    n: usize,
    rows: &[Row],
    initial: &[f64],
    budget: usize,
) -> Result<Report, String> {
    if n == 0
        || n > 192
        || b.is_empty()
        || b.len() > 384
        || a.len() != n * b.len()
        || initial.len() != n
        || budget == 0
        || rows.len() > 1024
        || a.iter().chain(b).chain(initial).any(|v| !v.is_finite())
        || rows.iter().any(|r| {
            r.jacobian.len() != n
                || !r.offset.is_finite()
                || r.jacobian.iter().any(|v| !v.is_finite())
        })
    {
        return Err("Invalid constrained least-squares inputs".into());
    }
    let normalized: Vec<_> = rows
        .iter()
        .map(|r| {
            let length = norm(&r.jacobian);
            if length == 0. || !length.is_finite() {
                return Err("Invalid constrained row norm".to_string());
            }
            Ok(Row {
                jacobian: r.jacobian.iter().map(|v| v / length).collect(),
                offset: r.offset / length,
            })
        })
        .collect::<Result<_, _>>()?;
    let rows = &normalized;
    let scale = a.iter().map(|v| v.abs()).fold(0., f64::max).max(1e-150);
    let scaled: Vec<_> = a.iter().map(|v| v / scale).collect();
    let rhs: Vec<_> = b.iter().map(|v| v / scale).collect();
    if rhs.iter().any(|v| !v.is_finite()) {
        return Err("Constrained scaling overflow".into());
    }
    let mut identity = vec![0.; n * n];
    for i in 0..n {
        identity[i * n + i] = 1.;
    }
    let g: Vec<_> = initial.iter().map(|v| -v).collect();
    let phase = dense_contact::solve_with_method(
        &identity,
        &g,
        rows,
        Settings {
            max_sweeps: 4096,
            feasibility_m: 1e-12,
            stationarity_n: 1e-10,
            complementarity_j: 1e-12,
        },
        DualMethod::IndependentPivotBlock,
    )?;
    if !phase.converged {
        return Err("Constrained feasible-start search failed".into());
    }
    let mut x = phase.displacement;
    let mut active: Vec<usize> = vec![];
    for iteration in 0..budget {
        let trial = equality_minimum(&scaled, &rhs, n, rows, &active)?;
        let direction: Vec<_> = trial.iter().zip(&x).map(|(a, b)| a - b).collect();
        if norm(&direction) > 1e-12 * (1. + norm(&x)) {
            let mut alpha: f64 = 1.;
            let mut blocker = None;
            for (id, row) in rows.iter().enumerate() {
                if active.contains(&id) {
                    continue;
                }
                let slope = dot(&row.jacobian, &direction);
                if slope < -1e-14 * norm(&direction) {
                    let bound = ((dot(&row.jacobian, &x) - row.offset).max(0.) / -slope).min(1.);
                    if bound < alpha {
                        alpha = bound;
                        blocker = Some(id);
                    }
                }
            }
            for (x, d) in x.iter_mut().zip(&direction) {
                *x += alpha * d;
            }
            if let Some(id) = blocker {
                active.push(id);
            }
            continue;
        }
        let residual: Vec<_> = scaled
            .chunks_exact(n)
            .zip(&rhs)
            .map(|(r, b)| dot(r, &x) - b)
            .collect();
        let gradient: Vec<_> = (0..n)
            .map(|j| {
                (0..b.len())
                    .map(|i| scaled[i * n + j] * residual[i])
                    .sum::<f64>()
            })
            .collect();
        let gradient_norm = norm(&gradient);
        let matrix_norm = norm(&scaled);
        let tolerance = 1e-10 * gradient_norm
            + 256. * f64::EPSILON * matrix_norm * (matrix_norm * norm(&x) + norm(&rhs));
        let mut stationarity = gradient.clone();
        if !active.is_empty() {
            let dual_matrix: Vec<_> = (0..n)
                .flat_map(|i| active.iter().map(move |&j| rows[j].jacobian[i]))
                .collect();
            let dual = contact_newton::least_squares(&dual_matrix, &gradient, active.len(), 1e-12)?;
            if let Some((k, _)) = dual
                .solution
                .iter()
                .enumerate()
                .filter(|(_, v)| **v < -tolerance)
                .min_by(|a, b| a.1.total_cmp(b.1))
            {
                active.remove(k);
                continue;
            }
            for (&id, lambda) in active.iter().zip(dual.solution) {
                for (s, c) in stationarity.iter_mut().zip(&rows[id].jacobian) {
                    *s -= lambda * c;
                }
            }
        }
        let feasibility = rows
            .iter()
            .map(|r| (r.offset - dot(&r.jacobian, &x)).max(0.))
            .fold(0., f64::max);
        let residual_norm = norm(
            &a.chunks_exact(n)
                .zip(b)
                .map(|(r, b)| dot(r, &x) - b)
                .collect::<Vec<_>>(),
        );
        let stationarity = norm(&stationarity);
        if !residual_norm.is_finite() || !stationarity.is_finite() || !feasibility.is_finite() {
            return Err("Nonfinite constrained result".into());
        }
        return Ok(Report {
            solution: x,
            residual_norm,
            normalized_feasibility: feasibility,
            scaled_stationarity: stationarity,
            iterations: iteration + 1,
            phase_one_sweeps: phase.sweeps,
            converged: feasibility <= 1e-11 && stationarity <= tolerance,
        });
    }
    Err("Constrained least-squares iteration budget exhausted".into())
}

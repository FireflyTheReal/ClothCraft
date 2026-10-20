//! Small linearized contact complementarity search. Force and geometric
//! columns need not be transposes. No global convergence guarantee is assumed.
use super::{contact_newton, dense_contact::Row};

pub struct Problem<'a> {
    pub derivative: &'a [f64],
    pub gradient: &'a [f64],
    /// Positive reaction force columns, one for every geometric row.
    pub forces: &'a [Vec<f64>],
    pub rows: &'a [Row],
}
#[derive(Debug, serde::Serialize)]
pub struct Report {
    pub displacement: Vec<f64>,
    pub multipliers: Vec<f64>,
    pub pivots: usize,
    pub releases: usize,
    pub activations: usize,
    pub rank: usize,
    pub force_residual_n: f64,
    pub gap_violation_m: f64,
    pub complementarity_j: f64,
    pub converged: bool,
}
fn validate_natural(p: &Problem<'_>, history: &[f64]) -> Result<(), String> {
    let n = p.gradient.len();
    let m = p.rows.len();
    if n == 0
        || n + m > 384
        || p.derivative.len() != n * n
        || p.forces.len() != m
        || history.len() != m
        || history.iter().any(|v| !v.is_finite() || *v < 0.)
        || p.derivative
            .iter()
            .chain(p.gradient)
            .any(|v| !v.is_finite())
        || p.forces
            .iter()
            .any(|f| f.len() != n || f.iter().any(|v| !v.is_finite()))
        || p.rows.iter().any(|r| {
            r.jacobian.len() != n
                || !r.offset.is_finite()
                || r.jacobian.iter().any(|v| !v.is_finite())
        })
    {
        return Err("Invalid natural contact linearization".into());
    }
    Ok(())
}

/// One generalized Newton direction for the exact natural residual used by
/// globalization: min(lambda / 1e-7, gap / 1e-10). Unlike the LCP pivot search,
/// the branch is chosen at the current iterate, not at a proposed endpoint.
/// The returned multipliers may be negative: the caller must restrict the
/// step to the nonnegative cone and run its nonlinear/swept acceptance gates.
pub fn natural_direction(p: Problem<'_>, history: &[f64]) -> Result<(Vec<f64>, Vec<f64>), String> {
    validate_natural(&p, history)?;
    let n = p.gradient.len();
    let m = p.rows.len();
    let active: Vec<_> = (0..m)
        .filter(|&k| history[k] / 1e-7 > -p.rows[k].offset / 1e-10)
        .collect();
    let dim = n + active.len();
    let mut a = vec![0.; dim * dim];
    let mut b = vec![0.; dim];
    for i in 0..n {
        for j in 0..n {
            a[i * dim + j] = p.derivative[i * n + j] / 1e-7;
        }
        b[i] = -p.gradient[i] / 1e-7;
    }
    for (j, &k) in active.iter().enumerate() {
        let row = &p.rows[k];
        for i in 0..n {
            a[i * dim + n + j] = -p.forces[k][i] / 1e-7;
            a[(n + j) * dim + i] = row.jacobian[i] / 1e-10;
        }
        b[n + j] = row.offset / 1e-10;
    }
    let ls = contact_newton::least_squares(&a, &b, dim, 1e-12)?;
    let mut multipliers = vec![0.; m];
    for (j, &k) in active.iter().enumerate() {
        multipliers[k] = ls.solution[n + j];
    }
    Ok((ls.solution[..n].to_vec(), multipliers))
}

/// Least-squares minimization of the same linearized natural residual, with
/// nonnegative new reactions. Geometry still needs nonlinear swept validation.
/// Inactive reaction branches remain residual rows, not forced eliminations.
pub fn bounded_direction(
    p: Problem<'_>,
    history: &[f64],
) -> Result<super::bounded_least_squares::Report, String> {
    let s = natural_system(&p, history)?;
    super::bounded_least_squares::solve(
        &s.a,
        &s.b,
        s.initial.len(),
        p.gradient.len(),
        &s.initial,
        512,
    )
}

/// Constrain both new reactions and endpoint-linearized geometric gaps.
/// The nonlinear solver must still certify the actual swept trajectory.
pub fn constrained_direction(
    p: Problem<'_>,
    history: &[f64],
) -> Result<super::constrained_least_squares::Report, String> {
    constrained_direction_with_search_rows(p, history, &[])
}

/// Extra inequalities constrain the proposed path only. They must not acquire
/// physical reaction variables or enter the natural-complementarity residual.
/// Their linearization is not a swept safety certificate.
pub fn constrained_direction_with_search_rows(
    p: Problem<'_>,
    history: &[f64],
    search_rows: &[Row],
) -> Result<super::constrained_least_squares::Report, String> {
    let s = natural_system(&p, history)?;
    let n = p.gradient.len();
    let dim = s.initial.len();
    if search_rows.iter().any(|r| {
        r.jacobian.len() != n || !r.offset.is_finite() || r.jacobian.iter().any(|v| !v.is_finite())
    }) {
        return Err("Invalid contact search-path row".into());
    }
    let mut rows = p.rows.to_vec();
    rows.extend_from_slice(search_rows);
    for r in &mut rows {
        r.jacobian.resize(dim, 0.);
    }
    for i in n..dim {
        let mut j = vec![0.; dim];
        j[i] = 1.;
        rows.push(Row {
            jacobian: j,
            offset: 0.,
        });
    }
    super::constrained_least_squares::solve(&s.a, &s.b, dim, &rows, &s.initial, 512)
}

struct NaturalSystem {
    a: Vec<f64>,
    b: Vec<f64>,
    initial: Vec<f64>,
}
fn natural_system(p: &Problem<'_>, history: &[f64]) -> Result<NaturalSystem, String> {
    validate_natural(p, history)?;
    let n = p.gradient.len();
    let m = p.rows.len();
    let dim = n + m;
    let mut a = vec![0.; dim * dim];
    let mut b = vec![0.; dim];
    for i in 0..n {
        for j in 0..n {
            a[i * dim + j] = p.derivative[i * n + j] / 1e-7;
        }
        b[i] = -p.gradient[i] / 1e-7;
    }
    for (k, row) in p.rows.iter().enumerate() {
        for i in 0..n {
            a[i * dim + n + k] = -p.forces[k][i] / 1e-7;
        }
        if history[k] / 1e-7 > -row.offset / 1e-10 {
            for i in 0..n {
                a[(n + k) * dim + i] = row.jacobian[i] / 1e-10;
            }
            b[n + k] = row.offset / 1e-10;
        } else {
            a[(n + k) * dim + n + k] = 1. / 1e-7;
        }
    }
    let mut initial = vec![0.; dim];
    initial[n..].copy_from_slice(history);
    Ok(NaturalSystem { a, b, initial })
}
/// Equality working sets are trial models only. Release a negative reaction;
/// activate an inactive violated geometric row. Every row remains in the final
/// residual. Detect cycles and exhaustion instead of claiming convergence.
pub fn solve(p: Problem<'_>, initial: &[bool], max_pivots: usize) -> Result<Report, String> {
    let n = p.gradient.len();
    let m = p.rows.len();
    if n == 0
        || n > 192
        || m > 1024
        || p.derivative.len() != n * n
        || p.forces.len() != m
        || initial.len() != m
        || max_pivots == 0
        || p.derivative
            .iter()
            .chain(p.gradient)
            .any(|v| !v.is_finite())
        || p.forces
            .iter()
            .any(|v| v.len() != n || v.iter().any(|v| !v.is_finite()))
        || p.rows.iter().any(|r| {
            r.jacobian.len() != n
                || !r.offset.is_finite()
                || r.jacobian.iter().any(|v| !v.is_finite())
        })
    {
        return Err("Invalid unilateral Newton problem".into());
    }
    let mut active = initial.to_vec();
    let mut visited = std::collections::HashSet::new();
    let (mut releases, mut activations) = (0, 0);
    for pivot in 0..max_pivots {
        if !visited.insert(active.clone()) {
            return Err("Unilateral Newton active-set cycle".into());
        }
        let ids: Vec<_> = (0..m).filter(|&i| active[i]).collect();
        let dim = n + ids.len();
        if dim > 384 {
            return Err("Unilateral Newton working set exceeds dense limit".into());
        }
        let mut a = vec![0.; dim * dim];
        let mut b = vec![0.; dim];
        for i in 0..n {
            for j in 0..n {
                a[i * dim + j] = p.derivative[i * n + j] / 1e-7;
            }
            b[i] = -p.gradient[i] / 1e-7;
        }
        for (j, &id) in ids.iter().enumerate() {
            for i in 0..n {
                a[i * dim + n + j] = -p.forces[id][i] / 1e-7;
                a[(n + j) * dim + i] = p.rows[id].jacobian[i] / 1e-10;
            }
            b[n + j] = p.rows[id].offset / 1e-10;
        }
        let ls = contact_newton::least_squares(&a, &b, dim, 1e-12)?;
        let mut multipliers = vec![0.; m];
        for (j, &id) in ids.iter().enumerate() {
            multipliers[id] = ls.solution[n + j];
        }
        if let Some(id) = ids
            .iter()
            .copied()
            .filter(|&i| multipliers[i] < 0.)
            .min_by(|&i, &j| multipliers[i].total_cmp(&multipliers[j]))
        {
            active[id] = false;
            releases += 1;
            continue;
        }
        let dx = ls.solution[..n].to_vec();
        let gaps: Vec<_> = p
            .rows
            .iter()
            .map(|r| r.jacobian.iter().zip(&dx).map(|(g, d)| g * d).sum::<f64>() - r.offset)
            .collect();
        if let Some(id) = (0..m)
            .filter(|&i| !active[i] && gaps[i] < -1e-11)
            .min_by(|&i, &j| gaps[i].total_cmp(&gaps[j]))
        {
            active[id] = true;
            activations += 1;
            continue;
        }
        let force_residual_n = (0..n)
            .map(|i| {
                let mut r = p.gradient[i]
                    + p.derivative[i * n..(i + 1) * n]
                        .iter()
                        .zip(&dx)
                        .map(|(a, d)| a * d)
                        .sum::<f64>();
                for (f, l) in p.forces.iter().zip(&multipliers) {
                    r -= f[i] * l;
                }
                r
            })
            .fold(0., f64::hypot);
        let gap_violation_m = gaps.iter().map(|g| (-g).max(0.)).fold(0., f64::max);
        let complementarity_j = gaps
            .iter()
            .zip(&multipliers)
            .map(|(g, l)| (g * l).abs())
            .fold(0., f64::max);
        if !force_residual_n.is_finite()
            || !gap_violation_m.is_finite()
            || !complementarity_j.is_finite()
        {
            return Err("Nonfinite unilateral Newton residual".into());
        }
        return Ok(Report {
            displacement: dx,
            multipliers,
            pivots: pivot,
            releases,
            activations,
            rank: ls.rank,
            force_residual_n,
            gap_violation_m,
            complementarity_j,
            converged: force_residual_n <= 1e-7
                && gap_violation_m <= 1e-11
                && complementarity_j <= 1e-12,
        });
    }
    Err("Unilateral Newton pivot budget exhausted".into())
}

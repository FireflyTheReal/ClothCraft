//! Dense research least squares with free leading variables and nonnegative
//! trailing variables. Uses QR subproblems, never normal equations.
use super::contact_newton;

#[derive(Debug, serde::Serialize)]
pub struct Report {
    pub solution: Vec<f64>,
    pub residual_norm: f64,
    /// KKT norm in column-normalized variables, not a physical force residual.
    pub projected_gradient: f64,
    pub iterations: usize,
    pub converged: bool,
}

pub fn solve(
    a: &[f64],
    b: &[f64],
    columns: usize,
    free_prefix: usize,
    initial: &[f64],
    max_iterations: usize,
) -> Result<Report, String> {
    let rows = b.len();
    if columns == 0
        || columns > 384
        || rows == 0
        || rows > 384
        || free_prefix > columns
        || a.len() != rows * columns
        || initial.len() != columns
        || max_iterations == 0
        || a.iter().chain(b).chain(initial).any(|v| !v.is_finite())
        || initial[free_prefix..].iter().any(|v| *v < 0.)
    {
        return Err("Invalid bounded least-squares inputs".into());
    }
    let scale: Vec<_> = (0..columns)
        .map(|j| {
            (0..rows)
                .map(|i| a[i * columns + j])
                .fold(0., f64::hypot)
                .max(1e-150)
        })
        .collect();
    let normalized: Vec<_> = a
        .iter()
        .enumerate()
        .map(|(k, v)| v / scale[k % columns])
        .collect();
    let mut z: Vec<_> = initial.iter().zip(&scale).map(|(v, s)| v * s).collect();
    if scale.iter().chain(&z).any(|v| !v.is_finite()) {
        return Err("Bounded least-squares scaling overflow".into());
    }
    let mut free: Vec<_> = (0..columns).map(|i| i < free_prefix || z[i] > 0.).collect();
    let rhs_norm = b.iter().copied().fold(1., f64::hypot);
    if !rhs_norm.is_finite() {
        return Err("Bounded least-squares residual scale overflow".into());
    }
    let tolerance = 1e-10 * rhs_norm;
    for iteration in 0..max_iterations {
        let ids: Vec<_> = (0..columns).filter(|&j| free[j]).collect();
        let mut trial = vec![0.; columns];
        if !ids.is_empty() {
            let mut reduced = Vec::with_capacity(rows * ids.len());
            for i in 0..rows {
                reduced.extend(ids.iter().map(|&j| normalized[i * columns + j]));
            }
            let ls = contact_newton::least_squares(&reduced, b, ids.len(), 1e-12)?;
            for (&j, value) in ids.iter().zip(ls.solution) {
                trial[j] = value;
            }
        }
        let blocked = (free_prefix..columns)
            .filter(|&j| free[j] && trial[j] < 0.)
            .map(|j| (j, z[j] / (z[j] - trial[j])))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((j, alpha)) = blocked {
            for k in 0..columns {
                z[k] += alpha * (trial[k] - z[k]);
                // Convex interpolation is feasible algebraically; remove only
                // floating-point negative roundoff in bounded search variables.
                if k >= free_prefix {
                    z[k] = z[k].max(0.);
                }
            }
            z[j] = 0.;
            free[j] = false;
            continue;
        }
        z = trial;
        let residual: Vec<_> = (0..rows)
            .map(|i| {
                normalized[i * columns..(i + 1) * columns]
                    .iter()
                    .zip(&z)
                    .map(|(v, x)| v * x)
                    .sum::<f64>()
                    - b[i]
            })
            .collect();
        let gradient: Vec<_> = (0..columns)
            .map(|j| {
                (0..rows)
                    .map(|i| normalized[i * columns + j] * residual[i])
                    .sum::<f64>()
            })
            .collect();
        if residual.iter().chain(&gradient).any(|v| !v.is_finite()) {
            return Err("Nonfinite bounded least-squares residual".into());
        }
        if let Some(j) = (free_prefix..columns)
            .filter(|&j| !free[j] && gradient[j] < -tolerance)
            .min_by(|&i, &j| gradient[i].total_cmp(&gradient[j]))
        {
            free[j] = true;
            continue;
        }
        let projected_gradient = gradient
            .iter()
            .enumerate()
            .map(|(j, g)| {
                if j < free_prefix || z[j] > 0. {
                    g.abs()
                } else {
                    (-g).max(0.)
                }
            })
            .fold(0., f64::max);
        let solution: Vec<_> = z.iter().zip(&scale).map(|(v, s)| v / s).collect();
        if solution.iter().any(|v| !v.is_finite()) {
            return Err("Bounded least-squares solution overflow".into());
        }
        return Ok(Report {
            solution,
            residual_norm: residual.into_iter().fold(0., f64::hypot),
            projected_gradient,
            iterations: iteration + 1,
            converged: projected_gradient <= tolerance,
        });
    }
    Err("Bounded least-squares iteration budget exhausted".into())
}

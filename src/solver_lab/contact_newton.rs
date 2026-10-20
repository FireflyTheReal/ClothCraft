//! Small nonsymmetric Newton oracle. Research only; no diagonal regularization.
//! Geometry and force derivatives are separate blocks. Callers must globalize
//! the result and independently certify every accepted geometric path.

/// Row-scaled Gaussian elimination with partial pivoting. Reject ill-conditioned
/// pivots instead of changing the equations. At most 384 unknowns.
pub fn solve(mut a: Vec<f64>, mut b: Vec<f64>) -> Result<Vec<f64>, String> {
    let n = b.len();
    if n == 0 || n > 384 || a.len() != n * n || a.iter().chain(&b).any(|v| !v.is_finite()) {
        return Err("Invalid nonsymmetric linear system".into());
    }
    for i in 0..n {
        let scale = a[i * n..(i + 1) * n]
            .iter()
            .map(|v| v.abs())
            .fold(0., f64::max);
        if scale == 0. {
            return Err("Zero Newton row".into());
        }
        for j in 0..n {
            a[i * n + j] /= scale;
        }
        b[i] /= scale;
    }
    for k in 0..n {
        let pivot = (k..n)
            .max_by(|&i, &j| a[i * n + k].abs().total_cmp(&a[j * n + k].abs()))
            .unwrap();
        if a[pivot * n + k].abs() < 1e-12 {
            return Err("Rank-deficient Newton system".into());
        }
        for j in k..n {
            a.swap(k * n + j, pivot * n + j);
        }
        b.swap(k, pivot);
        for i in k + 1..n {
            let f = a[i * n + k] / a[k * n + k];
            a[i * n + k] = 0.;
            for j in k + 1..n {
                a[i * n + j] -= f * a[k * n + j];
            }
            b[i] -= f * b[k];
        }
    }
    for i in (0..n).rev() {
        for j in i + 1..n {
            b[i] -= a[i * n + j] * b[j];
        }
        b[i] /= a[i * n + i];
    }
    if b.iter().any(|v| !v.is_finite()) {
        return Err("Nonfinite Newton solution".into());
    }
    Ok(b)
}

#[derive(Debug, serde::Serialize)]
pub struct LeastSquares {
    pub solution: Vec<f64>,
    pub rank: usize,
    /// Residual of ALL original rows, including inconsistent/redundant rows.
    pub residual_norm: f64,
}

fn norm(v: impl Iterator<Item = f64>) -> f64 {
    v.fold(0., f64::hypot)
}

fn reflector(mut v: Vec<f64>) -> Option<Vec<f64>> {
    let length = norm(v.iter().copied());
    if length == 0. {
        return None;
    }
    v[0] += length.copysign(v[0]);
    let length = norm(v.iter().copied());
    for x in &mut v {
        *x /= length;
    }
    Some(v)
}

/// Small minimum-norm least-squares oracle using two Householder QR stages.
/// QR with column pivoting exposes rank; QR of the retained R transpose gives
/// the minimum-norm solution without normal equations. Rank is selected by a
/// relative trailing-column norm threshold, NOT LAPACK's condition estimator.
/// This is a search direction, not proof of an exactly solved contact system.
pub fn least_squares(
    a: &[f64],
    b: &[f64],
    columns: usize,
    relative_rank_tolerance: f64,
) -> Result<LeastSquares, String> {
    let m = b.len();
    let n = columns;
    if m == 0
        || n == 0
        || m > 384
        || n > 384
        || a.len() != m * n
        || a.iter().chain(b).any(|v| !v.is_finite())
        || !relative_rank_tolerance.is_finite()
        || !(0. ..1.).contains(&relative_rank_tolerance)
        || relative_rank_tolerance == 0.
    {
        return Err("Invalid least-squares input".into());
    }
    let scale = a.iter().map(|v| v.abs()).fold(0., f64::max);
    if scale == 0. {
        let residual_norm = norm(b.iter().copied());
        if !residual_norm.is_finite() {
            return Err("Least-squares residual overflow".into());
        }
        return Ok(LeastSquares {
            solution: vec![0.; n],
            rank: 0,
            residual_norm,
        });
    }
    let mut r: Vec<_> = a.iter().map(|v| v / scale).collect();
    let mut qb: Vec<_> = b.iter().map(|v| v / scale).collect();
    if qb.iter().any(|v| !v.is_finite()) {
        return Err("Least-squares scaling overflow".into());
    }
    let mut permutation: Vec<_> = (0..n).collect();
    let mut rank = 0;
    let mut first_norm = 0.;
    for k in 0..m.min(n) {
        let (pivot, length) = (k..n)
            .map(|j| (j, norm((k..m).map(|i| r[i * n + j]))))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        if k == 0 {
            first_norm = length;
        }
        if length <= relative_rank_tolerance * first_norm {
            break;
        }
        for i in 0..m {
            r.swap(i * n + k, i * n + pivot);
        }
        permutation.swap(k, pivot);
        let v = reflector((k..m).map(|i| r[i * n + k]).collect()).ok_or("Missing QR reflector")?;
        for j in k..n {
            let dot: f64 = v
                .iter()
                .enumerate()
                .map(|(i, v)| v * r[(k + i) * n + j])
                .sum();
            for (i, v) in v.iter().enumerate() {
                r[(k + i) * n + j] -= 2. * v * dot;
            }
        }
        let dot: f64 = v.iter().zip(&qb[k..]).map(|(v, b)| v * b).sum();
        for (i, v) in v.iter().enumerate() {
            qb[k + i] -= 2. * v * dot;
        }
        for i in k + 1..m {
            r[i * n + k] = 0.;
        }
        rank += 1;
    }
    // B = R_top^T = Q2 T. Thus R_top z = c becomes T^T y = c,
    // z = Q2 [y;0]. This also handles underdetermined systems.
    let mut transposed = vec![0.; n * rank];
    for i in 0..n {
        for j in 0..rank {
            transposed[i * rank + j] = r[j * n + i];
        }
    }
    let mut reflectors = Vec::with_capacity(rank);
    for k in 0..rank {
        let v = reflector((k..n).map(|i| transposed[i * rank + k]).collect())
            .ok_or("Singular retained QR block")?;
        for j in k..rank {
            let dot: f64 = v
                .iter()
                .enumerate()
                .map(|(i, v)| v * transposed[(k + i) * rank + j])
                .sum();
            for (i, v) in v.iter().enumerate() {
                transposed[(k + i) * rank + j] -= 2. * v * dot;
            }
        }
        reflectors.push(v);
    }
    let mut z = vec![0.; n];
    for i in 0..rank {
        z[i] = qb[i];
        for j in 0..i {
            z[i] -= transposed[j * rank + i] * z[j];
        }
        z[i] /= transposed[i * rank + i];
    }
    for k in (0..rank).rev() {
        let v = &reflectors[k];
        let dot: f64 = v.iter().zip(&z[k..]).map(|(v, z)| v * z).sum();
        for (i, v) in v.iter().enumerate() {
            z[k + i] -= 2. * v * dot;
        }
    }
    let mut solution = vec![0.; n];
    for i in 0..n {
        solution[permutation[i]] = z[i];
    }
    let residual_norm = norm(
        a.chunks_exact(n)
            .zip(b)
            .map(|(row, b)| row.iter().zip(&solution).map(|(a, x)| a * x).sum::<f64>() - b),
    );
    if solution.iter().any(|v| !v.is_finite()) || !residual_norm.is_finite() {
        return Err("Nonfinite least-squares result".into());
    }
    Ok(LeastSquares {
        solution,
        rank,
        residual_norm,
    })
}

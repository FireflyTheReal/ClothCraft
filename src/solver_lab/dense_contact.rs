//! Small coupled contact-QP oracle, not a scalable garment backend.
//! Rows are distance Jacobians across ALL participating degrees of freedom.
//! Their coefficients are not normalized: a multiplier retains force units.
use super::contact_qp::Settings;
use serde::Serialize;

#[derive(Clone)]
pub struct Row {
    pub jacobian: Vec<f64>,
    pub offset: f64,
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum DualMethod {
    Coordinate,
    ActiveBlock,
    /// Conditional solve on independent active rows; all other multipliers and
    /// inequalities remain in the full problem.
    IndependentBlock,
    /// Arithmetic reference for testing the Schur cache.
    IndependentBlockUncached,
    /// Finish conditional block solves after hitting a nonnegative boundary,
    /// before a coordinate sweep can reactivate the just-removed multiplier.
    IndependentPivotBlock,
}
#[derive(Serialize, Debug)]
pub struct Report {
    pub displacement: Vec<f64>,
    pub multipliers: Vec<f64>,
    pub converged: bool,
    pub sweeps: usize,
    pub feasibility_m: f64,
    pub stationarity_n: f64,
    pub complementarity_j: f64,
    pub active_block_steps: usize,
    pub singular_active_blocks: usize,
    pub reduced_active_blocks: usize,
    pub schur_entries_computed: usize,
    pub schur_cache_hits: usize,
    pub schur_cache_bytes: usize,
    pub boundary_pivots: usize,
}
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}
/// Exact lazy storage of J H^-1 J^T within ONE immutable QP. A 1 MiB
/// budget limits allocation; larger systems use the same uncached arithmetic.
struct Schur<'a> {
    rows: &'a [Row],
    directions: &'a [Vec<f64>],
    values: Option<Vec<f64>>,
    computed: usize,
    hits: usize,
}
impl<'a> Schur<'a> {
    fn new(rows: &'a [Row], directions: &'a [Vec<f64>], enabled: bool) -> Self {
        let count = rows
            .len()
            .checked_add(1)
            .and_then(|v| rows.len().checked_mul(v))
            .map(|v| v / 2);
        Self {
            rows,
            directions,
            values: count
                .filter(|n| enabled && *n <= 131_072)
                .map(|n| vec![f64::NAN; n]),
            computed: 0,
            hits: 0,
        }
    }
    fn get(&mut self, i: usize, j: usize) -> f64 {
        let index = self.values.as_ref().map(|_| {
            let hi = i.max(j);
            hi * (hi + 1) / 2 + i.min(j)
        });
        if let Some(index) = index {
            let value = self.values.as_ref().unwrap()[index];
            if !value.is_nan() {
                self.hits += 1;
                return value;
            }
        }
        let value = if i == j {
            dot(&self.rows[i].jacobian, &self.directions[i])
        } else {
            0.5 * (dot(&self.rows[i].jacobian, &self.directions[j])
                + dot(&self.rows[j].jacobian, &self.directions[i]))
        };
        self.computed += 1;
        if let Some(index) = index {
            self.values.as_mut().unwrap()[index] = value;
        }
        value
    }
}
fn solve_factor(l: &[f64], b: &[f64]) -> Vec<f64> {
    let n = b.len();
    let mut x = b.to_vec();
    for i in 0..n {
        for j in 0..i {
            x[i] -= l[i * n + j] * x[j];
        }
        x[i] /= l[i * n + i];
    }
    for i in (0..n).rev() {
        for j in i + 1..n {
            x[i] -= l[j * n + i] * x[j];
        }
        x[i] /= l[i * n + i];
    }
    x
}
pub fn solve(h: &[f64], g: &[f64], rows: &[Row], cfg: Settings) -> Result<Report, String> {
    solve_with_method(h, g, rows, cfg, DualMethod::Coordinate)
}
/// Coupled dual Newton step on positive multipliers. A line to the nonnegative
/// boundary preserves dual feasibility and ascent. Rank-deficient/redundant
/// active sets fall back to coordinate ascent, without regularization.
fn active_block(
    rows: &[Row],
    directions: &[Vec<f64>],
    free: &[f64],
    lambda: &mut [f64],
) -> Result<bool, ()> {
    let ids: Vec<_> = lambda
        .iter()
        .enumerate()
        .filter(|(_, l)| **l > 0.)
        .map(|(i, _)| i)
        .collect();
    let m = ids.len();
    if m < 2 {
        return Ok(false);
    }
    if m > free.len() {
        return Err(());
    }
    let mut l = vec![0.; m * m];
    let mut scale: f64 = 0.;
    for &i in &ids {
        scale = scale.max(dot(&rows[i].jacobian, &directions[i]));
    }
    for (a, &i) in ids.iter().enumerate() {
        for (b, &j) in ids.iter().enumerate().take(a + 1) {
            let mut v = 0.5
                * (dot(&rows[i].jacobian, &directions[j]) + dot(&rows[j].jacobian, &directions[i]));
            for k in 0..b {
                v -= l[a * m + k] * l[b * m + k];
            }
            l[a * m + b] = if a == b {
                if !v.is_finite() || v <= 1e-12 * scale {
                    return Err(());
                }
                v.sqrt()
            } else {
                v / l[b * m + b]
            };
        }
    }
    let rhs: Vec<_> = ids
        .iter()
        .map(|&i| rows[i].offset - dot(&rows[i].jacobian, free))
        .collect();
    let target = solve_factor(&l, &rhs);
    if target.iter().any(|v| !v.is_finite()) {
        return Err(());
    }
    let mut alpha: f64 = 1.;
    for (&i, &t) in ids.iter().zip(&target) {
        if t < 0. {
            alpha = alpha.min(lambda[i] / (lambda[i] - t));
        }
    }
    let updates: Vec<_> = ids
        .iter()
        .zip(&target)
        .map(|(&i, &t)| lambda[i] + alpha * (t - lambda[i]))
        .collect();
    if updates.iter().any(|v| !v.is_finite()) {
        return Err(());
    }
    for (&i, &v) in ids.iter().zip(&updates) {
        lambda[i] = v.max(0.);
    }
    Ok(true)
}
/// Select a numerically independent active block in the H^-1 metric. Skipped
/// multipliers remain fixed during THIS block update; coordinate sweeps and
/// all KKT checks still include them. No geometric constraint is discarded.
fn independent_block(
    rows: &[Row],
    directions: &[Vec<f64>],
    free: &[f64],
    lambda: &mut [f64],
    schur: &mut Schur<'_>,
    pivot_boundary: bool,
) -> Result<Option<(bool, bool)>, ()> {
    let active: Vec<_> = lambda
        .iter()
        .enumerate()
        .filter(|(_, l)| **l > 0.)
        .map(|(i, _)| i)
        .collect();
    if active.is_empty() {
        return Ok(None);
    }
    let mut ids: Vec<usize> = vec![];
    let mut lower: Vec<Vec<f64>> = vec![];
    for &i in &active {
        if ids.len() == free.len() {
            break;
        }
        let diagonal = schur.get(i, i);
        let mut column = vec![];
        for (a, &j) in ids.iter().enumerate() {
            let cross = schur.get(i, j);
            let projected: f64 = lower[a][..a].iter().zip(&column).map(|(a, b)| a * b).sum();
            column.push((cross - projected) / lower[a][a]);
        }
        let pivot = diagonal - dot(&column, &column);
        if !pivot.is_finite() {
            return Err(());
        }
        if pivot <= 1e-12 * diagonal {
            continue;
        }
        column.push(pivot.sqrt());
        lower.push(column);
        ids.push(i);
    }
    if ids.is_empty() {
        return Ok(None);
    }
    let m = ids.len();
    let mut factor = vec![0.; m * m];
    for (i, row) in lower.iter().enumerate() {
        factor[i * m..i * m + row.len()].copy_from_slice(row);
    }
    let mut selected = vec![false; rows.len()];
    for &i in &ids {
        selected[i] = true;
    }
    let mut fixed = free.to_vec();
    for (i, d) in directions.iter().enumerate() {
        if !selected[i] {
            for (x, v) in fixed.iter_mut().zip(d) {
                *x += lambda[i] * v;
            }
        }
    }
    let rhs: Vec<_> = ids
        .iter()
        .map(|&i| rows[i].offset - dot(&rows[i].jacobian, &fixed))
        .collect();
    let target = solve_factor(&factor, &rhs);
    if target.iter().any(|v| !v.is_finite()) {
        return Err(());
    }
    let mut alpha: f64 = 1.;
    let mut blocking = None;
    for (&i, &t) in ids.iter().zip(&target) {
        if t < 0. {
            let bound = lambda[i] / (lambda[i] - t);
            if bound < alpha {
                alpha = bound;
                blocking = Some(i);
            }
        }
    }
    let updates: Vec<_> = ids
        .iter()
        .zip(&target)
        .map(|(&i, &t)| lambda[i] + alpha * (t - lambda[i]))
        .collect();
    if updates.iter().any(|v| !v.is_finite()) {
        return Err(());
    }
    for (&i, &v) in ids.iter().zip(&updates) {
        lambda[i] = v.max(0.);
    }
    if pivot_boundary && let Some(i) = blocking {
        // This is the exact endpoint of the nonnegative line step, not a
        // small-force threshold. Force it to zero despite cancellation error.
        lambda[i] = 0.;
    }
    Ok(Some((m < active.len(), blocking.is_some())))
}
pub fn solve_with_method(
    h: &[f64],
    g: &[f64],
    rows: &[Row],
    cfg: Settings,
    method: DualMethod,
) -> Result<Report, String> {
    let n = g.len();
    if n == 0
        || n > 192
        || h.len() != n * n
        || h.iter().chain(g).any(|v| !v.is_finite())
        || cfg.max_sweeps == 0
        || [cfg.feasibility_m, cfg.stationarity_n, cfg.complementarity_j]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.)
        || rows.iter().any(|r| {
            r.jacobian.len() != n
                || !r.offset.is_finite()
                || r.jacobian.iter().any(|v| !v.is_finite())
                || dot(&r.jacobian, &r.jacobian) <= 0.
        })
    {
        return Err("Invalid dense contact QP (limit: 192 degrees of freedom)".into());
    }
    let scale = h.iter().copied().map(f64::abs).fold(0., f64::max);
    let mut l = vec![0.; n * n];
    for i in 0..n {
        for j in 0..=i {
            if (h[i * n + j] - h[j * n + i]).abs() > 1e-12 * scale {
                return Err("Dense contact metric is asymmetric".into());
            }
            let mut a = (h[i * n + j] + h[j * n + i]) * 0.5;
            for k in 0..j {
                a -= l[i * n + k] * l[j * n + k];
            }
            l[i * n + j] = if i == j {
                if a <= 0. || !a.is_finite() {
                    return Err("Dense contact metric is not positive definite".into());
                }
                a.sqrt()
            } else {
                a / l[j * n + j]
            };
        }
    }
    let directions: Vec<_> = rows.iter().map(|r| solve_factor(&l, &r.jacobian)).collect();
    let diagonal: Vec<_> = rows
        .iter()
        .zip(&directions)
        .map(|(r, d)| dot(&r.jacobian, d))
        .collect();
    if diagonal.iter().any(|d| !d.is_finite() || *d <= 0.) {
        return Err("Invalid dense contact inverse metric".into());
    }
    let mut lambda = vec![0.; rows.len()];
    let free = solve_factor(&l, &g.iter().map(|v| -v).collect::<Vec<_>>());
    let mut d = free.clone();
    let mut block_steps = 0;
    let mut singular_blocks = 0;
    let mut reduced_blocks = 0;
    let mut boundary_pivots = 0;
    let pivot = method == DualMethod::IndependentPivotBlock;
    let mut schur = Schur::new(
        rows,
        &directions,
        method == DualMethod::IndependentBlock || pivot,
    );
    for sweep in 0..=cfg.max_sweeps {
        let mut force = vec![0.; n];
        for (r, lambda) in rows.iter().zip(&lambda) {
            for (f, a) in force.iter_mut().zip(&r.jacobian) {
                *f += lambda * a;
            }
        }
        let stationarity = (0..n)
            .map(|i| (dot(&h[i * n..(i + 1) * n], &d) + g[i] - force[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        let mut feasibility: f64 = 0.;
        let mut complementarity: f64 = 0.;
        for (r, lambda) in rows.iter().zip(&lambda) {
            let gap = dot(&r.jacobian, &d) - r.offset;
            if !gap.is_finite() || !(gap * lambda).is_finite() {
                return Err("Dense contact residual overflow".into());
            }
            feasibility = feasibility.max(-gap);
            complementarity = complementarity.max((gap * lambda).abs());
        }
        if !stationarity.is_finite() || d.iter().any(|v| !v.is_finite()) {
            return Err("Dense contact iterate overflow".into());
        }
        let converged = feasibility <= cfg.feasibility_m
            && stationarity <= cfg.stationarity_n
            && complementarity <= cfg.complementarity_j;
        if converged || sweep == cfg.max_sweeps {
            return Ok(Report {
                displacement: d,
                multipliers: lambda,
                converged,
                sweeps: sweep,
                feasibility_m: feasibility,
                stationarity_n: stationarity,
                complementarity_j: complementarity,
                active_block_steps: block_steps,
                singular_active_blocks: singular_blocks,
                reduced_active_blocks: reduced_blocks,
                boundary_pivots,
                schur_entries_computed: schur.computed,
                schur_cache_hits: schur.hits,
                schur_cache_bytes: schur
                    .values
                    .as_ref()
                    .map_or(0, |v| v.len() * std::mem::size_of::<f64>()),
            });
        }
        for (i, r) in rows.iter().enumerate() {
            let raw = lambda[i] + (r.offset - dot(&r.jacobian, &d)) / diagonal[i];
            if !raw.is_finite() {
                return Err("Dense contact dual overflow".into());
            }
            let next = raw.max(0.);
            for (d, a) in d.iter_mut().zip(&directions[i]) {
                *d += a * (next - lambda[i]);
            }
            if d.iter().any(|v| !v.is_finite()) {
                return Err("Dense contact primal overflow".into());
            }
            lambda[i] = next;
        }
        if method == DualMethod::ActiveBlock {
            match active_block(rows, &directions, &free, &mut lambda) {
                Ok(true) => block_steps += 1,
                Err(()) => singular_blocks += 1,
                _ => {}
            }
        }
        if matches!(
            method,
            DualMethod::IndependentBlock
                | DualMethod::IndependentBlockUncached
                | DualMethod::IndependentPivotBlock
        ) {
            // Each boundary pivot removes at least one positive multiplier;
            // skipped dependent rows remain fixed, and full KKT checks follow.
            let budget = if pivot {
                rows.len().saturating_add(1)
            } else {
                1
            };
            for _ in 0..budget {
                match independent_block(rows, &directions, &free, &mut lambda, &mut schur, pivot) {
                    Ok(Some((reduced, hit_boundary))) => {
                        block_steps += 1;
                        reduced_blocks += usize::from(reduced);
                        if pivot && hit_boundary {
                            boundary_pivots += 1;
                        } else {
                            break;
                        }
                    }
                    Err(()) => {
                        singular_blocks += 1;
                        break;
                    }
                    Ok(None) => break,
                }
            }
        }
        let mut rhs: Vec<_> = g.iter().map(|v| -v).collect();
        for (r, lambda) in rows.iter().zip(&lambda) {
            for (b, a) in rhs.iter_mut().zip(&r.jacobian) {
                *b += a * lambda;
            }
        }
        d = solve_factor(&l, &rhs);
    }
    unreachable!()
}

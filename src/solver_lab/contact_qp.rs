//! Independent CPU inequality-QP kernel for contact research.
//! Minimizes 0.5*d^T*H*d + g^T*d subject to n_i.d >= b_i.
//! Dual coordinate ascent preserves the FULL primal residual, including
//! non-diagonal H. No compliance, penalty, force cap or position damping.
//! This is not a complete AVBD/XPBD backend or a collision detector.
use glam::{DMat3, DVec3};
use serde::Serialize;

#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub normal: DVec3,
    pub offset: f64,
}
#[derive(Clone, Copy)]
pub struct Settings {
    pub max_sweeps: usize,
    pub feasibility_m: f64,
    pub stationarity_n: f64,
    pub complementarity_j: f64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            max_sweeps: 256,
            feasibility_m: 1e-10,
            stationarity_n: 1e-7,
            complementarity_j: 1e-10,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub displacement: DVec3,
    /// Nonnegative forces in NEWTONS along normalized row normals.
    /// Duplicates can have nonunique individual multipliers.
    pub lambda: Vec<f64>,
    pub reaction: DVec3,
    pub sweeps: usize,
    pub converged: bool,
    pub feasibility_m: f64,
    pub stationarity_n: f64,
    pub complementarity_j: f64,
    pub objective_j: f64,
}
struct Metric {
    h: DMat3,
    l: [f64; 6],
}
impl Metric {
    fn new(h: DMat3) -> Result<Self, String> {
        if !h.is_finite() {
            return Err("Nonfinite contact QP metric".into());
        }
        let scale = h
            .to_cols_array()
            .into_iter()
            .map(f64::abs)
            .fold(0., f64::max);
        let asym = (h - h.transpose())
            .to_cols_array()
            .into_iter()
            .map(f64::abs)
            .fold(0., f64::max);
        if scale <= 0. || asym > 1e-12 * scale {
            return Err("Contact QP metric must be symmetric".into());
        }
        // Antisymmetric roundoff contributes nothing to a quadratic form.
        let h = (h + h.transpose()) * 0.5;
        let a = h.x_axis.x;
        if a <= 0. {
            return Err("Contact QP metric is not positive definite".into());
        }
        let l00 = a.sqrt();
        let l10 = h.x_axis.y / l00;
        let l20 = h.x_axis.z / l00;
        let b = h.y_axis.y - l10 * l10;
        if b <= 0. {
            return Err("Contact QP metric is not positive definite".into());
        }
        let l11 = b.sqrt();
        let l21 = (h.y_axis.z - l20 * l10) / l11;
        let c = h.z_axis.z - l20 * l20 - l21 * l21;
        if c <= 0. || !c.is_finite() {
            return Err("Contact QP metric is not positive definite".into());
        }
        Ok(Self {
            h,
            l: [l00, l10, l20, l11, l21, c.sqrt()],
        })
    }
    fn solve(&self, b: DVec3) -> DVec3 {
        let [a, c, d, e, f, g] = self.l;
        let y0 = b.x / a;
        let y1 = (b.y - c * y0) / e;
        let y2 = (b.z - d * y0 - f * y1) / g;
        let z = y2 / g;
        let y = (y1 - f * z) / e;
        let x = (y0 - c * y - d * z) / a;
        DVec3::new(x, y, z)
    }
}

pub fn solve(
    h: DMat3,
    g: DVec3,
    rows: &[Row],
    warm_start: Option<&[f64]>,
    cfg: Settings,
) -> Result<Report, String> {
    if !g.is_finite()
        || cfg.max_sweeps == 0
        || [cfg.feasibility_m, cfg.stationarity_n, cfg.complementarity_j]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.)
    {
        return Err("Invalid contact QP input/tolerances".into());
    }
    let metric = Metric::new(h)?;
    let rows = rows
        .iter()
        .map(|r| {
            let len = r.normal.length();
            if !r.normal.is_finite() || !len.is_finite() || len <= 0. || !r.offset.is_finite() {
                return Err("Invalid contact QP row".to_string());
            }
            let r = Row {
                normal: r.normal / len,
                offset: r.offset / len,
            };
            if !r.offset.is_finite() {
                return Err("Contact QP row normalization overflow".into());
            }
            Ok(r)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut lambda = warm_start.map_or_else(|| vec![0.; rows.len()], |v| v.to_vec());
    if lambda.len() != rows.len() || lambda.iter().any(|v| !v.is_finite() || *v < 0.) {
        return Err("Invalid contact QP warm start".into());
    }
    let directions = rows
        .iter()
        .map(|r| metric.solve(r.normal))
        .collect::<Vec<_>>();
    let diagonal = rows
        .iter()
        .zip(&directions)
        .map(|(r, d)| r.normal.dot(*d))
        .collect::<Vec<_>>();
    if directions.iter().any(|v| !v.is_finite())
        || diagonal.iter().any(|v| !v.is_finite() || *v <= 0.)
    {
        return Err("Contact QP inverse metric is numerically invalid".into());
    }
    let reaction = |lambda: &[f64]| {
        rows.iter()
            .zip(lambda)
            .map(|(r, l)| r.normal * *l)
            .sum::<DVec3>()
    };
    let mut displacement = metric.solve(-g + reaction(&lambda));
    for sweep in 0..=cfg.max_sweeps {
        let force = reaction(&lambda);
        let stationarity = (metric.h * displacement + g - force).length();
        let mut feasibility: f64 = 0.;
        let mut complementarity: f64 = 0.;
        for (r, l) in rows.iter().zip(&lambda) {
            let gap = r.normal.dot(displacement) - r.offset;
            if !gap.is_finite() || !(l * gap).is_finite() {
                return Err("Contact QP residual overflow".into());
            }
            feasibility = feasibility.max(-gap);
            complementarity = complementarity.max((l * gap).abs());
        }
        if !displacement.is_finite()
            || !force.is_finite()
            || !stationarity.is_finite()
            || !feasibility.is_finite()
            || !complementarity.is_finite()
        {
            return Err("Contact QP iterate is nonfinite".into());
        }
        let converged = feasibility <= cfg.feasibility_m
            && stationarity <= cfg.stationarity_n
            && complementarity <= cfg.complementarity_j;
        if converged || sweep == cfg.max_sweeps {
            let objective = 0.5 * displacement.dot(metric.h * displacement) + g.dot(displacement);
            if !objective.is_finite() {
                return Err("Contact QP objective overflow".into());
            }
            return Ok(Report {
                displacement,
                lambda,
                reaction: force,
                sweeps: sweep,
                converged,
                feasibility_m: feasibility,
                stationarity_n: stationarity,
                complementarity_j: complementarity,
                objective_j: objective,
            });
        }
        for i in 0..rows.len() {
            let residual = rows[i].offset - rows[i].normal.dot(displacement);
            let raw = lambda[i] + residual / diagonal[i];
            if !raw.is_finite() {
                return Err("Contact QP dual update overflow".into());
            }
            let next = raw.max(0.);
            displacement += directions[i] * (next - lambda[i]);
            if !displacement.is_finite() {
                return Err("Contact QP primal update overflow".into());
            }
            lambda[i] = next;
        }
        // Reconstruct from the full stationarity equation; this is the same
        // dual iterate, not a post-solve geometric projection.
        displacement = metric.solve(-g + reaction(&lambda));
    }
    unreachable!()
}

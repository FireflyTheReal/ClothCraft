//! Coupled force/complementarity evaluation for discrete-contact research.
//! The force basis may differ from the derivative of endpoint feasibility.
use super::{deforming_contact, discrete_contact};
use crate::ogc::{feature_hierarchy::Pair, topology::Topology};
use glam::DVec3;

pub struct Reaction {
    pub force: Vec<f64>,
    pub gaps: Vec<f64>,
    pub complementarity_j: f64,
}
pub struct Problem<'a> {
    pub topology: &'a Topology,
    pub pairs: &'a [Pair],
    pub old: &'a [DVec3],
    pub cloth_vertices: usize,
    pub clearance: f64,
}
/// Multipliers are ordered as one analytic floor row per cloth vertex, then
/// the caller's fixed VF/EE universe. No constraints or negative reactions
/// are silently removed.
impl Problem<'_> {
    pub fn evaluate(
        &self,
        new: &[DVec3],
        multipliers: &[f64],
        discrete: bool,
    ) -> Result<Reaction, String> {
        let top = self.topology;
        let pairs = self.pairs;
        let old = self.old;
        let n = self.cloth_vertices;
        let clearance = self.clearance;
        if n > new.len()
            || new.len() != old.len()
            || new.len() != top.neighbors.len()
            || multipliers.len() != n + pairs.len()
            || !clearance.is_finite()
            || clearance <= 0.
            || multipliers.iter().any(|v| !v.is_finite() || *v < 0.)
            || old.iter().chain(new).any(|p| !p.is_finite())
        {
            return Err("Invalid contact residual inputs".into());
        }
        let mut force = vec![0.; 3 * n];
        let mut gaps = Vec::with_capacity(n + pairs.len());
        for i in 0..n {
            force[3 * i + 1] = multipliers[i];
            gaps.push(new[i].y - clearance);
        }
        for (i, &pair) in pairs.iter().enumerate() {
            let valid = match pair {
                Pair::VertexFace([v, f]) => v < new.len() && f < top.triangles.len(),
                Pair::EdgeEdge([a, b]) => a < top.edges.len() && b < top.edges.len(),
                _ => false,
            };
            if !valid {
                return Err("Invalid residual contact pair".into());
            }
            let gap = pair.distance(top, new) - clearance;
            if !gap.is_finite() {
                return Err("Nonfinite contact gap".into());
            }
            gaps.push(gap);
            let lambda = multipliers[n + i];
            if lambda == 0. {
                continue;
            }
            if discrete {
                let row = discrete_contact::row(pair, top, old, new)?;
                for (&id, j) in row.vertices.iter().zip(&row.gradient) {
                    if id < n {
                        for k in 0..3 {
                            force[3 * id + k] += lambda * j[k];
                        }
                    }
                }
            } else {
                let row = deforming_contact::linearize(pair, top, new, clearance)?;
                for (f, j) in force.iter_mut().zip(&row.jacobian) {
                    *f += lambda * j;
                }
            }
        }
        let comp = gaps
            .iter()
            .zip(multipliers)
            .map(|(g, l)| (g * l).abs())
            .fold(0., f64::max);
        if !comp.is_finite() || force.iter().any(|v| !v.is_finite()) {
            return Err("Contact reaction overflow".into());
        }
        Ok(Reaction {
            force,
            gaps,
            complementarity_j: comp,
        })
    }
}
/// Dimensionless natural-complementarity merit. The two scales are numerical
/// residual scales (N and m), not physical contact compliance or damping.
pub fn merit(
    balance: &[f64],
    gaps: &[f64],
    multipliers: &[f64],
    force_scale: f64,
    gap_scale: f64,
) -> Result<f64, String> {
    if gaps.len() != multipliers.len()
        || !force_scale.is_finite()
        || force_scale <= 0.
        || !gap_scale.is_finite()
        || gap_scale <= 0.
        || balance
            .iter()
            .chain(gaps)
            .chain(multipliers)
            .any(|v| !v.is_finite())
    {
        return Err("Invalid complementarity merit inputs".into());
    }
    let result = balance
        .iter()
        .map(|r| (r / force_scale).powi(2))
        .sum::<f64>()
        + gaps
            .iter()
            .zip(multipliers)
            .map(|(g, l)| (l / force_scale).min(g / gap_scale).powi(2))
            .sum::<f64>();
    if !result.is_finite() {
        return Err("Complementarity merit overflow".into());
    }
    Ok(result)
}

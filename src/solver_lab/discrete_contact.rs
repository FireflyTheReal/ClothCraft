//! Research discrete distance gradients in squared-distance invariants.
//! This is a force-row experiment, NOT a collision certificate or integrator.
//! It must not replace the geometric Jacobian in the existing contact QP.
use crate::ogc::{feature_hierarchy::Pair, topology::Topology};
use glam::DVec3;

#[derive(Clone, Copy)]
enum Kind {
    VertexFace,
    EdgeEdge,
}
const EDGES: [(usize, usize); 6] = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];

pub struct DiscreteRow {
    pub vertices: [usize; 4],
    pub gradient: [DVec3; 4],
    pub endpoint_gradient: [DVec3; 4],
    /// Negative control: secant norm with endpoint barycentrics frozen.
    /// This can create artificial tangential resistance.
    pub frozen_witness_gradient: [DVec3; 4],
    pub distance_before: f64,
    pub distance_after: f64,
    pub frozen_witness_old_distance: f64,
    pub chain_error_m: f64,
    pub near_invariant_coincidence: bool,
}

/// A six-dimensional embedding of averaged endpoint Gram matrices, represented
/// without reconstruction or subtraction of squared distances. Its metric is
/// (old dot old + new dot new)/2.
#[derive(Clone, Copy)]
struct Lift(DVec3, DVec3);
impl Lift {
    fn sub(self, b: Self) -> Self {
        Self(self.0 - b.0, self.1 - b.1)
    }
    fn dot(self, b: Self) -> f64 {
        0.5 * (self.0.dot(b.0) + self.1.dot(b.1))
    }
}
fn separation(weights: &[f64; 4], x: &[DVec3; 4]) -> DVec3 {
    // All weights sum to zero algebraically. Relative coordinates avoid loss
    // of translation invariance through large absolute position cancellation.
    (1..4).map(|i| weights[i] * (x[i] - x[0])).sum()
}
fn closest(kind: Kind, old: &[DVec3; 4], new: &[DVec3; 4]) -> Result<([f64; 4], f64), String> {
    let p: [Lift; 4] = std::array::from_fn(|i| Lift(old[i], new[i]));
    let mut best = ([0.; 4], f64::INFINITY);
    let mut consider = |weights: [f64; 4]| {
        let r = Lift(separation(&weights, old), separation(&weights, new));
        let d = r.dot(r);
        if d < best.1 {
            best = (weights, d);
        }
    };
    match kind {
        Kind::VertexFace => {
            for i in 1..4 {
                let mut w = [0.; 4];
                w[0] = 1.;
                w[i] = -1.;
                consider(w);
            }
            for (i, j) in [(1, 2), (2, 3), (3, 1)] {
                let edge = p[j].sub(p[i]);
                let a = edge.dot(edge);
                if a > 1e-24 {
                    let t = (p[0].sub(p[i]).dot(edge) / a).clamp(0., 1.);
                    let mut w = [0.; 4];
                    w[0] = 1.;
                    w[i] = -(1. - t);
                    w[j] = -t;
                    consider(w);
                }
            }
            let u = p[2].sub(p[1]);
            let v = p[3].sub(p[1]);
            let r = p[0].sub(p[1]);
            let a = u.dot(u);
            let b = u.dot(v);
            let c = v.dot(v);
            let det = a * c - b * b;
            if det > 64. * f64::EPSILON * a * c {
                let s = (r.dot(u) * c - r.dot(v) * b) / det;
                let t = (r.dot(v) * a - r.dot(u) * b) / det;
                if s >= 0. && t >= 0. && s + t <= 1. {
                    consider([1., -(1. - s - t), -s, -t]);
                }
            }
        }
        Kind::EdgeEdge => {
            let u = p[1].sub(p[0]);
            let v = p[3].sub(p[2]);
            let r = p[0].sub(p[2]);
            let a = u.dot(u);
            let b = u.dot(v);
            let c = v.dot(v);
            if a <= 1e-24 || c <= 1e-24 {
                return Err("Degenerate discrete-contact edge".into());
            }
            for s in [0., 1.] {
                let t = ((r.dot(v) + s * b) / c).clamp(0., 1.);
                consider([1. - s, s, -(1. - t), -t]);
            }
            for t in [0., 1.] {
                let s = ((t * b - r.dot(u)) / a).clamp(0., 1.);
                consider([1. - s, s, -(1. - t), -t]);
            }
            let det = a * c - b * b;
            if det > 64. * f64::EPSILON * a * c {
                let s = (b * r.dot(v) - c * r.dot(u)) / det;
                let t = (a * r.dot(v) - b * r.dot(u)) / det;
                if (0. ..=1.).contains(&s) && (0. ..=1.).contains(&t) {
                    consider([1. - s, s, -(1. - t), -t]);
                }
            }
        }
    }
    if !best.1.is_finite() || best.1 <= 1e-28 {
        return Err("Discrete contact requires finite separated primitives".into());
    }
    Ok((best.0, best.1.sqrt()))
}

/// Invariant Gonzalez-type discrete gradient of the actual finite-primitive
/// distance. Barycentrics are minimized in the averaged invariant metric.
/// Internal resultant and torque about midpoint positions vanish algebraically.
pub fn row(
    pair: Pair,
    top: &Topology,
    old: &[DVec3],
    new: &[DVec3],
) -> Result<DiscreteRow, String> {
    if old.len() != top.neighbors.len()
        || new.len() != old.len()
        || old.iter().chain(new).any(|p| !p.is_finite())
    {
        return Err("Invalid discrete-contact positions".into());
    }
    let (ids, kind) = match pair {
        Pair::VertexFace([v, f]) if v < old.len() => {
            let face = top
                .triangles
                .get(f)
                .ok_or("Invalid discrete-contact face")?;
            ([v, face[0], face[1], face[2]], Kind::VertexFace)
        }
        Pair::EdgeEdge([a, b]) => {
            let a = top.edges.get(a).ok_or("Invalid discrete-contact edge")?;
            let b = top.edges.get(b).ok_or("Invalid discrete-contact edge")?;
            ([a[0], a[1], b[0], b[1]], Kind::EdgeEdge)
        }
        _ => return Err("Discrete contact supports VF/EE pairs only".into()),
    };
    if (0..4).any(|i| (i + 1..4).any(|j| ids[i] == ids[j])) {
        return Err("Discrete contact requires disjoint primitives".into());
    }
    let before = ids.map(|i| old[i]);
    let after = ids.map(|i| new[i]);
    let (_, d0) = closest(kind, &before, &before)?;
    let (end_weights, d1) = closest(kind, &after, &after)?;
    let (weights, dm) = closest(kind, &before, &after)?;
    let mid: [DVec3; 4] = std::array::from_fn(|i| 0.5 * (before[i] + after[i]));
    let delta: [f64; 6] = EDGES.map(|(i, j)| {
        let a = before[i] - before[j];
        let b = after[i] - after[j];
        (0.5 * (a + b)).dot(b - a)
    });
    let scale = EDGES
        .iter()
        .map(|&(i, j)| {
            0.5 * (before[i].distance_squared(before[j])).max(after[i].distance_squared(after[j]))
        })
        .fold(0., f64::max);
    let norm2: f64 = delta.iter().map(|p| p * p).sum();
    if !scale.is_finite() || !norm2.is_finite() {
        return Err("Discrete-contact invariant overflow".into());
    }
    let near = norm2.sqrt() <= 64. * f64::EPSILON * scale;
    let mut coefficients = EDGES.map(|(i, j)| -weights[i] * weights[j] / dm);
    if !near {
        let approximate: f64 = coefficients.iter().zip(&delta).map(|(g, d)| g * d).sum();
        let correction = ((d1 - d0) - approximate) / norm2;
        for (g, d) in coefficients.iter_mut().zip(&delta) {
            *g += correction * d;
        }
    }
    let mut gradient = [DVec3::ZERO; 4];
    for ((i, j), c) in EDGES.into_iter().zip(coefficients) {
        let force = c * (mid[i] - mid[j]);
        gradient[i] += force;
        gradient[j] -= force;
    }
    let end_separation = separation(&end_weights, &after);
    let old_separation = separation(&end_weights, &before);
    let old_witness_distance = old_separation.length();
    let frozen_normal = (end_separation + old_separation) / (d1 + old_witness_distance);
    let endpoint_gradient = end_weights.map(|w| w * end_separation / d1);
    let frozen_witness_gradient = end_weights.map(|w| w * frozen_normal);
    let work: f64 = (0..4).map(|i| gradient[i].dot(after[i] - before[i])).sum();
    if gradient
        .iter()
        .chain(&endpoint_gradient)
        .chain(&frozen_witness_gradient)
        .any(|g| !g.is_finite())
        || !work.is_finite()
    {
        return Err("Nonfinite discrete contact row".into());
    }
    Ok(DiscreteRow {
        vertices: ids,
        gradient,
        endpoint_gradient,
        frozen_witness_gradient,
        distance_before: d0,
        distance_after: d1,
        frozen_witness_old_distance: old_witness_distance,
        chain_error_m: work - (d1 - d0),
        near_invariant_coincidence: near,
    })
}

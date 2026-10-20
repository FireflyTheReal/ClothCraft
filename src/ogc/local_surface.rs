//! Research-only normalized local plane field (MLS family).
//! This is a field for ONE oriented surface patch, not a self-contact solver.
//! Mixing nearby sheets can invent a surface between them. Original triangle
//! collision bounds remain necessary; missing support is an error, not zero force.
use super::potential::activation;
use crate::math::{Scalar, cross, dot, sub};

#[derive(Clone, Copy)]
pub struct Sample<S> {
    center: [S; 3],
    normal: [S; 3],
    weight: f64,
}
impl<S: Scalar> Sample<S> {
    pub fn oriented(center: [S; 3], normal: [S; 3], weight: f64) -> Result<Self, String> {
        let n2 = dot(normal, normal);
        if !weight.is_finite()
            || weight <= 0.
            || center
                .iter()
                .chain(normal.iter())
                .any(|a| !a.value().is_finite())
            || !n2.value().is_finite()
            || n2.value() <= 0.
        {
            return Err("Invalid local surface sample".into());
        }
        let norm = n2.sqrt();
        Ok(Self {
            center,
            normal: normal.map(|v| v / norm),
            weight,
        })
    }
    /// Fixed reference quadrature weight; centers and unit normals are
    /// differentiated through all three current triangle vertices.
    pub fn triangle(x: [[S; 3]; 3], weight: f64) -> Result<Self, String> {
        let [a, b, c] = x;
        let center = std::array::from_fn(|i| (a[i] + b[i] + c[i]) / S::c(3.));
        Self::oriented(center, cross(sub(b, a), sub(c, a)), weight)
    }
}

pub struct Field<S> {
    /// Signed field value in metres, generally NOT a Euclidean distance.
    pub value: S,
    pub weight_sum: f64,
    pub active_samples: usize,
    /// Diagnostic only. Near one does not prove that samples belong to one sheet.
    pub normal_coherence: f64,
}

/// Compact polynomial in squared radius: max(1-r²/R²,0)^3.
/// Its first and second derivatives vanish at the support boundary. The
/// normalized field is smooth there only while other support remains.
pub fn evaluate<S: Scalar>(
    query: [S; 3],
    samples: &[Sample<S>],
    support: f64,
) -> Result<Field<S>, String> {
    if !support.is_finite()
        || support <= 0.
        || !(support * support).is_finite()
        || support * support == 0.
        || !(1. / (support * support)).is_finite()
        || query.iter().any(|a| !a.value().is_finite())
    {
        return Err("Invalid local surface query/support".into());
    }
    let mut terms = Vec::new();
    let mut max_weight: f64 = 0.;
    for s in samples {
        let r = sub(query, s.center);
        let q = dot(r, r) * S::c(1. / (support * support));
        if !q.value().is_finite() {
            return Err("Nonfinite local surface radius".into());
        }
        if q.value() >= 1. {
            continue;
        }
        let z = S::c(1.) - q;
        let w = S::c(s.weight) * z * z * z;
        if !w.value().is_finite() {
            return Err("Nonfinite local surface weight".into());
        }
        max_weight = max_weight.max(w.value());
        terms.push((w, dot(r, s.normal), s.normal));
    }
    if max_weight <= f64::MIN_POSITIVE {
        return Err("Local surface support is empty or numerically exhausted".into());
    }
    // A common numeric scale cancels exactly from the quotient, including
    // all derivatives; no weight or normal is lagged. The anchored form
    // avoids cancellation in flat-plane query gradients near support edges.
    let anchor = terms[0].1;
    let mut numerator = S::c(0.);
    let mut denominator = S::c(0.);
    let mut normal_sum = [0.; 3];
    for &(w, plane, normal) in &terms {
        let w = w * S::c(1. / max_weight);
        numerator = numerator + w * (plane - anchor);
        denominator = denominator + w;
        for (i, n) in normal_sum.iter_mut().enumerate() {
            *n += w.value() * normal[i].value();
        }
    }
    let value = anchor + numerator / denominator;
    if !value.value().is_finite() {
        return Err("Nonfinite local surface field".into());
    }
    let normal_coherence = normal_sum
        .iter()
        .map(|v| (v / denominator.value()).powi(2))
        .sum::<f64>()
        .sqrt();
    let weight_sum = max_weight * denominator.value();
    if !weight_sum.is_finite() {
        return Err("Local surface weight sum overflowed".into());
    }
    Ok(Field {
        value,
        weight_sum,
        active_samples: terms.len(),
        normal_coherence,
    })
}

/// A chosen oriented side of ONE field, in joules. The side is fixed by the
/// caller; never reselect it from the sign of a trial position during a solve.
/// This barrier protects only the reconstructed zero-set, not the triangles.
pub fn energy<S: Scalar>(
    query: [S; 3],
    samples: &[Sample<S>],
    support: f64,
    side: f64,
    radius: f64,
    stiffness: f64,
) -> Result<S, String> {
    if (side != 1. && side != -1.)
        || !radius.is_finite()
        || radius <= 0.
        || !stiffness.is_finite()
        || stiffness <= 0.
    {
        return Err("Invalid local surface barrier parameters".into());
    }
    let field = evaluate(query, samples, support)?;
    let gap = S::c(side) * field.value;
    if gap.value() <= 0. {
        return Err("Trial crossed its oriented local surface".into());
    }
    Ok(activation(gap, radius, stiffness))
}

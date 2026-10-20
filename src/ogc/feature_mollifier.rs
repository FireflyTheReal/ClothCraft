//! Isolated GCP Appendix A feature-transition factors, not a GCP solver.
//! https://arxiv.org/html/2402.00719v3#A1
//! They suppress boundary/parallel higher-dimensional terms. A complete model
//! must retain the lower-dimensional contact terms and directional factors.
//! These weights alone provide NO collision protection.
use super::primitive_distance::{point_segment, point_triangle, segment_segment};
use crate::math::{Scalar, cross, dot, sub};

fn edge_valid<S: Scalar>(a: [S; 3], b: [S; 3]) -> bool {
    let e = sub(b, a);
    let length2 = dot(e, e).value();
    length2.is_finite() && length2 > 1e-24
}

fn point_point<S: Scalar>(a: [S; 3], b: [S; 3]) -> S {
    let d = sub(a, b);
    dot(d, d).sqrt()
}
fn factor<S: Scalar>(ratio: S, width: f64) -> S {
    let z = (ratio - S::c(1.)) / S::c(width);
    if z.value() <= 0. {
        S::c(0.)
    } else if z.value() >= 1. {
        S::c(1.)
    } else {
        z * (S::c(2.) - z)
    }
}
fn product<S: Scalar>(distance: S, numerators: &[S], width: f64) -> Result<S, String> {
    if !width.is_finite()
        || width <= 0.
        || !distance.value().is_finite()
        || distance.value() <= 0.
        || numerators.iter().any(|x| !x.value().is_finite())
    {
        return Err(
            "Feature mollification requires separated finite geometry and positive width".into(),
        );
    }
    Ok(numerators
        .iter()
        .fold(S::c(1.), |p, d| p * factor(*d / distance, width)))
}

/// p against segment ab. Width is dimensionless (paper examples: 0.01).
/// At exact boundaries we select the zero-side Hessian; only C1 is claimed.
pub fn vertex_edge<S: Scalar>(x: [[S; 3]; 3], width: f64) -> Result<S, String> {
    let [p, a, b] = x;
    if !edge_valid(a, b) {
        return Err("Degenerate feature edge".into());
    }
    product(
        point_segment(p, a, b),
        &[point_point(p, a), point_point(p, b)],
        width,
    )
}

/// p against triangle abc; no surface normals/orientation factors included.
pub fn vertex_face<S: Scalar>(x: [[S; 3]; 4], width: f64) -> Result<S, String> {
    let [p, a, b, c] = x;
    let normal = cross(sub(b, a), sub(c, a));
    let area2 = dot(normal, normal).value();
    if !area2.is_finite()
        || area2 <= 1e-30
        || !edge_valid(a, b)
        || !edge_valid(b, c)
        || !edge_valid(c, a)
    {
        return Err("Degenerate feature face".into());
    }
    product(
        point_triangle(x),
        &[
            point_segment(p, a, b),
            point_segment(p, b, c),
            point_segment(p, c, a),
        ],
        width,
    )
}

/// Segment ab against segment cd, including the parallel limit.
pub fn edge_edge<S: Scalar>(x: [[S; 3]; 4], width: f64) -> Result<S, String> {
    let [a, b, c, d] = x;
    if !edge_valid(a, b) || !edge_valid(c, d) {
        return Err("Degenerate feature edge".into());
    }
    product(
        segment_segment(x),
        &[
            point_segment(a, c, d),
            point_segment(b, c, d),
            point_segment(c, a, b),
            point_segment(d, a, b),
        ],
        width,
    )
}

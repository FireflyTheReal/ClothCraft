//! Finite primitive distances for an opt-in contact-law experiment.
//! Branch selection uses values; derivatives include the selected closest
//! coordinates. This is not frozen-barycentric or infinite-feature contact.
use super::geometry::{
    closest_segments, edge_edge_distance, point_edge_distance, point_face_distance,
};
use crate::math::{Scalar, cross, dot, sub};
use glam::DVec3;

fn point_point<S: Scalar>(p: [S; 3], q: [S; 3]) -> S {
    let v = sub(p, q);
    dot(v, v).sqrt()
}

pub(super) fn point_segment<S: Scalar>(p: [S; 3], a: [S; 3], b: [S; 3]) -> S {
    let ab = sub(b, a);
    let len2 = dot(ab, ab).value();
    if len2 <= 1e-24 {
        return point_point(p, a);
    }
    let t = dot(sub(p, a), ab).value() / len2;
    if t <= 0. {
        point_point(p, a)
    } else if t >= 1. {
        point_point(p, b)
    } else {
        point_edge_distance(p, a, b)
    }
}

pub(super) fn point_triangle<S: Scalar>(p: [[S; 3]; 4]) -> S {
    let [v, a, b, c] = p;
    let ab = sub(b, a);
    let ac = sub(c, a);
    let n = cross(ab, ac);
    let det = dot(n, n).value();
    if det > 1e-30 {
        // Scalar region decision, exact differentiated distance in that region.
        let ap = sub(v, a);
        let aa = dot(ab, ab).value();
        let bb = dot(ab, ac).value();
        let cc = dot(ac, ac).value();
        let u = (cc * dot(ap, ab).value() - bb * dot(ap, ac).value()) / det;
        let w = (aa * dot(ap, ac).value() - bb * dot(ap, ab).value()) / det;
        if u >= 0. && w >= 0. && u + w <= 1. {
            return point_face_distance(v, a, b, c);
        }
    }
    [
        point_segment(v, a, b),
        point_segment(v, b, c),
        point_segment(v, c, a),
    ]
    .into_iter()
    .min_by(|a, b| a.value().total_cmp(&b.value()))
    .unwrap()
}

pub(super) fn segment_segment<S: Scalar>(p: [[S; 3]; 4]) -> S {
    let [a, b, c, d] = p;
    let values = p.map(|v| DVec3::from_array(v.map(|s| s.value())));
    let (s, t) = closest_segments(values[0], values[1], values[2], values[3]);
    if s > 0. && s < 1. && t > 0. && t < 1. {
        edge_edge_distance(a, b, c, d)
    } else {
        // Also covers parallel/degenerate segments without an infinite-line
        // singularity. Nonunique parallel minimizers need not have a unique
        // Hessian; the contact's existing mollifier handles that limit.
        [
            point_segment(a, c, d),
            point_segment(b, c, d),
            point_segment(c, a, b),
            point_segment(d, a, b),
        ]
        .into_iter()
        .min_by(|a, b| a.value().total_cmp(&b.value()))
        .unwrap()
    }
}

/// Research-only generalized derivative at a single endpoint/interior tie.
/// The finite distance is C1 but not C2 at this boundary. Averaging its two
/// limiting branches selects the symmetric generalized Hessian. Values and
/// first derivatives coincide at the exact tie; no finite neighborhood is
/// blended and no closest coordinate or normal is frozen.
///
/// This lets cancellations in a larger C2 energy survive term-wise AD.
/// Double-endpoint ties and parallel degeneracies retain the old branch;
/// this is not a globally smooth finite-segment distance.
pub(super) fn segment_segment_symmetric_boundary<S: Scalar>(p: [[S; 3]; 4]) -> S {
    let [a, b, c, d] = p;
    let u = sub(b, a);
    let v = sub(d, c);
    let n = cross(u, v);
    let den = dot(n, n).value();
    if den > 1e-30 {
        // Test endpoint stationarity directly. Comparing a divided line
        // coordinate with 1 can lose an exact tie through denominator
        // roundoff and break endpoint-reversal symmetry.
        for (endpoint, other, line_a, line_b) in
            [(c, d, a, b), (d, c, a, b), (a, b, c, d), (b, a, c, d)]
        {
            let [e, o, l, r] =
                [endpoint, other, line_a, line_b].map(|p| DVec3::from_array(p.map(|s| s.value())));
            let direction = r - l;
            let len2 = direction.length_squared();
            if len2 <= 1e-24 {
                continue;
            }
            let t = (e - l).dot(direction) / len2;
            let residual = e - (l + t * direction);
            if t > 0. && t < 1. && residual.dot(o - e) == 0. {
                return (edge_edge_distance(a, b, c, d)
                    + point_edge_distance(endpoint, line_a, line_b))
                    * S::c(0.5);
            }
        }
    }
    segment_segment(p)
}

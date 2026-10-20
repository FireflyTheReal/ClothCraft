//! Isolated CPU contact-quadrature experiments, NOT an installed contact law.
//!
//! Two incident primitives can count the same repulsion twice. Subtracting the
//! barrier of their common feature is the local construction in Convergent IPC,
//! section 4.2/4.3 (https://arxiv.org/html/2307.15908). Here it uses our existing
//! dimensioned activation, not that paper's complete discretization/solver.
//!
//! For these TWO-primitive patches, each primitive contains the common feature,
//! so A >= C and B >= C for a decreasing barrier. A+B-C >= max(A,B) >= 0.
//! This proof does not license arbitrary signed sums over an entire mesh.
//! No remeshing, search, collision guard, rest data or production mode changes.
//! In particular, the wire kernel has no parallel-edge mollifier: its finite
//! distance is continuous but need not have a unique Hessian at parallelism.
//! Inputs must be strictly separated; coincident common features can evaluate
//! as infinity minus infinity. There is no finite-overlap repair in this kernel.
use super::{geometry, potential::activation, primitive_distance};
use crate::math::{Scalar, cross, dot, sub};
use glam::DVec3;

// Detect exact algebraic cancellation before AD. At a shared-feature tie,
// differentiating A+B-C independently can choose incompatible one-sided
// Hessians, even when the combined energy is smooth (e.g. a flat patch).
// This is a closest-feature branch, not a distance epsilon or damping rule.
fn face_uses_shared_edge(x: [[f64; 3]; 4]) -> bool {
    let [p, a, b, c] = x;
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let n = cross(ab, ac);
    let det = dot(n, n);
    if det > 1e-30 {
        let aa = dot(ab, ab);
        let bb = dot(ab, ac);
        let cc = dot(ac, ac);
        let u = (cc * dot(ap, ab) - bb * dot(ap, ac)) / det;
        let w = (aa * dot(ap, ac) - bb * dot(ap, ab)) / det;
        if u >= 0. && w >= 0. && u + w <= 1. {
            return w == 0.;
        }
    }
    let common = primitive_distance::point_segment(p, a, b);
    common <= primitive_distance::point_segment(p, b, c)
        && common <= primitive_distance::point_segment(p, c, a)
}

fn wire_coordinates<S: Scalar>(x: [[S; 3]; 4]) -> (f64, f64) {
    let v = x.map(|p| DVec3::from_array(p.map(|s| s.value())));
    geometry::closest_segments(v[0], v[1], v[2], v[3])
}
fn wire_distance<S: Scalar>(x: [[S; 3]; 4]) -> S {
    let u = sub(x[1], x[0]);
    let v = sub(x[3], x[2]);
    let w = sub(x[0], x[2]);
    let n = cross(u, v);
    let den = dot(n, n).value();
    // At a tie, use the interior one-sided Hessian after common-feature
    // cancellation. Parallel edges still use the finite-distance branch.
    if den > 1e-30 {
        let aa = dot(u, u).value();
        let bb = dot(u, v).value();
        let ee = dot(v, v).value();
        let cc = dot(u, w).value();
        let ff = dot(v, w).value();
        let s = (bb * ff - cc * ee) / den;
        let t = (aa * ff - bb * cc) / den;
        if (0. ..=1.).contains(&s) && (0. ..=1.).contains(&t) {
            return geometry::edge_edge_distance(x[0], x[1], x[2], x[3]);
        }
    }
    primitive_distance::segment_segment(x)
}

/// Point p against triangles abc and bad, sharing edge ab.
/// Inputs are [p, a, b, c, d]; all five positions participate in derivatives.
pub fn face_pair<S: Scalar>(x: [[S; 3]; 5], radius: f64, stiffness: f64) -> S {
    let [p, a, b, c, d] = x;
    if face_uses_shared_edge([p, a, b, c].map(|p| p.map(|s| s.value()))) {
        return activation(
            primitive_distance::point_triangle([p, b, a, d]),
            radius,
            stiffness,
        );
    }
    if face_uses_shared_edge([p, b, a, d].map(|p| p.map(|s| s.value()))) {
        return activation(
            primitive_distance::point_triangle([p, a, b, c]),
            radius,
            stiffness,
        );
    }
    activation(
        primitive_distance::point_triangle([p, a, b, c]),
        radius,
        stiffness,
    ) + activation(
        primitive_distance::point_triangle([p, b, a, d]),
        radius,
        stiffness,
    ) - activation(
        primitive_distance::point_segment(p, a, b),
        radius,
        stiffness,
    )
}

/// Segment ab against the two incident segments cd and de.
/// Inputs are [a, b, c, d, e]. The common feature is the point d.
/// Not a complete surface-contact energy and not parallel-mollified.
pub fn wire_pair<S: Scalar>(x: [[S; 3]; 5], radius: f64, stiffness: f64) -> S {
    let [a, b, c, d, e] = x;
    if wire_coordinates([a, b, c, d]).1 == 1. {
        return activation(wire_distance([a, b, d, e]), radius, stiffness);
    }
    if wire_coordinates([a, b, d, e]).1 == 0. {
        return activation(wire_distance([a, b, c, d]), radius, stiffness);
    }
    activation(
        primitive_distance::segment_segment([a, b, c, d]),
        radius,
        stiffness,
    ) + activation(
        primitive_distance::segment_segment([a, b, d, e]),
        radius,
        stiffness,
    ) - activation(
        primitive_distance::point_segment(d, a, b),
        radius,
        stiffness,
    )
}

//! Closest features and the constructive feasible regions, equations 8–10/14–15.
use super::topology::Topology;
use crate::math::{Scalar, cross, dot, sub};
use glam::DVec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Feature {
    Vertex(usize),
    Edge(usize),
    Face(usize),
}

pub fn vertex_feasible(p: DVec3, v: usize, x: &[DVec3], top: &Topology) -> bool {
    top.neighbors[v]
        .iter()
        .all(|j| (p - x[v]).dot(x[v] - x[*j]) >= -1e-14)
}
pub fn edge_feasible(p: DVec3, e: usize, x: &[DVec3], top: &Topology) -> bool {
    let [a, b] = top.edges[e].map(|i| x[i]);
    let u = b - a;
    let t = (p - a).dot(u) / u.length_squared();
    t > 0.0
        && t < 1.0
        && top.opposites[e].iter().all(|i| {
            let c = x[*i];
            let q = a + u * ((c - a).dot(u) / u.length_squared());
            (p - q).dot(q - c) >= -1e-14
        })
}

pub fn closest_triangle(p: DVec3, t: usize, x: &[DVec3], top: &Topology) -> (DVec3, Feature) {
    let ids = top.triangles[t];
    let [a, b, c] = ids.map(|i| x[i]);
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return (a, Feature::Vertex(ids[0]));
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return (b, Feature::Vertex(ids[1]));
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return (
            a + ab * (d1 / (d1 - d3)),
            Feature::Edge(top.triangle_edges[t][0]),
        );
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return (c, Feature::Vertex(ids[2]));
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return (
            a + ac * (d2 / (d2 - d6)),
            Feature::Edge(top.triangle_edges[t][2]),
        );
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        return (
            b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6))),
            Feature::Edge(top.triangle_edges[t][1]),
        );
    }
    let denom = 1.0 / (va + vb + vc);
    (a + ab * (vb * denom) + ac * (vc * denom), Feature::Face(t))
}
pub fn closest_segments(a: DVec3, b: DVec3, c: DVec3, d: DVec3) -> (f64, f64) {
    let u = b - a;
    let v = d - c;
    let w = a - c;
    let aa = u.length_squared();
    let ee = v.length_squared();
    if aa < 1e-24 || ee < 1e-24 {
        return (0.0, 0.0);
    }
    let bb = u.dot(v);
    let cc = u.dot(w);
    let ff = v.dot(w);
    let den = aa * ee - bb * bb;
    let mut s = if den > 1e-14 * aa * ee {
        ((bb * ff - cc * ee) / den).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let t = (bb * s + ff) / ee;
    let t = if t < 0.0 {
        s = (-cc / aa).clamp(0.0, 1.0);
        0.0
    } else if t > 1.0 {
        s = ((bb - cc) / aa).clamp(0.0, 1.0);
        1.0
    } else {
        t
    };
    (s, t)
}

/// Signed-distance magnitude to a fixed feature. Closest-point coordinates are
/// differentiated, not frozen barycentrics; required for exact force/Hessian.
pub fn point_edge_distance<S: Scalar>(p: [S; 3], a: [S; 3], b: [S; 3]) -> S {
    let u = sub(b, a);
    let w = sub(p, a);
    let q = dot(w, u) / dot(u, u);
    let d = std::array::from_fn(|k| w[k] - q * u[k]);
    dot(d, d).sqrt()
}
pub fn point_face_distance<S: Scalar>(p: [S; 3], a: [S; 3], b: [S; 3], c: [S; 3]) -> S {
    let n = cross(sub(b, a), sub(c, a));
    let h = dot(sub(p, a), n) / dot(n, n).sqrt();
    if h.value() < 0.0 { -h } else { h }
}
pub fn edge_edge_distance<S: Scalar>(a: [S; 3], b: [S; 3], c: [S; 3], d: [S; 3]) -> S {
    let n = cross(sub(b, a), sub(d, c));
    let h = dot(sub(a, c), n) / dot(n, n).sqrt();
    if h.value() < 0.0 { -h } else { h }
}

/// Initial-state diagnostic only. OGC maintains separation; it cannot undo an
/// already intersecting mesh. Shared topological features are handled by caller.
pub fn segment_triangle(a: DVec3, b: DVec3, t: [DVec3; 3]) -> bool {
    let [p, q, r] = t;
    let e = q - p;
    let f = r - p;
    let d = b - a;
    let h = d.cross(f);
    let det = e.dot(h);
    if det.abs() < 1e-14 {
        return false;
    }
    let s = a - p;
    let u = s.dot(h) / det;
    let z = s.cross(e);
    let v = d.dot(z) / det;
    let k = f.dot(z) / det;
    u >= -1e-10 && v >= -1e-10 && u + v <= 1.0 + 1e-10 && (1e-10..1.0 - 1e-10).contains(&k)
}

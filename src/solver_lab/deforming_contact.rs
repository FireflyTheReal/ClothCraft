//! Research VF/EE distance rows and conservative straight-path certificates.
//! Both sides move. No collision candidate filtering or production integration.
use super::dense_contact::Row;
use crate::ogc::{
    feature_hierarchy::Pair,
    geometry::{closest_segments, closest_triangle},
    topology::Topology,
};
use glam::DVec3;
use serde::Serialize;

fn sides(pair: Pair, top: &Topology) -> Result<(Vec<usize>, Vec<usize>), String> {
    match pair {
        Pair::VertexFace([v, f]) if v < top.neighbors.len() => Ok((
            vec![v],
            top.triangles.get(f).ok_or("Invalid contact face")?.to_vec(),
        )),
        Pair::EdgeEdge([a, b]) => Ok((
            top.edges.get(a).ok_or("Invalid contact edge")?.to_vec(),
            top.edges.get(b).ok_or("Invalid contact edge")?.to_vec(),
        )),
        _ => Err("Deforming contact supports VF/EE pairs only".into()),
    }
}
fn validate(top: &Topology, x: &[DVec3]) -> Result<(), String> {
    if x.len() != top.neighbors.len() || x.iter().any(|p| !p.is_finite()) {
        return Err("Invalid deforming contact positions".into());
    }
    Ok(())
}
fn witness(pair: Pair, top: &Topology, x: &[DVec3]) -> (DVec3, DVec3) {
    match pair {
        Pair::VertexFace([v, f]) => (x[v], closest_triangle(x[v], f, x, top).0),
        Pair::EdgeEdge([a, b]) => {
            let [p, q] = top.edges[a].map(|i| x[i]);
            let [r, s] = top.edges[b].map(|i| x[i]);
            let (u, v) = closest_segments(p, q, r, s);
            (p.lerp(q, u), r.lerp(s, v))
        }
        _ => unreachable!(),
    }
}
/// Full distance Jacobian. At a nonunique closest witness this is one selected
/// limiting row, not a differentiability guarantee. Swept checks remain needed.
pub fn linearize(pair: Pair, top: &Topology, x: &[DVec3], clearance: f64) -> Result<Row, String> {
    validate(top, x)?;
    let (a, b) = sides(pair, top)?;
    if !clearance.is_finite() || clearance <= 0. {
        return Err("Invalid contact clearance".into());
    }
    let (p, q) = witness(pair, top, x);
    let delta = p - q;
    let distance = delta.length();
    if !distance.is_finite() || distance <= 1e-14 {
        return Err("Contact row requires separated primitives".into());
    }
    let normal = delta / distance;
    let (wa, wb) = match pair {
        Pair::VertexFace(_) => {
            let u = x[b[1]] - x[b[0]];
            let v = x[b[2]] - x[b[0]];
            let w = q - x[b[0]];
            let uu = u.length_squared();
            let vv = v.length_squared();
            let uv = u.dot(v);
            let det = uu * vv - uv * uv;
            if !det.is_finite() || det <= 16. * f64::EPSILON * uu * vv {
                return Err("Degenerate/ill-conditioned contact triangle".into());
            }
            let s = (w.dot(u) * vv - w.dot(v) * uv) / det;
            let t = (w.dot(v) * uu - w.dot(u) * uv) / det;
            (vec![1.], vec![1. - s - t, s, t])
        }
        Pair::EdgeEdge(_) => {
            let (s, t) = closest_segments(x[a[0]], x[a[1]], x[b[0]], x[b[1]]);
            if x[a[0]].distance_squared(x[a[1]]) <= 1e-24
                || x[b[0]].distance_squared(x[b[1]]) <= 1e-24
            {
                return Err("Degenerate contact edge".into());
            }
            (vec![1. - s, s], vec![1. - t, t])
        }
        _ => unreachable!(),
    };
    let mut jacobian = vec![0.; x.len() * 3];
    for (ids, weights, sign) in [(&a, &wa, 1.), (&b, &wb, -1.)] {
        for (&i, &weight) in ids.iter().zip(weights) {
            for axis in 0..3 {
                jacobian[3 * i + axis] += sign * weight * normal[axis];
            }
        }
    }
    if jacobian.iter().any(|v| !v.is_finite()) {
        return Err("Nonfinite contact Jacobian".into());
    }
    Ok(Row {
        jacobian,
        offset: clearance - distance,
    })
}
#[derive(Debug, Serialize, PartialEq)]
pub enum Certificate {
    Safe {
        intervals: usize,
    },
    Violated {
        time: f64,
        distance_m: f64,
    },
    Unresolved {
        begin: f64,
        end: f64,
        intervals: usize,
    },
}
/// Certify linear trajectories of two convex finite primitives using separating
/// planes. For a fixed normal, support separation at BOTH interval endpoints
/// bounds the whole interval. Subdivision resolves rotating separating planes.
/// Exhaustion is Unresolved, never Safe. tolerance is explicit clearance slack.
pub fn certify(
    pair: Pair,
    top: &Topology,
    from: &[DVec3],
    to: &[DVec3],
    clearance: f64,
    tolerance: f64,
    max_depth: usize,
) -> Result<Certificate, String> {
    certify_impl(pair, top, from, to, clearance, tolerance, max_depth, false)
}

/// Also try an affine normal between endpoint separating directions.
/// For each vertex pair, its projection is quadratic in time. The three
/// Bernstein coefficients bound it on the entire interval. Convexity bounds
/// the interpolated normal's norm by its endpoint norms. All coefficients must
/// certify the same clearance; unresolved intervals still fail closed.
pub fn certify_moving_planes(
    pair: Pair,
    top: &Topology,
    from: &[DVec3],
    to: &[DVec3],
    clearance: f64,
    tolerance: f64,
    max_depth: usize,
) -> Result<Certificate, String> {
    certify_impl(pair, top, from, to, clearance, tolerance, max_depth, true)
}

#[allow(clippy::too_many_arguments)]
fn certify_impl(
    pair: Pair,
    top: &Topology,
    from: &[DVec3],
    to: &[DVec3],
    clearance: f64,
    tolerance: f64,
    max_depth: usize,
    moving_plane: bool,
) -> Result<Certificate, String> {
    validate(top, from)?;
    validate(top, to)?;
    let (a, b) = sides(pair, top)?;
    if !clearance.is_finite()
        || !tolerance.is_finite()
        || tolerance <= 0.
        || tolerance >= clearance
        || max_depth > 20
    {
        return Err("Invalid swept-contact settings".into());
    }
    let at = |time: f64| {
        from.iter()
            .zip(to)
            .map(|(p, q)| p.lerp(*q, time))
            .collect::<Vec<_>>()
    };
    let support = |x: &[DVec3], n: DVec3| {
        a.iter().map(|i| n.dot(x[*i])).fold(f64::INFINITY, f64::min)
            - b.iter()
                .map(|i| n.dot(x[*i]))
                .fold(f64::NEG_INFINITY, f64::max)
    };
    let threshold = clearance - tolerance;
    let mut stack = vec![(0., 1., 0usize)];
    let mut intervals = 0;
    while let Some((lo, hi, depth)) = stack.pop() {
        intervals += 1;
        // Bound cost as well as recursion depth. An unresolved path is rejected.
        if intervals > 4096 {
            return Ok(Certificate::Unresolved {
                begin: lo,
                end: hi,
                intervals,
            });
        }
        let left = at(lo);
        let right = at(hi);
        let mid = at(0.5 * (lo + hi));
        let mut safe = false;
        let mut normals = [DVec3::ZERO; 2];
        for (sample, (time, x)) in [(lo, &left), (hi, &right), (0.5 * (lo + hi), &mid)]
            .into_iter()
            .enumerate()
        {
            let (p, q) = witness(pair, top, x);
            let d = p.distance(q);
            if !d.is_finite() {
                return Err("Nonfinite swept distance".into());
            }
            if d < threshold {
                return Ok(Certificate::Violated {
                    time,
                    distance_m: d,
                });
            }
            let n = (p - q) / d;
            if sample < 2 {
                normals[sample] = n;
            }
            // Roundoff allowance is subtracted from the geometric lower bound.
            let magnitude = a
                .iter()
                .chain(&b)
                .map(|i| left[*i].length().max(right[*i].length()))
                .fold(1., f64::max);
            let roundoff = 64. * f64::EPSILON * magnitude;
            if support(&left, n).min(support(&right, n)) - roundoff >= threshold {
                safe = true;
                break;
            }
        }
        if !safe && moving_plane {
            let magnitude = a
                .iter()
                .chain(&b)
                .map(|i| left[*i].length().max(right[*i].length()))
                .fold(1., f64::max);
            // Account for dot products, coefficient accumulation and rounded
            // endpoint normalization. This remains an f64 conservative test,
            // not a formally verified interval-arithmetic implementation.
            let roundoff = 256. * f64::EPSILON * magnitude;
            let norm_bound = normals[0].length().max(normals[1].length()) + 16. * f64::EPSILON;
            let required = threshold * norm_bound;
            safe = a.iter().all(|&i| {
                b.iter().all(|&j| {
                    let r0 = left[i] - left[j];
                    let r1 = right[i] - right[j];
                    let coefficients = [
                        normals[0].dot(r0),
                        0.5 * (normals[0].dot(r1) + normals[1].dot(r0)),
                        normals[1].dot(r1),
                    ];
                    coefficients
                        .iter()
                        .all(|q| q.is_finite() && q - roundoff >= required)
                })
            });
        }
        if !safe {
            if depth == max_depth {
                return Ok(Certificate::Unresolved {
                    begin: lo,
                    end: hi,
                    intervals,
                });
            }
            let mid = 0.5 * (lo + hi);
            stack.push((mid, hi, depth + 1));
            stack.push((lo, mid, depth + 1));
        }
    }
    Ok(Certificate::Safe { intervals })
}

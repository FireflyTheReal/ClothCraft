//! Ray picking independent of the UI. Attachments retain barycentric weights.
use glam::DVec3;
pub fn ray_triangle(
    origin: DVec3,
    direction: DVec3,
    points: [DVec3; 3],
) -> Option<(f64, [f64; 3])> {
    let [a, b, c] = points;
    let e = b - a;
    let f = c - a;
    let p = direction.cross(f);
    let det = e.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let s = origin - a;
    let u = s.dot(p) / det;
    let q = s.cross(e);
    let v = direction.dot(q) / det;
    let distance = f.dot(q) / det;
    if u < 0. || v < 0. || u + v > 1. || distance <= 0. {
        None
    } else {
        Some((distance, [1. - u - v, u, v]))
    }
}
pub fn ray_plane(origin: DVec3, direction: DVec3, point: DVec3, normal: DVec3) -> Option<DVec3> {
    let denominator = direction.dot(normal);
    if denominator.abs() < 1e-10 {
        return None;
    }
    let t = (point - origin).dot(normal) / denominator;
    if t > 0. {
        Some(origin + direction * t)
    } else {
        None
    }
}

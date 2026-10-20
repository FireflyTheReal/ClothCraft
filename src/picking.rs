//! Ray picking independent of the UI. Attachments retain barycentric weights.
use glam::DVec3;
/// Closest cloth hit, excluding cloth hidden by the avatar. No backface culling.
pub fn visible_cloth_face(
    scene: &crate::fitting::FittingScene,
    origin: DVec3,
    direction: DVec3,
) -> Option<usize> {
    let (face, distance) = scene
        .garment
        .mesh
        .triangles
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            ray_triangle(origin, direction, t.vertices.map(|v| scene.state.x[v]))
                .map(|(d, _)| (i, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))?;
    (!scene
        .collider
        .occludes_ray(origin, direction, distance - 1e-5))
    .then_some(face)
}
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

//! CPU-only visibility benchmark; deliberately separate from solver/window FPS.
use glam::DVec3;
use marvelous_cloth::{
    avatar::{MeshCollider, SurfaceMesh},
    picking::ray_triangle,
};
use std::{hint::black_box, path::Path, sync::Arc, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let surface = Arc::new(SurfaceMesh::load_obj(Path::new(
        "assets/makehuman/mannequin.obj",
    ))?);
    let collider = MeshCollider::new(surface.clone(), 1000.)?;
    let rays: Vec<_> = (0..4096)
        .map(|i| {
            let x = (i % 64) as f64 / 63. * 2. - 1.;
            let y = (i / 64) as f64 / 63. * 2.;
            (DVec3::new(x, y, 2.), -DVec3::Z, 1.5 + (i % 3) as f64 * 0.4)
        })
        .collect();
    let brute = |&(o, d, t): &(DVec3, DVec3, f64)| {
        surface.triangles.iter().any(|tri| {
            ray_triangle(o, d, tri.map(|v| surface.vertices[v])).is_some_and(|(hit, _)| hit < t)
        })
    };
    let fast = |&(o, d, t): &(DVec3, DVec3, f64)| collider.occludes_ray(o, d, t);
    let reference: Vec<_> = rays.iter().map(brute).collect();
    assert_eq!(reference, rays.iter().map(fast).collect::<Vec<_>>());
    let measure = |f: &dyn Fn(&(DVec3, DVec3, f64)) -> bool| {
        let start = Instant::now();
        for _ in 0..3 {
            for ray in &rays {
                black_box(f(black_box(ray)));
            }
        }
        start.elapsed().as_secs_f64() * 1000. / 3.
    };
    let exhaustive_ms = measure(&brute);
    let bvh_ms = measure(&fast);
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "scope":"CPU avatar visibility only, identical rays and triangle predicate; not simulation FPS",
        "rays":rays.len(),"hits":reference.iter().filter(|&&hit|hit).count(),
        "triangles":surface.triangles.len(),"equal_results":true,
            "exhaustive_ms":exhaustive_ms,"bvh_ms":bvh_ms,"speedup":exhaustive_ms/bvh_ms
        }))?
    );
    Ok(())
}

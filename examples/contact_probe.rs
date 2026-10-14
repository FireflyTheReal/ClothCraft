//! Diagnose contact derivatives at a saved fitting state, CPU only.
use marvelous_cloth::{contact::Collider, fitting::FittingProject};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("Pass fitting-project.json")?;
    let s = FittingProject::load(std::path::Path::new(&path))?.compile()?;
    let mut errors = vec![];
    let eps = 1e-7;
    for (i, x) in s.state.x.iter().enumerate() {
        let thick = s.garment.materials[s.garment.vertex_material[i]].thickness;
        let area = s.garment.mesh.rest_area[i];
        if let Some(e) = s.collider.element(i, *x, thick, area) {
            for k in 0..3 {
                let mut a = *x;
                let mut b = *x;
                a[k] += eps;
                b[k] -= eps;
                let numerical = (s.collider.value(a, thick, area)
                    - s.collider.value(b, thick, area))
                    / (2.0 * eps);
                let err = (numerical - e.gradient[k]).abs() / (1.0 + numerical.abs());
                if err > 1e-5 {
                    errors.push((i, k, err, numerical, e.gradient[k]));
                }
            }
        }
    }
    println!("{}", serde_json::to_string_pretty(&errors)?);
    if !errors.is_empty() {
        return Err("Contact derivative mismatch".into());
    }
    Ok(())
}

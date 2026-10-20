//! Shared synthetic fixture; no private garment data.
use glam::DVec3;
use marvelous_cloth::{avatar::SurfaceMesh, garment::GarmentTemplate, model::Pattern};
pub const GAP: f64 = 0.001;
pub fn template(ease: Option<f64>, fold_degrees: f64) -> Result<GarmentTemplate, String> {
    if !fold_degrees.is_finite()
        || !(0. ..=60.).contains(&fold_degrees)
        || ease.is_some_and(|e| !e.is_finite() || !(0. ..=0.3).contains(&e))
    {
        return Err("Fold must be 0..60 degrees and easing 0..0.3".into());
    }
    let mut t = GarmentTemplate::two_panel_demo();
    if let Some(ease) = ease {
        let theta = fold_degrees.to_radians();
        for (i, p) in t.panels.iter_mut().enumerate() {
            p.pins.clear();
            let h = 0.04 * if i == 1 { 1. + ease } else { 1. };
            p.pattern = Pattern {
                width: 0.03,
                height: h,
                nx: 1,
                ny: 2,
            };
            p.placement.rotation_degrees =
                DVec3::new(90., if i == 0 { -fold_degrees } else { fold_degrees }, 0.);
            p.placement.translation = if i == 0 {
                DVec3::new(
                    -0.001 - 0.03 * theta.cos(),
                    0.004 + 0.03 * theta.sin(),
                    h * 0.5,
                )
            } else {
                DVec3::new(0.001, 0.004, h * 0.5)
            };
        }
        t.seams[0].allow_ease = true;
        let mut support = t.panels[0].clone();
        support.id = "support".into();
        support.name = "Deformable lower layer".into();
        support.pattern = Pattern {
            width: 0.12,
            height: 0.1,
            nx: 1,
            ny: 1,
        };
        support.placement.rotation_degrees = DVec3::new(90., 0., 0.);
        support.placement.translation = DVec3::new(-0.06, GAP, 0.05);
        t.panels.push(support);
        return Ok(t);
    }
    t.seams.clear();
    for (i, p) in t.panels.iter_mut().enumerate() {
        p.pins.clear();
        let w = if i == 0 { 0.06 } else { 0.03 };
        p.pattern = Pattern {
            width: w,
            height: w,
            nx: 1,
            ny: 1,
        };
        p.placement.rotation_degrees = DVec3::new(90., 0., 0.);
        p.placement.translation = DVec3::new(
            -w * 0.5 + if i == 1 { 0.002 } else { 0. },
            if i == 0 { GAP } else { 0.004 },
            -w * 0.5,
        );
    }
    Ok(t)
}
pub fn floor() -> SurfaceMesh {
    SurfaceMesh {
        vertices: vec![
            DVec3::new(-1., 0., -1.),
            DVec3::new(1., 0., -1.),
            DVec3::new(0., 0., 1.),
        ],
        triangles: vec![[0, 1, 2]],
    }
}

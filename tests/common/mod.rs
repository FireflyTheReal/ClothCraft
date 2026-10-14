use glam::DVec3;
use marvelous_cloth::{
    avatar::SurfaceMesh,
    garment::{CompiledGarment, GarmentTemplate},
};
use std::sync::Arc;

pub fn resting_patch(offset: DVec3) -> (Arc<CompiledGarment>, SurfaceMesh) {
    let mut t = GarmentTemplate::two_panel_demo();
    t.panels.truncate(1);
    t.seams.clear();
    let p = &mut t.panels[0];
    p.pins.clear();
    p.pattern = marvelous_cloth::model::Pattern {
        width: 0.12,
        height: 0.12,
        nx: 3,
        ny: 3,
    };
    p.placement.rotation_degrees = DVec3::new(90., 0., 0.);
    // Fifty micrometres outside the 3 mm shell. The very next gravity step
    // enters it; that interaction must act in that same implicit solve.
    p.placement.translation = offset + DVec3::new(-0.06, 0.00305, 0.06);
    let floor = SurfaceMesh {
        vertices: vec![
            DVec3::new(-1., 0., -1.),
            DVec3::new(1., 0., -1.),
            DVec3::new(0., 0., 1.),
        ]
        .into_iter()
        .map(|p| p + offset)
        .collect(),
        triangles: vec![[0, 1, 2]],
    };
    (Arc::new(t.compile().unwrap()), floor)
}

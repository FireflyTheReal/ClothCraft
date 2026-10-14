use glam::DVec3;
use marvelous_cloth::garment::{GarmentTemplate, diagnostics};

#[test]
fn extension_diagnostic_uses_material_rest_and_is_rigid_motion_invariant() {
    let mut t = GarmentTemplate::two_panel_demo();
    t.panels.truncate(1);
    t.seams.clear();
    let g = t.compile().unwrap();
    let mut s = g.initial_state.clone();
    assert!(diagnostics::extension_percent(&g, &s)[1] < 1e-10);
    for p in &mut s.x {
        *p = DVec3::new(-p.y * 1.2, p.x * 1.2, p.z) + DVec3::splat(2.);
    }
    let values = diagnostics::extension_percent(&g, &s);
    for v in values {
        assert!((v - 20.).abs() < 1e-10);
    }
}

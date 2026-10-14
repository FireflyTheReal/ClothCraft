use glam::DVec3;
use marvelous_cloth::{
    garment::GarmentTemplate,
    picking,
    vbd::{Grab, VbdSettings, VbdSolver},
};
use std::sync::Arc;
mod common;
#[test]
fn contact_activates_during_the_step_and_settles_without_a_sawtooth() {
    let (g, floor) = common::resting_patch(DVec3::ZERO);
    let mut s = g.initial_state.clone();
    let mut solver = VbdSolver::new(g, Some(&floor), &s).unwrap();
    let settings = VbdSettings {
        dt: 1. / 60.,
        ..Default::default()
    };
    let mut max_delta = 0.0_f64;
    for i in 0..60 {
        let old = s.x.clone();
        solver.step(&mut s, &settings, None).unwrap();
        for (x, prev) in s.x.iter().zip(old) {
            assert!(
                (x.y - settings.contact.radius).abs() < 1e-5,
                "step {i}: {}",
                x.y
            );
            if i > 10 {
                max_delta = max_delta.max((x.y - prev.y).abs());
            }
        }
    }
    assert!(max_delta < 1e-6, "normal contact motion {max_delta} m");
}
fn fixture() -> Arc<marvelous_cloth::garment::CompiledGarment> {
    let mut t = GarmentTemplate::two_panel_demo();
    t.panels.truncate(1);
    t.seams.clear();
    t.panels[0].pins.clear();
    t.panels[0].pattern.nx = 2;
    t.panels[0].pattern.ny = 2;
    Arc::new(t.compile().unwrap())
}
#[test]
fn free_fall_preserves_rest_shape_and_backward_euler_velocity() {
    let g = fixture();
    let mut s = g.initial_state.clone();
    let old = s.clone();
    let mut solver = VbdSolver::new(g, None, &s).unwrap();
    let settings = VbdSettings {
        dt: 0.005,
        damping: 0.0,
        iterations: 3,
        ..Default::default()
    };
    solver.step(&mut s, &settings, None).unwrap();
    for i in 0..s.x.len() {
        assert!((s.v[i] - settings.gravity * settings.dt).length() < 1e-9);
        assert!((s.x[i] - old.x[i] - settings.gravity * settings.dt.powi(2)).length() < 1e-12);
    }
}
#[test]
fn mouse_attachment_pulls_cloth_and_invalid_input_is_transactional() {
    let g = fixture();
    let mut s = g.initial_state.clone();
    let mut solver = VbdSolver::new(g.clone(), None, &s).unwrap();
    let vertices = g.mesh.triangles[0].vertices;
    let center = vertices.iter().map(|i| s.x[*i] / 3.).sum::<DVec3>();
    let grab = Grab {
        vertices,
        weights: [1. / 3.; 3],
        target: center + DVec3::Z * 0.01,
        stiffness: 100.,
    };
    let settings = VbdSettings {
        gravity: DVec3::ZERO,
        iterations: 6,
        ..Default::default()
    };
    solver.step(&mut s, &settings, Some(&grab)).unwrap();
    let moved = vertices.iter().map(|i| s.x[*i] / 3.).sum::<DVec3>();
    assert!(moved.z > center.z);
    let old = s.clone();
    let bad = Grab {
        target: DVec3::NAN,
        ..grab
    };
    assert!(solver.step(&mut s, &settings, Some(&bad)).is_err());
    assert_eq!(s.x, old.x);
    assert_eq!(s.time, old.time);
}
#[test]
fn picking_retains_material_attachment_and_rejects_behind_camera() {
    let t = [DVec3::ZERO, DVec3::X, DVec3::Y];
    let origin = DVec3::new(0.2, 0.3, 1.);
    let (d, w) = picking::ray_triangle(origin, -DVec3::Z, t).unwrap();
    assert!((d - 1.).abs() < 1e-12);
    assert!((w[0] - 0.5).abs() < 1e-12);
    assert!(picking::ray_triangle(origin, DVec3::Z, t).is_none());
    let p = picking::ray_plane(origin, -DVec3::Z, DVec3::ZERO, DVec3::Z).unwrap();
    assert!((p - DVec3::new(0.2, 0.3, 0.)).length() < 1e-12);
}

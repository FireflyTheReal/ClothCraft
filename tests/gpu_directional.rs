//! Hardware acceptance gates for sparse directional contacts. Explicit opt-in.
#![cfg(feature = "desktop")]
use eframe::wgpu;
use glam::DVec3;
use marvelous_cloth::{
    garment::GarmentTemplate,
    gpu_vbd::GpuVbd,
    model::Pattern,
    vbd::{VbdSettings, VbdSolver},
};
use std::sync::Arc;
mod common;

fn gpu() -> GpuVbd {
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build_global()
        .ok();
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .unwrap();
    assert_ne!(adapter.get_info().device_type, wgpu::DeviceType::Cpu);
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let mut gpu = GpuVbd::new(device, queue);
    gpu.check_contacts = true;
    gpu
}

#[test]
#[ignore = "requires an explicitly authorized hardware GPU"]
fn collapsed_patch_preserves_initial_tangential_velocity_and_rejoins_ordinary_path() {
    let gpu = gpu();
    for gap in [3e-6, 2e-5] {
        for offset in [DVec3::ZERO, DVec3::new(50., 20., -30.)] {
            let mut t = GarmentTemplate::two_panel_demo();
            t.panels.truncate(1);
            t.seams.clear();
            t.panels[0].pins.clear();
            t.panels[0].pattern = Pattern {
                width: 0.02,
                height: 0.02,
                nx: 1,
                ny: 1,
            };
            let g = Arc::new(t.compile().unwrap());
            let mut state = g.initial_state.clone();
            // Use the actual compiled shared edge, never a mesher-specific
            // diagonal convention. The material rest square is unchanged.
            assert_eq!(g.mesh.hinges.len(), 1);
            let vertices = g.mesh.hinges[0].vertices;
            for (v, p) in vertices.into_iter().zip([
                DVec3::new(-0.01, 0., 0.),
                DVec3::new(0.01, 0., 0.),
                DVec3::new(0., gap, 0.),
                DVec3::new(0., -0.01, 0.),
            ]) {
                state.x[v] = p + offset;
            }
            state.v.fill(DVec3::Z * 0.1);
            let mut solver = VbdSolver::new(g.clone(), None, &state).unwrap();
            let settings = VbdSettings {
                dt: 1. / 60.,
                damping: 0.,
                gravity: DVec3::ZERO,
                ..Default::default()
            };
            let contacts = solver
                .world
                .detect(&state.x, 0.006, &settings.contact)
                .unwrap();
            assert!((contacts.minimum_distance - gap).abs() < 1e-12);
            assert_eq!(contacts.directional.vertex_count(), 4);
            let total_mass: f64 = g.mesh.masses.iter().sum();
            let center = |state: &marvelous_cloth::model::State| -> f64 {
                state
                    .x
                    .iter()
                    .zip(&g.mesh.masses)
                    .map(|(x, m)| (x.z - offset.z) * m / total_mass)
                    .sum()
            };
            let metrics = gpu.step(&mut solver, &mut state, &settings, None).unwrap();
            assert_eq!(metrics.directional_vertices, 4);
            let expected = settings.dt * 0.1;
            assert!(
                (center(&state) - expected).abs() < 2e-5,
                "center {}, expected {expected}",
                center(&state)
            );
            solver
                .world
                .validate_start(&solver.world.positions(&state.x))
                .unwrap();
            // Elastic recovery opens the collapsed face. Subsequent ordinary
            // isotropic truncation is NOT Galilean invariant under this large
            // strain: do not incorrectly require exact momentum conservation
            // from the pre-existing finite-iteration solver. Require bitwise
            // equivalence when returning to that path instead.
            let mut baseline = state.clone();
            let mut ordinary = VbdSolver::new(g.clone(), None, &baseline).unwrap();
            ordinary.world.directional_bounds = false;
            let m = gpu.step(&mut solver, &mut state, &settings, None).unwrap();
            assert_eq!(m.directional_vertices, 0);
            gpu.step(&mut ordinary, &mut baseline, &settings, None)
                .unwrap();
            assert_eq!(state.x, baseline.x);
            assert_eq!(state.v, baseline.v);
        }
    }
}

#[test]
#[ignore = "requires an explicitly authorized hardware GPU"]
fn ordinary_supported_motion_is_bit_identical_without_close_directional_pairs() {
    let gpu = gpu();
    let (g, floor) = common::resting_patch(DVec3::ZERO);
    let mut a = g.initial_state.clone();
    let mut b = a.clone();
    let mut sa = VbdSolver::new(g.clone(), Some(&floor), &a).unwrap();
    let mut sb = VbdSolver::new(g, Some(&floor), &b).unwrap();
    sa.world.directional_bounds = false;
    let settings = VbdSettings {
        dt: 1. / 60.,
        ..Default::default()
    };
    for _ in 0..48 {
        gpu.step(&mut sa, &mut a, &settings, None).unwrap();
        let m = gpu.step(&mut sb, &mut b, &settings, None).unwrap();
        assert_eq!(m.directional_vertices, 0);
        assert_eq!(a.x, b.x);
        assert_eq!(a.v, b.v);
    }
}

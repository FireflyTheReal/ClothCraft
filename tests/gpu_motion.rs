//! Explicit hardware regression. Never run as part of ordinary CPU checks.
#![cfg(feature = "desktop")]
use eframe::wgpu;
use glam::DVec3;
use marvelous_cloth::{
    garment::GarmentTemplate,
    gpu_vbd::GpuVbd,
    vbd::{VbdSettings, VbdSolver},
};
use std::sync::Arc;

#[test]
#[ignore = "uses a hardware GPU; run explicitly with --ignored"]
fn gpu_dense_free_panel_falls_and_rebuilds_after_topology_growth() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .unwrap();
    assert_ne!(adapter.get_info().device_type, wgpu::DeviceType::Cpu);
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let gpu = GpuVbd::new(device, queue);
    // Reuse the same GPU allocations across an actual change in topology.
    for (nx, ny) in [(6, 4), (20, 12)] {
        let mut t = GarmentTemplate::two_panel_demo();
        t.panels.truncate(1);
        t.seams.clear();
        let p = &mut t.panels[0];
        p.pins.clear();
        p.pattern.nx = nx;
        p.pattern.ny = ny;
        let g = Arc::new(t.compile().unwrap());
        let mut state = g.initial_state.clone();
        let old = state.clone();
        let mut solver = VbdSolver::new(g.clone(), None, &state).unwrap();
        let settings = VbdSettings {
            dt: 1. / 60.,
            iterations: 8,
            ..Default::default()
        };
        gpu.step(&mut solver, &mut state, &settings, None).unwrap();
        let expected =
            settings.gravity * settings.dt.powi(2) / (1. + settings.damping * settings.dt);
        let mean = state
            .x
            .iter()
            .zip(&old.x)
            .map(|(a, b)| *a - *b)
            .sum::<DVec3>()
            / state.x.len() as f64;
        assert!(
            (mean - expected).length() < 1e-5,
            "{nx}x{ny}: actual {mean:?}, expected {expected:?}"
        );
        for (a, b) in state.x.iter().zip(&old.x) {
            assert!(
                (*a - *b - expected).length() < 2e-5,
                "free fall must not stretch the cloth"
            );
        }
        // Large motion is still limited by the original OGC trust regions.
        state.v.fill(DVec3::new(0., -10., 0.));
        let before = state.clone();
        let contacts = solver
            .world
            .detect(
                &solver.world.positions(&state.x),
                settings.contact.radius * 4.,
                &settings.contact,
            )
            .unwrap();
        gpu.step(&mut solver, &mut state, &settings, None).unwrap();
        for (i, p) in state.x.iter().enumerate() {
            assert!(p.distance(before.x[i]) <= contacts.radii[i] + 1e-8);
        }
    }
}

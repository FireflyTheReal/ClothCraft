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
mod common;

#[test]
#[ignore = "uses a hardware GPU; run explicitly with --ignored"]
fn gpu_contact_settles_and_is_invariant_under_world_translation() {
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
    let settings = VbdSettings {
        dt: 1. / 60.,
        ..Default::default()
    };
    let mut reference = None::<Vec<DVec3>>;
    for offset in [DVec3::ZERO, DVec3::new(100., 101.4, -100.)] {
        let (g, floor) = common::resting_patch(offset);
        let mut s = g.initial_state.clone();
        let mut solver = VbdSolver::new(g, Some(&floor), &s).unwrap();
        let mut jitter = 0.0_f64;
        for i in 0..120 {
            gpu.check_gradients = i < 2;
            let old = s.x.clone();
            gpu.step(&mut solver, &mut s, &settings, None).unwrap();
            for (x, prev) in s.x.iter().zip(old) {
                assert!(
                    (x.y - offset.y - settings.contact.radius).abs() < 1e-5,
                    "step {i}: {x:?}"
                );
                if i > 10 {
                    jitter = jitter.max((x.y - prev.y).abs());
                }
            }
        }
        eprintln!("resting contact at {offset}: max normal step displacement {jitter:.9} m");
        assert!(jitter < 1e-6, "normal contact motion {jitter} m");
        let local: Vec<_> = s.x.iter().map(|p| *p - offset).collect();
        if let Some(reference) = &reference {
            for (a, b) in local.iter().zip(reference) {
                assert!(a.distance(*b) < 2e-6);
            }
        } else {
            reference = Some(local);
        }
    }
}

#[test]
#[ignore = "uses a hardware GPU; run explicitly with --ignored"]
fn supported_cloth_does_not_accumulate_artificial_gravity_stretch() {
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
    let mut t = GarmentTemplate::two_panel_demo();
    t.version = 2;
    t.panels.truncate(1);
    t.seams.clear();
    let panel = &mut t.panels[0];
    panel.pattern = marvelous_cloth::model::Pattern {
        width: 0.2,
        height: 0.4,
        nx: 4,
        ny: 16,
    };
    panel.grain_degrees = 90.;
    panel.pins.clear();
    panel.pin_points = (0..=4)
        .map(|i| glam::DVec2::new(i as f64 * 0.05, 0.))
        .collect();
    let g = Arc::new(t.compile().unwrap());
    let mut state = g.initial_state.clone();
    let mut solver = VbdSolver::new(g.clone(), None, &state).unwrap();
    let settings = VbdSettings {
        dt: 1. / 60.,
        iterations: 16,
        ..Default::default()
    };
    let hem: Vec<_> = g
        .mesh
        .uv
        .iter()
        .enumerate()
        .filter_map(|(i, uv)| (uv.y > 0.399).then_some(i))
        .collect();
    let extension = |s: &marvelous_cloth::model::State| {
        hem.iter()
            .map(|i| (g.initial_state.x[*i].y - s.x[*i].y) / 0.4)
            .sum::<f64>()
            / hem.len() as f64
    };
    let mut middle = 0.;
    for i in 0..300 {
        gpu.step(&mut solver, &mut state, &settings, None).unwrap();
        if i == 179 {
            middle = extension(&state);
        }
    }
    let final_extension = extension(&state);
    eprintln!("supported cloth: extension at 3s={middle:.6}, 5s={final_extension:.6}");
    // The self-weight load is <0.8 N/m; this material's tangent modulus near
    // rest is ~127 N/m. A 2% mean extension bound leaves room for dynamics,
    // transverse coupling and incomplete convergence, not rubber-like creep.
    assert!(
        (0.0001..0.02).contains(&final_extension),
        "{final_extension}"
    );
    assert!(
        (final_extension - middle).abs() < 0.003,
        "creep {middle} -> {final_extension}"
    );
    for (i, pin) in g.mesh.pins.iter().enumerate() {
        if let Some(p) = pin {
            assert!(state.x[i].distance(*p) < 1e-7);
        }
    }
}

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

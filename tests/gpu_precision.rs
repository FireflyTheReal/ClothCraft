//! Opt-in precision regression. The fixture contains no user garment data.
#![cfg(feature = "desktop")]
use eframe::wgpu;
use glam::DVec3;
use marvelous_cloth::{
    gpu_vbd::GpuVbd,
    vbd::{VbdSettings, VbdSolver},
};
mod common;

#[test]
#[ignore = "uses a hardware GPU; run explicitly with --ignored"]
fn micrometre_contact_releases_without_enlarging_collision_balls() {
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
    let gpu = GpuVbd::new(device, queue);
    let settings = VbdSettings {
        dt: 1. / 60.,
        gravity: DVec3::ZERO,
        ..Default::default()
    };
    for initial_gap in [4.26445598e-6, 2e-7] {
        for offset in [DVec3::ZERO, DVec3::new(100., 101.4, -100.)] {
            let (g, avatar) = common::resting_patch(offset);
            let mut state = g.initial_state.clone();
            for x in &mut state.x {
                x.y = offset.y + initial_gap;
            }
            let mut solver = VbdSolver::new(g, Some(&avatar), &state).unwrap();
            let mut repaired = 0;
            for _ in 0..24 {
                let before = state.clone();
                let contacts = solver
                    .world
                    .detect(
                        &solver.world.positions(&state.x),
                        settings.contact.radius * 2.,
                        &settings.contact,
                    )
                    .unwrap();
                let metrics = gpu.step(&mut solver, &mut state, &settings, None).unwrap();
                repaired += metrics.precision_vertices;
                for (i, x) in state.x.iter().enumerate() {
                    let f64_roundoff = 64. * f64::EPSILON * x.abs().max_element().max(1.);
                    assert!(x.distance(before.x[i]) <= contacts.radii[i] + f64_roundoff);
                    assert!(x.y > offset.y, "must remain outside the avatar");
                }
                solver
                    .world
                    .validate_start(&solver.world.positions(&state.x))
                    .unwrap();
            }
            let gap = state
                .x
                .iter()
                .map(|x| x.y - offset.y)
                .fold(f64::INFINITY, f64::min);
            eprintln!("offset={offset:?}: minimum gap {gap:.9} m, f64 vertex updates {repaired}");
            assert!(repaired > 0);
            assert!(
                gap > 2e-5,
                "precision work must escape the stalled configuration: {gap}"
            );
        }
    }
}

#[test]
#[ignore = "uses a hardware GPU; run explicitly with --ignored"]
fn subgroup_and_portable_paths_preserve_motion_and_bounds() {
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
    let features = adapter.features() & wgpu::Features::SUBGROUP;
    let make = |features| {
        let (d, q) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: features,
            ..Default::default()
        }))
        .unwrap();
        GpuVbd::new(d, q)
    };
    let mut portable = make(wgpu::Features::empty());
    let mut native = make(features);
    let settings = VbdSettings {
        dt: 1. / 60.,
        ..Default::default()
    };
    for offset in [DVec3::ZERO, DVec3::new(50., 20., -30.)] {
        let (g, avatar) = common::resting_patch(offset);
        let mut a = g.initial_state.clone();
        let mut b = a.clone();
        let mut sa = VbdSolver::new(g.clone(), Some(&avatar), &a).unwrap();
        let mut sb = VbdSolver::new(g.clone(), Some(&avatar), &b).unwrap();
        let tri = g.mesh.triangles[0].vertices;
        let grab = marvelous_cloth::vbd::Grab {
            patch: Vec::new(),
            vertices: tri,
            weights: [1. / 3.; 3],
            target: tri.iter().map(|v| a.x[*v] / 3.).sum::<DVec3>() + DVec3::new(0.004, 0.006, 0.),
            stiffness: 30.,
        };
        for step in 0..24 {
            portable.check_gradients = step == 0;
            native.check_gradients = step == 0;
            portable
                .step(&mut sa, &mut a, &settings, Some(&grab))
                .unwrap();
            let metrics = native
                .step(&mut sb, &mut b, &settings, Some(&grab))
                .unwrap();
            if features.contains(wgpu::Features::SUBGROUP)
                && adapter.get_info().subgroup_min_size == 32
                && adapter.get_info().subgroup_max_size == 32
            {
                assert!(metrics.gpu_subgroups);
            }
            let deviation =
                a.x.iter()
                    .zip(&b.x)
                    .map(|(a, b)| a.distance(*b))
                    .fold(0., f64::max);
            assert!(
                deviation < 2e-6,
                "shader paths diverged: {deviation} m at step {step}"
            );
            assert!(b.x.iter().all(|p| p.y > offset.y));
        }
        sb.world.validate_start(&sb.world.positions(&b.x)).unwrap();
    }
}

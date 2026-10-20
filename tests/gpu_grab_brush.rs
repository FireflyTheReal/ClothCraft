//! Optional brush-drag acceptance. Ordinary test runs must not initialize a GPU.
#![cfg(feature = "desktop")]
use eframe::wgpu;
use glam::DVec3;
use marvelous_cloth::{
    gpu_vbd::GpuVbd,
    vbd::{Grab, VbdSettings, VbdSolver, grab::surface_patch},
};
mod common;

#[test]
#[ignore = "requires an explicitly authorized hardware GPU"]
fn brush_pull_hold_release_checks_forces_and_preserves_surface_separation() {
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
    // Compare packed GPU forces with f64 CPU forces at the same geometry, and
    // check GPU broadphase coverage against the full CPU contact detector.
    // Keeping these on after release also catches retained brush incidence.
    gpu.check_gradients = true;
    gpu.check_contacts = true;
    let (garment, floor) = common::resting_patch(DVec3::ZERO);
    let mut accelerated = garment.initial_state.clone();
    let mut reference = accelerated.clone();
    let mut hybrid = VbdSolver::new(garment.clone(), Some(&floor), &accelerated).unwrap();
    let mut cpu = VbdSolver::new(garment.clone(), Some(&floor), &reference).unwrap();
    let settings = VbdSettings {
        iterations: 16,
        ..Default::default()
    };
    let vertices = garment.mesh.triangles[0].vertices;
    let center = vertices
        .iter()
        .map(|&v| accelerated.x[v] / 3.)
        .sum::<DVec3>();
    let mut grab = Grab {
        vertices,
        weights: [1. / 3.; 3],
        target: center,
        stiffness: 300.,
        patch: surface_patch(
            &garment.mesh,
            &accelerated.x,
            garment.panels[0].triangles.clone(),
            vertices,
            center,
            0.15,
        ),
    };
    assert!(
        grab.patch.len() > 3,
        "This gate must exercise a brush, not just the picked face"
    );
    assert!(grab.valid_patch(accelerated.x.len()));
    let mean_x =
        |positions: &[DVec3]| positions.iter().map(|p| p.x).sum::<f64>() / positions.len() as f64;
    let initial_mean = mean_x(&accelerated.x);
    let mut release_positions = None;
    let mut max_difference = 0_f64;
    let mut detection_updates = [0; 2];
    for step in 0..36 {
        // Eight steps pulling sideways and INTO the floor, four holding,
        // followed by 24 unforced steps. The floor must keep resisting the
        // brush while tangential motion remains possible.
        let fraction = ((step + 1) as f64 / 8.).min(1.);
        grab.target = center + DVec3::new(0.02, -0.006, 0.) * fraction;
        let control = (step < 12).then_some(&grab);
        detection_updates[0] += cpu
            .step(&mut reference, &settings, control)
            .unwrap()
            .detection_updates;
        detection_updates[1] += gpu
            .step(&mut hybrid, &mut accelerated, &settings, control)
            .unwrap()
            .detection_updates;
        assert_eq!(reference.time, accelerated.time);
        for (a, b) in accelerated.x.iter().zip(&reference.x) {
            assert!(
                a.is_finite() && a.y > 0.,
                "step {step}: invalid/floor-crossing {a:?}"
            );
            max_difference = max_difference.max(a.distance(*b));
        }
        // Force parity is checked above at identical geometry. End-to-end
        // trajectory equality is not a contract of these existing numerical
        // paths: the point-only control also diverges under this protocol.
        // Record the difference without using it as a false equivalence gate.
        for (solver, state) in [(&mut hybrid, &accelerated), (&mut cpu, &reference)] {
            let positions = solver.world.positions(&state.x);
            solver.world.validate_start(&positions).unwrap();
            let contacts = solver
                .world
                .detect(&positions, 0.012, &settings.contact)
                .unwrap();
            assert!(
                contacts.minimum_distance > 0.,
                "step {step}: contact separation lost"
            );
            if control.is_none() {
                assert_eq!(
                    solver
                        .audit_energy(state, &settings, &contacts, None)
                        .unwrap()
                        .grab_j,
                    0.
                );
            }
        }
        if step == 11 {
            assert!(
                mean_x(&accelerated.x) > initial_mean + 2e-4
                    && mean_x(&reference.x) > initial_mean + 2e-4,
                "The brush must move the cloth tangentially, not just keep it collision-free"
            );
            release_positions = Some((accelerated.x.clone(), reference.x.clone()));
        }
    }
    let (release_gpu, release_cpu) = release_positions.unwrap();
    for (state, release) in [(&accelerated, &release_gpu), (&reference, &release_cpu)] {
        assert!(
            state
                .x
                .iter()
                .zip(release)
                .any(|(a, b)| a.distance(*b) > 1e-5),
            "Release must retain motion rather than freeze the brush vertices"
        );
    }
    eprintln!("36-step brush pull/hold/release: maximum CPU/GPU difference {max_difference:.9} m");
    eprintln!("Contact detections CPU/GPU: {detection_updates:?}");
}

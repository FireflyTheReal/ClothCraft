//! Opt-in equality against the complete f64 detector, including its movement
//! bounds (not only the subset of contacts that currently exert a force).
#![cfg(feature = "desktop")]
use eframe::wgpu;
use glam::DVec3;
use marvelous_cloth::{
    gpu_vbd::contact_filter::GpuContactFilter,
    ogc::{ContactSet, OgcSettings, OgcWorld},
};

fn equal(a: &ContactSet, b: &ContactSet) {
    assert_eq!(a.radii, b.radii);
    assert_eq!(a.minimum_distance, b.minimum_distance);
    assert_eq!(a.incident, b.incident);
    assert_eq!(a.active_contacts, b.active_contacts);
    assert_eq!(a.vertex_face_candidates, b.vertex_face_candidates);
    assert_eq!(a.edge_edge_candidates, b.edge_edge_candidates);
    let keys = |s: &ContactSet| {
        s.contacts
            .iter()
            .map(|c| (c.kind, c.vertices, c.count, c.mollifier_epsilon.to_bits()))
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(a), keys(b));
}

#[test]
#[ignore = "uses a hardware GPU; run explicitly with --ignored"]
fn gpu_bvh_preserves_all_contacts_and_exact_f64_balls() {
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
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mut filter = GpuContactFilter::new(device, queue);
    let mut rebuilt = 0;
    let mut reused = 0;
    let mut rejected = 0;
    let mut retries = 0;
    for (size, offset, scale) in [
        (8, DVec3::ZERO, 1.),
        (24, DVec3::splat(1e6), 1.),
        (4, DVec3::new(100., -200., 300.), 1e-3),
        (12, DVec3::ZERO, 1e3),
    ] {
        let mut rest = Vec::new();
        let mut triangles = Vec::new();
        for layer in 0..3 {
            for y in 0..size {
                for x in 0..size {
                    rest.push(
                        offset
                            + scale
                                * DVec3::new(
                                    x as f64 * 0.008,
                                    y as f64 * 0.008,
                                    if layer == 1 {
                                        2e-7
                                    } else {
                                        layer as f64 * 0.003
                                    },
                                ),
                    );
                }
            }
            for y in 0..size - 1 {
                for x in 0..size - 1 {
                    let a = layer * size * size + y * size + x;
                    triangles.extend([[a, a + 1, a + size], [a + 1, a + size + 1, a + size]]);
                }
            }
        }
        // Fixed primitive IDs occur first; exercise both VF directions and the
        // moving-first EE operand order even when IDs are not partitioned.
        triangles.reverse();
        let mut cpu = OgcWorld::new(&rest, triangles, 2 * size * size, [[0, size * size]]).unwrap();
        let mut gpu = cpu.clone();
        let settings = OgcSettings {
            radius: 0.003 * scale,
            ..Default::default()
        };
        for frame in 0..24 {
            let mut x = rest.clone();
            for (i, p) in x[..cpu.moving].iter_mut().enumerate() {
                let t = frame as f64 * 0.13 + i as f64 * 0.37;
                *p += scale * DVec3::new(t.sin() * 0.0004, t.cos() * 0.0002, t.sin() * 0.0001);
                if frame >= 18 {
                    p.x += scale * 0.007;
                }
            }
            let radius = scale
                * match frame {
                    0..=11 => 0.006,
                    12..=15 => 0.009,
                    _ => 0.004,
                };
            let a = cpu.detect(&x, radius, &settings).unwrap();
            let b = gpu
                .detect_with_filter(&x, radius, &settings, &mut filter)
                .unwrap();
            assert!(b.profile.gpu_filtered);
            assert!(b.profile.spatial_queries > 0);
            retries += b.profile.spatial_retries;
            equal(&a, &b);
            if frame == 16 && offset == DVec3::ZERO && scale == 1. {
                // Unsupported GPU numeric range falls back to a freshly refit
                // complete CPU query, then resident GPU state can be reused.
                let tiny = OgcSettings {
                    radius: 1e-13,
                    ..settings.clone()
                };
                let c = cpu.detect(&x, 2e-13, &tiny).unwrap();
                let d = gpu
                    .detect_with_filter(&x, 2e-13, &tiny, &mut filter)
                    .unwrap();
                assert!(!d.profile.gpu_filtered);
                equal(&c, &d);
            }
            rebuilt += usize::from(a.broad_phase_rebuilt);
            reused += usize::from(!a.broad_phase_rebuilt);
            rejected += a
                .profile
                .candidate_pairs
                .saturating_sub(b.profile.filter_pairs);
        }
    }
    assert!(rebuilt > 1 && reused > 0 && rejected > 0 && retries > 0);
}

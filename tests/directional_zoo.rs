//! Deterministic geometry laboratory, CPU-only. No private garment assets.
use glam::{DQuat, DVec3};
use marvelous_cloth::ogc::{CpuContactKernel, OgcSettings, OgcWorld, geometry};

fn random(seed: &mut u64) -> f64 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*seed >> 11) as f64 / ((1u64 << 53) as f64)
}

#[test]
fn seeded_convex_pair_zoo_preserves_separation_along_every_accepted_segment() {
    let settings = OgcSettings::default();
    let mut seed = std::env::var("CLOTHCRAFT_ZOO_SEED")
        .map_or(0x434c4f5448, |s| s.parse().expect("integer zoo seed"));
    let cases = std::env::var("CLOTHCRAFT_ZOO_CASES")
        .map_or(96, |s| s.parse::<usize>().expect("integer zoo case count"));
    assert!((1..=100_000).contains(&cases));
    eprintln!(
        "Directional geometry zoo: seed={seed}, cases={cases}, proposals_per_case=24, times_per_segment=5"
    );
    for case in 0..cases {
        let gap = 10_f64.powf(-7. + random(&mut seed) * 2.5);
        let angle = random(&mut seed) * 6.;
        let r = DQuat::from_axis_angle(DVec3::new(1., 2., 3.).normalize(), angle);
        let shift = if case % 3 == 0 {
            DVec3::new(30., -20., 10.)
        } else {
            DVec3::ZERO
        };
        let transform = |p: DVec3| r * p + shift;
        // Alternate point/face (both VF directions) and crossing edges with
        // non-intersecting supporting triangles. Include closest-edge features.
        let (x, triangles) = if case % 2 == 0 {
            (
                vec![
                    DVec3::new(0., 0., gap),
                    DVec3::new(-0.01, -0.01, 0.),
                    DVec3::new(0.01, -0.01, 0.),
                    DVec3::new(0., 0.01, 0.),
                ],
                vec![[1, 2, 3]],
            )
        } else {
            (
                vec![
                    DVec3::new(-0.01, 0., gap),
                    DVec3::new(0.01, 0., gap),
                    DVec3::new(-0.01, -0.01, gap),
                    DVec3::new(0., -0.01, 0.),
                    DVec3::new(0., 0.01, 0.),
                    DVec3::new(0.01, 0.01, 0.),
                ],
                vec![[0, 1, 2], [3, 4, 5]],
            )
        };
        let x: Vec<_> = x.into_iter().map(transform).collect();
        let mut world = OgcWorld::new(&x, triangles, x.len(), []).unwrap();
        world.directional_bounds = true;
        world.validate_start(&x).unwrap();
        let c = world.detect(&x, 0.006, &settings).unwrap();
        assert!(!c.directional.at(0).is_empty(), "case {case}");
        // The scalar and SIMD versions must construct the same planes, not
        // merely the same scalar minimum distance.
        let mut scalar = world.clone();
        scalar.cpu_kernel = CpuContactKernel::Scalar;
        let reference = scalar.detect(&x, 0.006, &settings).unwrap();
        assert_eq!(reference.directional, c.directional);
        assert_eq!(reference.radii, c.radii);
        let mut current = x.clone();
        for proposal in 0..24 {
            let proposed: Vec<_> = current
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let d = DVec3::new(
                        random(&mut seed) - 0.5,
                        random(&mut seed) - 0.5,
                        random(&mut seed) - 0.5,
                    ) * 0.03;
                    let q = if proposal % 2 == 0 {
                        c.truncate(i, *p + d).0
                    } else {
                        c.truncate_step(i, *p, *p + d).0
                    };
                    assert!(c.contains(i, q, 1e-12), "case {case}, vertex {i}");
                    q
                })
                .collect();
            // Independent geometric distances at interior times, in addition
            // to the half-space invariant. This is a sampled stress test;
            // the continuous guarantee comes from convex separation.
            for u in [0., 0.13, 0.37, 0.71, 1.] {
                let y: Vec<_> = current
                    .iter()
                    .zip(&proposed)
                    .map(|(a, b)| a.lerp(*b, u))
                    .collect();
                let distance = if case % 2 == 0 {
                    let (q, _) = geometry::closest_triangle(y[0], 0, &y, &world.topology);
                    y[0].distance(q)
                } else {
                    let (s, t) = geometry::closest_segments(y[0], y[1], y[3], y[4]);
                    y[0].lerp(y[1], s).distance(y[3].lerp(y[4], t))
                };
                assert!(
                    distance > gap * 0.09,
                    "case {case}, u {u}, separation {distance}"
                );
                world.validate_start(&y).unwrap();
            }
            current = proposed;
        }
    }
}

#[test]
fn fixed_avatar_and_reciprocal_vertex_face_keep_their_separating_sides() {
    let gap = 2e-6;
    let settings = OgcSettings::default();
    for moving in [1, 3] {
        let mut x = vec![
            DVec3::new(-0.01, -0.01, 0.),
            DVec3::new(0.01, -0.01, 0.),
            DVec3::new(0., 0.01, 0.),
            DVec3::new(0., 0., gap),
        ];
        let face = if moving == 1 {
            x.rotate_right(1);
            [1, 2, 3]
        } else {
            [0, 1, 2]
        };
        let mut w = OgcWorld::new(&x, vec![face], moving, []).unwrap();
        w.directional_bounds = true;
        let c = w.detect(&x, 0.006, &settings).unwrap();
        assert_eq!(c.directional.vertex_count(), moving);
        let mut y = x.clone();
        for v in 0..moving {
            let tangent = c.truncate(v, x[v] + DVec3::X * 0.001).0;
            assert!((tangent.x - x[v].x - 0.001).abs() < 1e-12);
            y[v] = c
                .truncate_step(
                    v,
                    tangent,
                    tangent + DVec3::Z * if moving == 1 { -0.01 } else { 0.01 },
                )
                .0;
            assert!(c.contains(v, y[v], 1e-12));
        }
        assert_eq!(&x[moving..], &y[moving..]);
        w.validate_start(&y).unwrap();
        let support = if moving == 1 {
            y[0].z - y[1..].iter().map(|p| p.z).fold(f64::NEG_INFINITY, f64::max)
        } else {
            y[3].z - y[..3].iter().map(|p| p.z).fold(f64::NEG_INFINITY, f64::max)
        };
        assert!(support > gap * 0.05);
    }
}

#[test]
fn flattened_patch_can_translate_tangentially_without_disabling_its_contacts() {
    let gap = 3e-6;
    let x = vec![
        DVec3::new(-0.01, 0., 0.),
        DVec3::new(0.01, 0., 0.),
        DVec3::new(0., gap, 0.),
        DVec3::new(0., -0.01, 0.),
    ];
    let mut w = OgcWorld::new(&x, vec![[0, 1, 2], [1, 0, 3]], 4, []).unwrap();
    let settings = OgcSettings::default();
    let old = w.detect(&x, 0.006, &settings).unwrap();
    w.directional_bounds = true;
    let new = w.detect(&x, 0.006, &settings).unwrap();
    assert_eq!(old.minimum_distance, new.minimum_distance);
    assert_eq!(old.active_contacts, new.active_contacts);
    let moved: Vec<_> = x
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let target = *p + DVec3::Z * 0.001;
            assert!(old.truncate(i, target).1);
            assert_eq!(new.truncate(i, target), (target, false));
            // An additional normal Newton proposal must not pull back a previously
            // accepted tangential displacement while clipping the forbidden part.
            let blocked = new
                .truncate_step(
                    i,
                    target,
                    target + DVec3::Y * if i == 2 { -0.01 } else { 0.01 },
                )
                .0;
            assert_eq!(blocked.z, target.z);
            assert!(new.contains(i, blocked, 1e-12));
            target
        })
        .collect();
    w.validate_start(&moved).unwrap();
}

#[test]
fn distant_pairs_still_have_a_query_envelope_and_uncertain_planes_fall_back() {
    let x = vec![
        DVec3::new(0., 0., 0.007),
        DVec3::new(-1., -1., 0.),
        DVec3::new(1., -1., 0.),
        DVec3::new(0., 1., 0.),
    ];
    let mut w = OgcWorld::new(&x, vec![[1, 2, 3]], 4, []).unwrap();
    w.directional_bounds = true;
    let c = w.detect(&x, 0.006, &OgcSettings::default()).unwrap();
    assert!(c.directional.at(0).is_empty());
    let moved: Vec<_> = x
        .iter()
        .enumerate()
        .map(|(i, p)| {
            c.truncate(i, *p + DVec3::Z * if i == 0 { -1. } else { 1. })
                .0
        })
        .collect();
    assert!(moved[0].z > moved[1].z);
    let offset = DVec3::splat(1e6);
    let mut close = x.clone();
    close[0].z = 1e-7;
    for p in &mut close {
        *p += offset;
    }
    let mut uncertain = OgcWorld::new(&close, vec![[1, 2, 3]], 4, []).unwrap();
    let old = uncertain
        .detect(&close, 0.006, &OgcSettings::default())
        .unwrap();
    uncertain.directional_bounds = true;
    let new = uncertain
        .detect(&close, 0.006, &OgcSettings::default())
        .unwrap();
    assert!(new.directional.at(0).is_empty());
    assert_eq!(old.radii, new.radii);
}

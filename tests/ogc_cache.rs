use glam::DVec3;
use marvelous_cloth::ogc::{OgcSettings, OgcWorld};

#[test]
fn temporal_candidates_match_fresh_detection_through_motion_and_radius_changes() {
    let mut x = vec![
        DVec3::new(-1., -1., 0.06),
        DVec3::new(1., -1., 0.06),
        DVec3::new(1., 1., 0.06),
        DVec3::new(-1., 1., 0.06),
        DVec3::new(-1.1, -1.1, 0.),
        DVec3::new(1.1, -1.1, 0.),
        DVec3::new(1.1, 1.1, 0.),
        DVec3::new(-1.1, 1.1, 0.),
    ];
    let original = OgcWorld::new(
        &x,
        vec![[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]],
        4,
        [[0, 1]],
    )
    .unwrap();
    let mut cached = original.clone();
    let s = OgcSettings {
        radius: 0.1,
        ..Default::default()
    };
    let mut reused = 0;
    let mut rebuilt = 0;
    for frame in 0..50 {
        for (i, p) in x[..4].iter_mut().enumerate() {
            p.x += if frame == 12 {
                0.3
            } else {
                0.0007 * (i + 1) as f64
            };
            p.z += 0.0002 * (i + 1) as f64;
        }
        let rq = if frame >= 25 { 0.21 } else { 0.16 };
        let a = cached.detect(&x, rq, &s).unwrap();
        let b = original.clone().detect(&x, rq, &s).unwrap();
        reused += usize::from(!a.broad_phase_rebuilt);
        rebuilt += usize::from(a.broad_phase_rebuilt);
        assert_eq!(a.radii, b.radii, "frame {frame}");
        assert_eq!(a.minimum_distance, b.minimum_distance);
        assert_eq!(a.vertex_face_candidates, b.vertex_face_candidates);
        assert_eq!(a.edge_edge_candidates, b.edge_edge_candidates);
        let key = |c: &marvelous_cloth::ogc::potential::Contact| {
            (c.kind, c.vertices, c.mollifier_epsilon)
        };
        assert_eq!(
            a.contacts.iter().map(key).collect::<Vec<_>>(),
            b.contacts.iter().map(key).collect::<Vec<_>>()
        );
        assert_eq!(a.centers, x[..4]);
    }
    assert!(reused > 30 && rebuilt >= 3);
    x[4].x += 0.01;
    assert!(
        cached.detect(&x, 0.21, &s).is_err(),
        "fixed geometry cannot silently invalidate its BVH"
    );
}

#[test]
fn cached_broad_phase_catches_a_pair_entering_the_query_radius() {
    let mut x = vec![
        DVec3::new(0., 0., 0.065),
        DVec3::new(-1., -1., 0.),
        DVec3::new(1., -1., 0.),
        DVec3::new(0., 1., 0.),
    ];
    let mut w = OgcWorld::new(&x, vec![[1, 2, 3]], 1, []).unwrap();
    let s = OgcSettings {
        radius: 0.02,
        ..Default::default()
    };
    let a = w.detect(&x, 0.05, &s).unwrap();
    assert_eq!(a.vertex_face_candidates, 0);
    x[0].z = 0.047;
    let b = w.detect(&x, 0.05, &s).unwrap();
    assert!(!b.broad_phase_rebuilt);
    assert_eq!(b.vertex_face_candidates, 1);
    assert!((b.radii[0] - 0.45 * 0.047).abs() < 1e-14);
    assert_eq!(b.contacts.len(), 1);
    assert_eq!(b.active_contacts, 0);
    assert_eq!(b.value(&x, &s), 0.);
}

use glam::DVec3;
use marvelous_cloth::ogc::bvh::{Bounds, Bvh};

#[test]
fn bounds_distance_never_discards_a_close_primitive_pair() {
    use marvelous_cloth::ogc::{
        geometry::{closest_segments, closest_triangle},
        topology::Topology,
    };
    let top = Topology::new(3, vec![[0, 1, 2]]).unwrap();
    let mut seed = 19u64;
    let mut random = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 32) as f64 / u32::MAX as f64 * 4. - 2.
    };
    for _ in 0..2000 {
        let p: Vec<_> = (0..4)
            .map(|_| DVec3::new(random(), random(), random()))
            .collect();
        let (q, _) = closest_triangle(p[3], 0, &p, &top);
        let vf = Bounds::points([p[3]]).distance_squared(Bounds::points(p[..3].iter().copied()));
        assert!(vf <= p[3].distance_squared(q) + 1e-12);
        let (s, t) = closest_segments(p[0], p[1], p[2], p[3]);
        let ee = Bounds::points([p[0], p[1]]).distance_squared(Bounds::points([p[2], p[3]]));
        assert!(ee <= p[0].lerp(p[1], s).distance_squared(p[2].lerp(p[3], t)) + 1e-12);
    }
    let a = Bounds::points([DVec3::ZERO, DVec3::ONE]);
    assert_eq!(a.distance_squared(a), 0.);
    assert_eq!(a.distance_squared(Bounds::points([DVec3::ONE])), 0.);
    assert_eq!(a.distance_squared(Bounds::points([DVec3::splat(2.)])), 3.);
}

#[test]
fn partitioned_bvh_matches_exhaustive_queries_after_large_deformations() {
    let mut seed = 7u64;
    let mut random = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 32) as f64 / u32::MAX as f64
    };
    let mut boxes: Vec<_> = (0..513)
        .map(|_| {
            let p = DVec3::new(random(), random(), random());
            Bounds::points([p, p + DVec3::splat(0.04)])
        })
        .collect();
    let dynamic: Vec<_> = (0..boxes.len()).map(|i| i % 3 == 0).collect();
    let mut bvh = Bvh::new(&boxes, &dynamic);
    for frame in 0..4 {
        if frame > 0 {
            for (i, b) in boxes.iter_mut().enumerate() {
                if dynamic[i] {
                    let delta = DVec3::new(random() - 0.5, random() - 0.5, random() - 0.5);
                    b.min += delta;
                    b.max += delta;
                }
            }
            bvh.refit_dynamic(|i| boxes[i]);
        }
        for _ in 0..50 {
            let p = DVec3::new(random(), random(), random());
            let q = Bounds::points([p]).inflate(0.15);
            for only_moving in [false, true] {
                let expected: Vec<_> = boxes
                    .iter()
                    .enumerate()
                    .filter_map(|(i, b)| {
                        (q.overlaps(*b) && (!only_moving || dynamic[i])).then_some(i)
                    })
                    .collect();
                let mut actual = Vec::new();
                bvh.query(q, only_moving, |i| actual.push(i));
                actual.sort_unstable();
                assert_eq!(actual, expected);
            }
        }
    }
    Bvh::new(&[], &[]).query(Bounds::points([DVec3::ZERO]), false, |_| panic!("empty"));
}

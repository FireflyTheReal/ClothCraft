use glam::DVec3;
use marvelous_cloth::ogc::bvh::{Bounds, Bvh};

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

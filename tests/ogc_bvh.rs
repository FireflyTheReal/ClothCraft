use glam::DVec3;
use marvelous_cloth::ogc::bvh::{Bounds, Bvh};

#[test]
fn dual_tree_pairs_match_exhaustive_search_without_duplicates() {
    use std::collections::BTreeSet;
    let mut seed = 53u64;
    let mut random = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 32) as f64 / u32::MAX as f64
    };
    let mut a: Vec<_> = (0..129)
        .map(|_| {
            let p = DVec3::new(random(), random(), random());
            Bounds::points([p, p + DVec3::splat(0.03)])
        })
        .collect();
    let b: Vec<_> = (0..97)
        .map(|_| Bounds::points([DVec3::new(random(), random(), random())]))
        .collect();
    // Fixed IDs interspersed with moving IDs, and point boxes of zero volume.
    let da: Vec<_> = (0..a.len()).map(|i| i % 3 == 1).collect();
    let db: Vec<_> = (0..b.len()).map(|i| i % 2 == 1).collect();
    let mut ta = Bvh::new(&a, &da);
    let tb = Bvh::new(&b, &db);
    for frame in 0..4 {
        if frame > 0 {
            for (i, box_) in a.iter_mut().enumerate() {
                if da[i] {
                    let delta = DVec3::new(random() - 0.5, random() - 0.5, random() - 0.5);
                    box_.min += delta;
                    box_.max += delta;
                }
            }
            ta.refit_dynamic(|i| a[i]);
        }
        for radius in [0., 0.05, 0.2, 2.] {
            let mut pairs = BTreeSet::new();
            ta.pairs_with(&tb, radius, |i, j| {
                assert!(pairs.insert((i, j)));
            });
            let mut expected = BTreeSet::new();
            for i in 0..a.len() {
                for j in 0..b.len() {
                    if (da[i] || db[j]) && a[i].distance_squared(b[j]) <= radius * radius {
                        expected.insert((i, j));
                    }
                }
            }
            assert_eq!(pairs, expected);
            pairs.clear();
            expected.clear();
            ta.self_pairs(radius, |i, j| {
                assert!(i < j && pairs.insert((i, j)));
            });
            for i in 0..a.len() {
                for j in i + 1..a.len() {
                    if (da[i] || da[j]) && a[i].distance_squared(a[j]) <= radius * radius {
                        expected.insert((i, j));
                    }
                }
            }
            assert_eq!(pairs, expected);
        }
    }
    Bvh::new(&[], &[]).pairs_with(&tb, 1., |_, _| panic!("empty"));
}

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

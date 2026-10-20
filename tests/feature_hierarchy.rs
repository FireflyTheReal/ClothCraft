use glam::DVec3;
use marvelous_cloth::ogc::{
    OgcSettings, OgcWorld,
    feature_hierarchy::{Hierarchy, Pair},
    geometry::{closest_segments, closest_triangle},
    topology::Topology,
};
use std::collections::BTreeSet;

fn distance(pair: Pair, top: &Topology, x: &[DVec3]) -> f64 {
    match pair {
        Pair::VertexFace([v, f]) => x[v].distance(closest_triangle(x[v], f, x, top).0),
        Pair::EdgeEdge([e, f]) => {
            let [a, b] = top.edges[e].map(|i| x[i]);
            let [c, d] = top.edges[f].map(|i| x[i]);
            let (s, t) = closest_segments(a, b, c, d);
            a.lerp(b, s).distance(c.lerp(d, t))
        }
        Pair::VertexEdge([v, e]) => {
            let [a, b] = top.edges[e].map(|i| x[i]);
            let t = ((x[v] - a).dot(b - a) / a.distance_squared(b)).clamp(0., 1.);
            x[v].distance(a.lerp(b, t))
        }
        Pair::VertexVertex([a, b]) => x[a].distance(x[b]),
    }
}
fn oracle(
    top: &Topology,
    x: &[DVec3],
    moving: usize,
    excluded: &BTreeSet<[usize; 2]>,
    radius: f64,
) -> BTreeSet<Pair> {
    let mut set = BTreeSet::new();
    let mut add = |pair: Pair, a: &[usize], b: &[usize]| {
        if !a.iter().chain(b).any(|i| *i < moving)
            || a.iter().any(|i| {
                b.iter()
                    .any(|j| i == j || excluded.contains(&[(*i).min(*j), (*i).max(*j)]))
            })
        {
            return;
        }
        if distance(pair, top, x) <= radius {
            set.insert(pair);
        }
    };
    for v in 0..x.len() {
        for (f, t) in top.triangles.iter().enumerate() {
            add(Pair::VertexFace([v, f]), &[v], t);
        }
        for (e, ids) in top.edges.iter().enumerate() {
            add(Pair::VertexEdge([v, e]), &[v], ids);
        }
        for u in v + 1..x.len() {
            add(Pair::VertexVertex([v, u]), &[v], &[u]);
        }
    }
    for e in 0..top.edges.len() {
        for f in e + 1..top.edges.len() {
            add(Pair::EdgeEdge([e, f]), &top.edges[e], &top.edges[f]);
        }
    }
    set
}

#[test]
fn four_family_bvh_queries_match_independent_exhaustive_distance_oracle() {
    let mut seed = 12345u64;
    let mut random = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 32) as f64 / u32::MAX as f64
    };
    let mut x = Vec::new();
    let mut faces = Vec::new();
    for layer in 0..8 {
        let i = x.len();
        faces.push([i, i + 1, i + 2]);
        for _ in 0..3 {
            x.push(DVec3::new(
                random() * 0.02,
                layer as f64 * 0.0007,
                random() * 0.02,
            ));
        }
    }
    let top = Topology::new(x.len(), faces).unwrap();
    let excluded = BTreeSet::from([[0, 7], [2, 10], [5, 23]]);
    for moving in [0, 1, 7, 24] {
        for radius in [0.00013, 0.0013, 0.0027, 0.0061] {
            let h = Hierarchy::build(&top, &x, moving, &excluded, radius).unwrap();
            assert_eq!(h.duplicate_visits, 0);
            assert_eq!(
                h.pairs.iter().copied().collect::<BTreeSet<_>>(),
                oracle(&top, &x, moving, &excluded, radius)
            );
        }
    }
}

#[test]
fn lower_features_survive_an_unrelated_parent_seam_exclusion() {
    let x = vec![
        DVec3::new(0.005, 0.001, 0.),
        DVec3::ZERO,
        DVec3::X * 0.02,
        DVec3::Z * 0.02,
    ];
    let top = Topology::new(4, vec![[1, 2, 3]]).unwrap();
    let h = Hierarchy::build(&top, &x, 1, &BTreeSet::from([[0, 3]]), 0.003).unwrap();
    let edge = top.edges.iter().position(|e| *e == [1, 2]).unwrap();
    assert!(!h.pairs.contains(&Pair::VertexFace([0, 0])));
    assert!(h.pairs.contains(&Pair::VertexEdge([0, edge])));
}

#[test]
fn vertex_face_boundary_and_parallel_edge_handoffs_remain_covered() {
    for p in [
        DVec3::new(0.003, 0.001, 0.003),
        DVec3::new(0.005, 0.001, 0.),
        DVec3::new(-0.0002, 0.001, -0.0002),
    ] {
        let x = vec![p, DVec3::ZERO, DVec3::X * 0.02, DVec3::Z * 0.02];
        let top = Topology::new(4, vec![[1, 2, 3]]).unwrap();
        let h = Hierarchy::build(&top, &x, 1, &BTreeSet::new(), 0.006).unwrap();
        let c = h.coverage(&top, &x, 0.003, 0.01).unwrap();
        assert!(
            c.factor_errors.is_empty()
                && c.missing_active_lower_pairs.is_empty()
                && c.uncovered_zero_weight_pairs.is_empty()
        );
        assert_eq!(c.active_by_kind[0], 1);
    }
    let x = vec![
        DVec3::new(-0.01, 0.001, 0.),
        DVec3::new(0.01, 0.001, 0.),
        DVec3::new(0., 0.03, 0.02),
        DVec3::new(-0.02, 0., 0.),
        DVec3::new(0.02, 0., 0.),
        DVec3::new(0., -0.03, -0.02),
    ];
    let top = Topology::new(6, vec![[0, 1, 2], [3, 4, 5]]).unwrap();
    let h = Hierarchy::build(&top, &x, 3, &BTreeSet::new(), 0.006).unwrap();
    let c = h.coverage(&top, &x, 0.003, 0.01).unwrap();
    assert!(c.zero_weight_by_kind[1] > 0);
    assert!(
        c.factor_errors.is_empty()
            && c.missing_active_lower_pairs.is_empty()
            && c.uncovered_zero_weight_pairs.is_empty()
    );
    let mut changed = x.clone();
    changed[0].x += 1e-9;
    assert!(h.coverage(&top, &changed, 0.003, 0.01).is_err());
    assert!(h.coverage(&top, &x, 0.007, 0.01).is_err());
}

#[test]
fn snapshot_hierarchy_does_not_change_production_contacts_or_bounds() {
    let x = vec![
        DVec3::new(0.005, 0.001, 0.),
        DVec3::ZERO,
        DVec3::X * 0.02,
        DVec3::Z * 0.02,
    ];
    let mut world = OgcWorld::new(&x, vec![[1, 2, 3]], 1, []).unwrap();
    world.directional_bounds = true;
    let cfg = OgcSettings::default();
    let before = world.detect(&x, 0.006, &cfg).unwrap();
    let h = world.feature_hierarchy(&x, 0.006).unwrap();
    assert!(!h.pairs.is_empty());
    let after = world.detect(&x, 0.006, &cfg).unwrap();
    assert_eq!(before.radii, after.radii);
    assert_eq!(before.centers, after.centers);
    assert_eq!(
        format!("{:?}", before.contacts),
        format!("{:?}", after.contacts)
    );
    assert_eq!(
        format!("{:?}", before.directional),
        format!("{:?}", after.directional)
    );
}

#[test]
fn fixed_boundary_handoff_is_not_a_missing_dynamic_pair() {
    let x = vec![
        DVec3::Z * 0.02,
        DVec3::ZERO,
        DVec3::X * 0.02,
        DVec3::new(0.005, 0.001, 0.),
    ];
    let top = Topology::new(4, vec![[0, 1, 2]]).unwrap();
    let h = Hierarchy::build(&top, &x, 1, &BTreeSet::new(), 0.006).unwrap();
    let c = h.coverage(&top, &x, 0.003, 0.01).unwrap();
    assert!(c.static_lower_witnesses > 0);
    assert!(c.missing_active_lower_pairs.is_empty());
    assert!(c.uncovered_zero_weight_pairs.is_empty());
    assert!(c.factor_errors.is_empty());
}

#[test]
fn degenerate_and_coincident_inputs_are_not_reported_as_valid_coverage() {
    let mut x = vec![DVec3::ZERO, DVec3::ZERO, DVec3::X, DVec3::Z];
    let top = Topology::new(4, vec![[1, 2, 3]]).unwrap();
    let h = Hierarchy::build(&top, &x, 1, &BTreeSet::new(), 0.01).unwrap();
    let c = h.coverage(&top, &x, 0.003, 0.01).unwrap();
    assert!(!c.factor_errors.is_empty());
    assert!(Hierarchy::build(&top, &x, 1, &BTreeSet::new(), f64::NAN).is_err());
    x[3] = DVec3::X * 2.;
    assert!(Hierarchy::build(&top, &x, 1, &BTreeSet::new(), 0.01).is_err());
}

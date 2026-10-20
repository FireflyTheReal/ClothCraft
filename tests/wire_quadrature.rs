use glam::DVec3;
use marvelous_cloth::ogc::{
    OgcSettings, OgcWorld,
    geometry::closest_segments,
    potential::{Contact, Kind, activation},
};

fn fixture() -> (Vec<DVec3>, Vec<[usize; 3]>) {
    (
        vec![
            DVec3::new(-0.01, 0.001, 0.),
            DVec3::new(0.01, 0.001, 0.0001),
            DVec3::new(0., 0.02, 0.03),
            DVec3::new(-0.00001, 0., 0.),
            DVec3::new(0.00001, 0., 0.),
            DVec3::new(0., 0.00001, 0.),
            DVec3::new(0., 0., 0.00001),
        ],
        vec![[0, 1, 2], [3, 4, 5], [3, 6, 4], [3, 5, 6], [4, 6, 5]],
    )
}

#[test]
fn dense_wire_cycle_is_a_negative_density_counterexample() {
    let (x, triangles) = fixture();
    let w = OgcWorld::new(&x, triangles.clone(), 3, []).unwrap();
    let report = w
        .audit_wire_quadrature(&x, &OgcSettings::default())
        .unwrap();
    let row = report
        .worst_rows
        .iter()
        .find(|r| r.source_vertices == [0, 1])
        .unwrap();
    assert_eq!(row.active_pairs, 6);
    assert_eq!(row.components, 1);
    assert_eq!(row.max_vertices_in_component, 4);
    assert!(row.signed_density_j < -0.35);
    assert!(row.cycle_corrections_skipped > 0);
    assert!(row.forest_density_j + 1e-12 >= row.maximum_primitive_j);
    assert_eq!(report.forest_below_maximum_primitive, 0);
    assert!(row.minimum_distance_m > 0.0009);
    assert!(report.negative_source_edges > 0);
    assert_eq!(report.oracle_mismatch_sources, 0);
    assert_eq!(report.oracle_max_pair_count_error, 0);
    let cfg = OgcSettings {
        radius: 0.0002,
        ..Default::default()
    };
    let small = w.audit_wire_quadrature(&x, &cfg).unwrap();
    assert_eq!(small.active_source_edges, 0);
    // Shrinking until the condition holds can simply remove the force shell.
    let shifted = |dy: f64| {
        let mut y = x.clone();
        y[0].y += dy;
        y[1].y += dy;
        let world = OgcWorld::new(&y, triangles.clone(), 3, []).unwrap();
        let r = world
            .audit_wire_quadrature(&y, &OgcSettings::default())
            .unwrap();
        let row = r
            .worst_rows
            .iter()
            .find(|r| r.source_vertices == [0, 1])
            .unwrap();
        (row.signed_density_j, row.forest_density_j)
    };
    let eps = 1e-8;
    let above = shifted(eps);
    let below = shifted(-eps);
    let raw_force = -(above.0 - below.0) / (2. * eps);
    let forest_force = -(above.1 - below.1) / (2. * eps);
    eprintln!("dense-cycle force Y: signed={raw_force:.9} N; forest={forest_force:.9} N");
    assert!(raw_force < -300.); // Attraction toward the target, not just an energy offset.
    assert!(forest_force > 100.);
}

#[test]
fn local_two_edge_contact_has_positive_density_and_one_active_vertex() {
    let x = vec![
        DVec3::new(-0.002, 0.001, 0.),
        DVec3::new(0.002, 0.001, 0.),
        DVec3::new(0., 0.02, -0.02),
        DVec3::new(-0.02, 0., 0.02),
        DVec3::ZERO,
        DVec3::new(0.02, 0., 0.02),
    ];
    let w = OgcWorld::new(&x, vec![[0, 1, 2], [3, 4, 5]], 3, []).unwrap();
    let audit = w
        .audit_wire_quadrature(&x, &OgcSettings::default())
        .unwrap();
    let row = audit
        .worst_rows
        .iter()
        .find(|r| r.source_vertices == [0, 1])
        .unwrap();
    assert_eq!(row.active_pairs, 2);
    assert_eq!(row.max_vertices_in_component, 1);
    assert_eq!(row.components_with_multiple_vertices, 0);
    assert!(row.signed_density_j + 1e-12 >= row.maximum_primitive_j);
    assert_eq!(row.cycle_corrections_skipped, 0);
    assert!((row.forest_density_j - row.signed_density_j).abs() < 1e-12);
}

#[test]
fn forest_cycle_correction_preserves_barrier_but_can_introduce_a_force_cusp() {
    let sample = |shift: f64| {
        let x = vec![
            DVec3::new(shift, 0.001, -0.00025),
            DVec3::new(shift, 0.001, -0.00015),
            DVec3::new(shift, 0.02, 0.03),
            DVec3::new(-0.001, 0., 0.),
            DVec3::new(0.001, 0., 0.),
            DVec3::new(0., 0., 0.0002),
        ];
        let world = OgcWorld::new(&x, vec![[0, 1, 2], [3, 4, 5]], 3, []).unwrap();
        let report = world
            .audit_wire_quadrature(&x, &OgcSettings::default())
            .unwrap();
        let row = report
            .worst_rows
            .iter()
            .find(|r| r.source_vertices == [0, 1])
            .unwrap();
        assert_eq!(row.active_pairs, 3);
        assert_eq!(row.cycle_corrections_skipped, 1);
        assert!(row.forest_density_j >= row.maximum_primitive_j - 1e-12);
        (row.forest_density_j, row.signed_density_j)
    };
    let middle = sample(0.);
    for eps in [1e-7, 1e-8, 1e-9] {
        let left = sample(-eps);
        let right = sample(eps);
        let left_slope = (middle.0 - left.0) / eps;
        let right_slope = (right.0 - middle.0) / eps;
        let original_jump = ((middle.1 - left.1) - (right.1 - middle.1)) / eps;
        eprintln!(
            "epsilon={eps:e} m; forest slopes={left_slope:.9},{right_slope:.9} N; original slope difference={original_jump:.9} N"
        );
        // A negative result for promotion: energy continuity/positivity alone
        // does not certify differentiable, jitter-free forces.
        assert!(left_slope - right_slope > 200.);
        assert!(original_jump.abs() < 0.2);
    }
}

#[test]
fn independent_global_degree_oracle_matches_with_seam_exclusions() {
    let (x, triangles) = fixture();
    for excluded in [vec![], vec![[0, 3]], vec![[0, 3], [1, 4]]] {
        let w = OgcWorld::new(&x, triangles.clone(), 3, excluded.clone()).unwrap();
        let cfg = OgcSettings::default();
        let report = w.audit_wire_quadrature(&x, &cfg).unwrap();
        assert_eq!(report.oracle_mismatch_sources, 0);
        for row in &report.worst_rows {
            let [a, b] = row.source_vertices;
            let mut degree = vec![0usize; x.len()];
            let mut sum = 0.;
            for &[c, d] in &w.topology.edges {
                if a >= 3 && b >= 3 && c >= 3 && d >= 3 {
                    continue;
                }
                if [a, b].iter().any(|i| {
                    [c, d].iter().any(|j| {
                        i == j || excluded.contains(&[*i, *j]) || excluded.contains(&[*j, *i])
                    })
                }) {
                    continue;
                }
                degree[c] += 1;
                degree[d] += 1;
                let (s, t) = closest_segments(x[a], x[b], x[c], x[d]);
                let distance = x[a].lerp(x[b], s).distance(x[c].lerp(x[d], t));
                sum += activation(distance, cfg.radius, cfg.stiffness);
            }
            for (i, degree) in degree.into_iter().enumerate() {
                if degree < 2 {
                    continue;
                }
                let t =
                    ((x[i] - x[a]).dot(x[b] - x[a]) / x[a].distance_squared(x[b])).clamp(0., 1.);
                let distance = x[i].distance(x[a].lerp(x[b], t));
                sum -= (degree - 1) as f64 * activation(distance, cfg.radius, cfg.stiffness);
            }
            assert!((sum - row.signed_density_j).abs() < 1e-9);
        }
    }
}

#[test]
fn mollifying_only_positive_edge_terms_breaks_even_a_two_edge_patch() {
    let mut x = [
        DVec3::new(-0.01, 0.001, 0.),
        DVec3::new(0.01, 0.001, 0.),
        DVec3::new(-0.02, 0., 0.),
        DVec3::ZERO,
        DVec3::new(0.02, 0., 0.),
    ];
    let cfg = OgcSettings::default();
    let subtraction = activation(0.001, cfg.radius, cfg.stiffness);
    for degrees in [0_f64, 0.1, 0.25, 0.5, 0.75, 1.5] {
        let theta = degrees.to_radians();
        let tangent = DVec3::new(theta.cos(), 0., theta.sin()) * 0.01;
        x[0] = DVec3::Y * 0.001 - tangent;
        x[1] = DVec3::Y * 0.001 + tangent;
        let positive: f64 = [[0, 1, 2, 3], [0, 1, 3, 4]]
            .into_iter()
            .map(|vertices| {
                Contact {
                    kind: Kind::SegmentSegment,
                    vertices,
                    count: 4,
                    mollifier_epsilon: 1.6e-10,
                }
                .value(&x, cfg.radius, cfg.stiffness)
            })
            .sum();
        if degrees == 0. {
            assert_eq!(positive, 0.);
        }
        if degrees < 1. {
            assert!(positive - subtraction < 0., "{degrees}");
        } else {
            assert!(positive - subtraction > 0.);
        }
    }
}

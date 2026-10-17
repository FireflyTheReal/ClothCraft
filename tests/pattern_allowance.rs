//! Seam-allowance cutting contours: joined corners and self-intersection
//! cleanup around the unchanged sewing contour. CPU-only.
use glam::DVec2;
use marvelous_cloth::{
    authoring::{self, Command},
    garment::{
        GarmentTemplate,
        domain::area,
        meshing::MeshingSettings,
        polygon::PolygonPattern,
        presets::{self, GarmentKind},
        shapes::Primitive,
    },
};

fn settings() -> MeshingSettings {
    MeshingSettings {
        target_edge_m: 0.05,
        ..Default::default()
    }
}

fn rectangle(w: f64, h: f64) -> GarmentTemplate {
    authoring::apply(
        &presets::basic(GarmentKind::Tshirt, 0),
        Command::Primitive {
            kind: Primitive::Rectangle,
            size: DVec2::new(w, h),
            sweep_degrees: 180.,
            origin: DVec2::ZERO,
            meshing: settings(),
            fabric: "woven-reference".into(),
            replace: true,
        },
    )
    .unwrap()
    .template
}

fn polygon(t: &GarmentTemplate) -> &PolygonPattern {
    t.panels[0].polygon.as_ref().unwrap()
}

fn allowance(mut t: GarmentTemplate, edges: &[&str], width: f64) -> GarmentTemplate {
    for edge in edges {
        t = authoring::apply(
            &t,
            Command::SeamAllowance {
                panel: "panel_1".into(),
                edge: (*edge).into(),
                width_m: width,
            },
        )
        .unwrap()
        .template;
    }
    t
}

fn outline(t: &GarmentTemplate) -> Vec<Vec<DVec2>> {
    polygon(t).cutting_outline().unwrap().unwrap()
}

/// True when the closed ring has no self-intersection (adjacent segments share
/// an endpoint and are ignored).
fn is_simple(ring: &[DVec2]) -> bool {
    let n = ring.len();
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        for j in i + 1..n {
            if j == i || (i == 0 && j == n - 1) || j == i + 1 {
                continue;
            }
            let (c, d) = (ring[j], ring[(j + 1) % n]);
            let cross = |p: DVec2, q: DVec2, r: DVec2| (q - p).perp_dot(r - p);
            let eps = 1e-12;
            if a.min(b).x > c.max(d).x + eps
                || c.min(d).x > a.max(b).x + eps
                || a.min(b).y > c.max(d).y + eps
                || c.min(d).y > a.max(b).y + eps
            {
                continue;
            }
            if cross(a, b, c) * cross(a, b, d) <= eps * eps
                && cross(c, d, a) * cross(c, d, b) <= eps * eps
            {
                return false;
            }
        }
    }
    true
}

#[test]
fn no_allowance_means_no_cutting_contour() {
    assert!(
        polygon(&rectangle(0.4, 0.6))
            .cutting_outline()
            .unwrap()
            .is_none()
    );
}

#[test]
fn one_marked_edge_extends_only_that_edge() {
    let t = allowance(rectangle(0.4, 0.6), &["edge_1"], 0.01);
    let rings = outline(&t);
    assert_eq!(rings.len(), 1);
    assert!(area(&rings[0]) > 0., "outer ring is counter-clockwise");
    // 400x600 mm plus a 10 mm skirt below edge_1, with square corner ends.
    assert!((area(&rings[0]) - (0.24 + 0.42 * 0.01)).abs() < 1e-5);
    let lo = rings[0]
        .iter()
        .copied()
        .fold(DVec2::splat(f64::INFINITY), DVec2::min);
    let hi = rings[0]
        .iter()
        .copied()
        .fold(DVec2::splat(f64::NEG_INFINITY), DVec2::max);
    assert!((lo - DVec2::new(-0.01, -0.01)).length() < 1e-5, "{lo:?}");
    assert!((hi - DVec2::new(0.41, 0.6)).length() < 1e-5, "{hi:?}");
    assert!(is_simple(&rings[0]));
}

#[test]
fn all_edges_grow_the_outline_with_joined_corners() {
    let t = allowance(
        rectangle(0.4, 0.6),
        &["edge_1", "edge_2", "edge_3", "edge_4"],
        0.01,
    );
    let rings = outline(&t);
    assert_eq!(rings.len(), 1);
    // Rectangle area + perimeter * width + four square corners.
    let expected = 0.24 + 2.0 * 0.01 + 4.0 * 0.01 * 0.01;
    assert!(
        (area(&rings[0]) - expected).abs() < 1e-6,
        "{}",
        area(&rings[0])
    );
    let lo = rings[0]
        .iter()
        .copied()
        .fold(DVec2::splat(f64::INFINITY), DVec2::min);
    let hi = rings[0]
        .iter()
        .copied()
        .fold(DVec2::splat(f64::NEG_INFINITY), DVec2::max);
    assert!((lo - DVec2::splat(-0.01)).length() < 1e-6, "{lo:?}");
    assert!((hi - DVec2::new(0.41, 0.61)).length() < 1e-6, "{hi:?}");
    assert!(is_simple(&rings[0]));
}

#[test]
fn a_reflex_corner_stays_a_simple_joined_outline() {
    let t = authoring::apply(
        &presets::basic(GarmentKind::Tshirt, 0),
        Command::Polygon {
            points: vec![
                DVec2::new(0., 0.),
                DVec2::new(0.3, 0.),
                DVec2::new(0.3, 0.1),
                DVec2::new(0.1, 0.1),
                DVec2::new(0.1, 0.3),
                DVec2::new(0., 0.3),
            ],
            tangents: vec![DVec2::ZERO; 6],
            meshing: settings(),
            fabric: "woven-reference".into(),
            replace: true,
        },
    )
    .unwrap()
    .template;
    let t = allowance(
        t,
        &["edge_1", "edge_2", "edge_3", "edge_4", "edge_5", "edge_6"],
        0.01,
    );
    let rings = outline(&t);
    assert_eq!(rings.len(), 1, "an L-shape offset stays one ring");
    let sewing = area(&polygon(&t).outer_points().unwrap());
    assert!(area(&rings[0]) > sewing + 1.2 * 0.01 - 1e-6);
    assert!(
        is_simple(&rings[0]),
        "the reflex corner must not self-intersect"
    );
}

#[test]
fn a_curved_sewing_contour_is_offset_smoothly() {
    let t = authoring::apply(
        &presets::basic(GarmentKind::Tshirt, 0),
        Command::Primitive {
            kind: Primitive::Ellipse,
            size: DVec2::new(0.3, 0.2),
            sweep_degrees: 360.,
            origin: DVec2::ZERO,
            meshing: settings(),
            fabric: "woven-reference".into(),
            replace: true,
        },
    )
    .unwrap()
    .template;
    let t = allowance(t, &["edge_1", "edge_2", "edge_3", "edge_4"], 0.01);
    let rings = outline(&t);
    assert_eq!(rings.len(), 1);
    assert!(rings[0].len() > 32, "the curve is sampled, not a 4-gon");
    assert!(is_simple(&rings[0]));
    // Area grows by roughly perimeter * width.
    let sewing = area(&polygon(&t).outer_points().unwrap());
    assert!(area(&rings[0]) > sewing, "{} vs {sewing}", area(&rings[0]));
}

#[test]
fn holes_are_copied_as_clockwise_rings() {
    let mut t = rectangle(0.4, 0.4);
    let hole = marvelous_cloth::garment::domain::Hole {
        points: vec![
            DVec2::new(0.15, 0.15),
            DVec2::new(0.15, 0.25),
            DVec2::new(0.25, 0.25),
            DVec2::new(0.25, 0.15),
        ],
        edges: (0..4).map(|i| format!("hole_1_edge_{}", i + 1)).collect(),
    };
    t.panels[0].polygon.as_mut().unwrap().holes.push(hole);
    let t = allowance(t, &["edge_1", "edge_2", "edge_3", "edge_4"], 0.005);
    let rings = outline(&t);
    assert_eq!(rings.len(), 2, "outer ring plus the untouched hole rim");
    assert!(rings.iter().any(|r| area(r) > 0.));
    let hole_ring = rings.iter().find(|r| area(r) < 0.).unwrap();
    assert!((area(hole_ring) + 0.01).abs() < 1e-6, "{}", area(hole_ring));
}

#[test]
fn an_invalid_stored_allowance_is_rejected() {
    let mut t = rectangle(0.4, 0.6);
    t.panels[0]
        .polygon
        .as_mut()
        .unwrap()
        .seam_allowances
        .insert("edge_1".into(), 0.2);
    assert!(polygon(&t).cutting_outline().is_err());
}

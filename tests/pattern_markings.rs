//! Draft pattern markings: notches and seam allowances as stable material data.
//! CPU-only; they must never move rest geometry or the simulated drape.
use glam::DVec2;
use marvelous_cloth::{
    authoring::{self, Command},
    garment::{
        GarmentTemplate,
        meshing::MeshingSettings,
        polygon::PolygonPattern,
        presets::{self, GarmentKind},
        shapes::{BooleanOp, MirrorAxis, Primitive},
    },
};

fn settings() -> MeshingSettings {
    MeshingSettings {
        target_edge_m: 0.05,
        ..Default::default()
    }
}

/// A bare 400 x 600 mm rectangle, replacing the preset shirt.
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

/// Apply a command and keep its edited template.
fn step(t: &GarmentTemplate, command: Command) -> GarmentTemplate {
    authoring::apply(t, command).unwrap().template
}

fn polygon(t: &GarmentTemplate) -> &PolygonPattern {
    t.panels[0].polygon.as_ref().unwrap()
}

fn area(t: &GarmentTemplate) -> f64 {
    t.compile()
        .unwrap()
        .mesh
        .triangles
        .iter()
        .map(|t| t.area)
        .sum()
}

fn notch(edge: &str) -> Command {
    Command::Notch {
        panel: "panel_1".into(),
        edge: edge.into(),
        t: 0.25,
        depth_m: 0.01,
    }
}

#[test]
fn markings_are_draft_only_and_keep_the_drape() {
    let base = rectangle(0.4, 0.6);
    let with_notch = authoring::apply(&base, notch("edge_1")).unwrap();
    assert!(
        !with_notch.reset_simulation,
        "adding a notch must not invalidate the drape"
    );
    let with_allowance = authoring::apply(
        &with_notch.template,
        Command::SeamAllowance {
            panel: "panel_1".into(),
            edge: "edge_1".into(),
            width_m: 0.01,
        },
    )
    .unwrap();
    assert!(
        !with_allowance.reset_simulation,
        "storing an allowance must not invalidate the drape"
    );
    // The sewing contour and its area are untouched.
    assert!((area(&base) - area(&with_allowance.template)).abs() < 1e-12);
    assert_eq!(
        polygon(&with_allowance.template).points,
        polygon(&base).points
    );
}

#[test]
fn notch_lands_on_the_boundary_and_extends_outward() {
    let t = step(&rectangle(0.4, 0.6), notch("edge_1"));
    let [base, tip] = polygon(&t).notch_segments("edge_1").unwrap()[0];
    // 0.25 of a 400 mm edge is 100 mm from the edge start.
    assert!((base - DVec2::new(0.1, 0.)).length() < 1e-12);
    // The outer contour is CCW, so the marker points to v < 0.
    assert!((tip - DVec2::new(0.1, -0.01)).length() < 1e-12);
}

#[test]
fn markings_survive_a_json_round_trip() {
    let marked = step(&rectangle(0.4, 0.6), notch("edge_1"));
    let t = step(
        &marked,
        Command::SeamAllowance {
            panel: "panel_1".into(),
            edge: "edge_1".into(),
            width_m: 0.008,
        },
    );
    let text = serde_json::to_string(&t).unwrap();
    let read: GarmentTemplate = serde_json::from_str(&text).unwrap();
    read.compile().unwrap();
    assert_eq!(polygon(&read).notches, polygon(&t).notches);
    assert_eq!(polygon(&read).seam_allowances["edge_1"], 0.008);
}

#[test]
fn empty_markings_are_omitted_from_serialization() {
    let t = rectangle(0.4, 0.6);
    let text = serde_json::to_string(&t).unwrap();
    assert!(!text.contains("notches"));
    assert!(!text.contains("seam_allowances"));
}

#[test]
fn splitting_a_marked_boundary_rebases_the_markings() {
    let marked = step(
        &step(&rectangle(0.4, 0.6), notch("edge_1")),
        Command::Notch {
            panel: "panel_1".into(),
            edge: "edge_1".into(),
            t: 0.75,
            depth_m: 0.008,
        },
    );
    let marked = step(
        &marked,
        Command::SeamAllowance {
            panel: "panel_1".into(),
            edge: "edge_1".into(),
            width_m: 0.01,
        },
    );
    // Split the 400 mm edge in half: a notch at 0.25 and 0.75 both land at 0.5
    // of their half, and each half inherits the allowance.
    let split = step(
        &marked,
        Command::SplitBoundary {
            panel: "panel_1".into(),
            edge: "edge_1".into(),
            at_m: 0.2,
        },
    );
    let poly = polygon(&split);
    assert_eq!(poly.notches["edge_1"][0].t, 0.5);
    assert_eq!(poly.seam_allowances["edge_1"], 0.01);
    assert_eq!(poly.notches["edge_1_2"][0].t, 0.5);
    assert_eq!(poly.seam_allowances["edge_1_2"], 0.01);
    // Material positions are unchanged: the markers are still at 100 / 300 mm.
    assert!((poly.notch_segments("edge_1").unwrap()[0][0] - DVec2::new(0.1, 0.)).length() < 1e-12);
    assert!(
        (poly.notch_segments("edge_1_2").unwrap()[0][0] - DVec2::new(0.3, 0.)).length() < 1e-12
    );
}

#[test]
fn removing_a_point_repoints_markings_proportionally() {
    // Split once, mark both halves, then remove the inserted point.
    let split = step(
        &rectangle(0.4, 0.6),
        Command::SplitBoundary {
            panel: "panel_1".into(),
            edge: "edge_1".into(),
            at_m: 0.2,
        },
    );
    let marked = step(
        &split,
        Command::Notch {
            panel: "panel_1".into(),
            edge: "edge_1".into(),
            t: 0.5,
            depth_m: 0.01,
        },
    );
    let marked = step(
        &marked,
        Command::Notch {
            panel: "panel_1".into(),
            edge: "edge_1_2".into(),
            t: 0.5,
            depth_m: 0.01,
        },
    );
    let marked = step(
        &marked,
        Command::SeamAllowance {
            panel: "panel_1".into(),
            edge: "edge_1".into(),
            width_m: 0.01,
        },
    );
    let joined = step(
        &marked,
        Command::RemovePoint {
            panel: "panel_1".into(),
            point: 1,
        },
    );
    let poly = polygon(&joined);
    // Collinear halves of equal length: 0.25/0.75 of the rejoined 400 mm edge.
    assert!((poly.notches["edge_1"][0].t - 0.25).abs() < 1e-12);
    assert!((poly.notches["edge_1"][1].t - 0.75).abs() < 1e-12);
    assert_eq!(poly.seam_allowances["edge_1"], 0.01);
}

#[test]
fn mirroring_inverts_the_notch_fraction() {
    let marked = step(&rectangle(0.4, 0.6), notch("edge_1"));
    let mirrored = step(
        &marked,
        Command::Mirror {
            panel: "panel_1".into(),
            axis: MirrorAxis::X,
            copy_side: None,
        },
    );
    let poly = polygon(&mirrored);
    // Reversing the contour flips the directed boundary, so 0.25 becomes 0.75.
    assert_eq!(poly.notches["edge_1"][0].t, 0.75);
    assert!((poly.notch_segments("edge_1").unwrap()[0][0] - DVec2::new(0.3, 0.)).length() < 1e-12);
}

#[test]
fn invalid_markings_are_rejected_atomically() {
    let base = rectangle(0.4, 0.6);
    for command in [
        Command::Notch {
            panel: "panel_1".into(),
            edge: "missing".into(),
            t: 0.5,
            depth_m: 0.01,
        },
        Command::Notch {
            panel: "panel_1".into(),
            edge: "edge_1".into(),
            t: 1.0,
            depth_m: 0.01,
        },
        Command::Notch {
            panel: "panel_1".into(),
            edge: "edge_1".into(),
            t: 0.5,
            depth_m: 0.0,
        },
        Command::SeamAllowance {
            panel: "panel_1".into(),
            edge: "edge_1".into(),
            width_m: 0.2,
        },
        Command::RemoveNotch {
            panel: "panel_1".into(),
            edge: "edge_1".into(),
            t: 0.5,
        },
    ] {
        assert!(authoring::apply(&base, command).is_err());
    }
    // The rejected edits left the document exactly as it was.
    assert!(polygon(&base).notches.is_empty());
    assert!(polygon(&base).seam_allowances.is_empty());
}

#[test]
fn seam_allowance_zero_clears_it() {
    let stored = step(
        &rectangle(0.4, 0.6),
        Command::SeamAllowance {
            panel: "panel_1".into(),
            edge: "edge_1".into(),
            width_m: 0.01,
        },
    );
    let cleared = step(
        &stored,
        Command::SeamAllowance {
            panel: "panel_1".into(),
            edge: "edge_1".into(),
            width_m: 0.0,
        },
    );
    assert!(polygon(&cleared).seam_allowances.is_empty());
}

#[test]
fn fresh_primitives_start_unmarked_and_booleans_reject_marked_operands() {
    let primitive = marvelous_cloth::garment::shapes::primitive(
        Primitive::Rectangle,
        DVec2::splat(0.3),
        180.,
        settings(),
    )
    .unwrap();
    assert!(primitive.notches.is_empty() && primitive.seam_allowances.is_empty());
    let marked = step(&rectangle(0.4, 0.6), notch("edge_1"));
    let with_tool = step(
        &marked,
        Command::Primitive {
            kind: Primitive::Rectangle,
            size: DVec2::splat(0.3),
            sweep_degrees: 180.,
            origin: DVec2::ZERO,
            meshing: settings(),
            fabric: "woven-reference".into(),
            replace: false,
        },
    );
    let tool = with_tool.panels.last().unwrap().id.clone();
    assert!(
        authoring::apply(
            &with_tool,
            Command::Boolean {
                a: "panel_1".into(),
                b: tool,
                op: BooleanOp::Union,
                keep_tool: false,
            }
        )
        .is_err()
    );
}

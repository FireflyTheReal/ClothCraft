//! Cut counts: how many times a piece is cut. A tech-pack specification that
//! the sheets and the cut list print, and that never touches rest geometry,
//! meshing or the simulated drape. CPU-only.
use glam::DVec2;
use marvelous_cloth::{
    authoring::{self, Command},
    garment::{
        GarmentTemplate,
        meshing::MeshingSettings,
        presets::{self, GarmentKind},
        shapes::{MirrorAxis, Primitive},
    },
};

fn settings() -> MeshingSettings {
    MeshingSettings {
        target_edge_m: 0.05,
        ..Default::default()
    }
}

fn rectangle() -> GarmentTemplate {
    authoring::apply(
        &presets::basic(GarmentKind::Tshirt, 0),
        Command::Primitive {
            kind: Primitive::Rectangle,
            size: DVec2::splat(0.3),
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

fn set(t: &GarmentTemplate, panel: &str, count: u32) -> Result<GarmentTemplate, String> {
    authoring::apply(
        t,
        Command::CutCount {
            panel: panel.into(),
            count,
        },
    )
    .map(|edit| edit.template)
}

#[test]
fn a_piece_is_cut_once_by_default() {
    let t = rectangle();
    assert_eq!(t.panels[0].cut_count, 1);
    // The default is omitted from the document, so existing files are unchanged.
    let json = serde_json::to_string(&t).unwrap();
    assert!(!json.contains("cut_count"), "{json}");
    let read: GarmentTemplate = serde_json::from_str(&json).unwrap();
    assert_eq!(read.panels[0].cut_count, 1);
}

#[test]
fn a_cut_count_survives_a_document_round_trip() {
    let t = set(&rectangle(), "panel_1", 4).unwrap();
    let json = serde_json::to_string(&t).unwrap();
    assert!(json.contains("\"cut_count\":4"), "{json}");
    let read: GarmentTemplate = serde_json::from_str(&json).unwrap();
    assert_eq!(read.panels[0].cut_count, 4);
    assert_eq!(read.compile().unwrap().panels.len(), 1);
}

#[test]
fn a_cut_count_only_touches_its_own_panel() {
    let t = presets::basic(GarmentKind::Tshirt, 0);
    let front = t.panels[0].id.clone();
    let back = t.panels[1].id.clone();
    let marked = set(&t, &front, 3).unwrap();
    assert_eq!(
        marked
            .panels
            .iter()
            .find(|p| p.id == front)
            .unwrap()
            .cut_count,
        3
    );
    assert_eq!(
        marked
            .panels
            .iter()
            .find(|p| p.id == back)
            .unwrap()
            .cut_count,
        1
    );
}

#[test]
fn a_cut_count_is_specification_not_geometry() {
    let t = rectangle();
    let marked = set(&t, "panel_1", 2).unwrap();
    // The drape is untouched: a quantity is not a rest-shape change.
    assert!(authoring::same_physics(&t, &marked));
    // Clearing it restores the exact document byte for byte, because the field
    // is omitted at its default of one.
    let cleared = set(&marked, "panel_1", 1).unwrap();
    assert_eq!(
        serde_json::to_vec(&t).unwrap(),
        serde_json::to_vec(&cleared).unwrap()
    );
}

#[test]
fn an_out_of_range_cut_count_is_rejected() {
    let t = rectangle();
    for count in [0, 100, u32::MAX] {
        assert!(
            set(&t, "panel_1", count).is_err(),
            "{count} must be refused"
        );
    }
    assert!(set(&t, "missing", 2).is_err());
    // A document that carries an invalid count is refused at compile time too,
    // rather than printing a label it cannot honour.
    let mut broken = rectangle();
    broken.panels[0].cut_count = 0;
    assert!(broken.compile().is_err());
}

#[test]
fn a_copy_is_its_own_cut() {
    // Duplicating a piece makes another physical piece: the copy starts at one
    // cut instead of inheriting a quantity that would then be doubled.
    let t = set(&rectangle(), "panel_1", 3).unwrap();
    let copy = authoring::apply(
        &t,
        Command::Duplicate {
            panel: "panel_1".into(),
        },
    )
    .unwrap()
    .template;
    assert_eq!(copy.panels.len(), 2);
    assert_eq!(copy.panels[0].cut_count, 3, "the source keeps its count");
    assert_eq!(copy.panels[1].cut_count, 1, "the copy is cut once");

    // A mirrored piece is likewise a separate cut.
    let mirrored = authoring::apply(
        &t,
        Command::Mirror {
            panel: "panel_1".into(),
            axis: MirrorAxis::X,
            copy_side: Some(1),
        },
    )
    .unwrap()
    .template;
    assert_eq!(mirrored.panels.len(), 2);
    assert_eq!(mirrored.panels[0].cut_count, 3);
    assert_eq!(mirrored.panels[1].cut_count, 1);
}

#[test]
fn the_cut_count_undoes_like_any_other_edit() {
    let t = rectangle();
    let edit = authoring::apply(
        &t,
        Command::CutCount {
            panel: "panel_1".into(),
            count: 2,
        },
    )
    .unwrap();
    assert_eq!(edit.template.panels[0].cut_count, 2);
    assert_eq!(t.panels[0].cut_count, 1);
    assert!(!edit.reset_simulation, "a quantity never resets the drape");
}

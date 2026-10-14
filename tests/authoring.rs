use glam::{DVec2, DVec3};
use marvelous_cloth::{
    authoring::{self, Command, History},
    garment::{
        GarmentTemplate,
        meshing::MeshingSettings,
        presets::{self, GarmentKind},
        seam::{BoundaryRef, Side},
    },
};

fn rectangle(width: f64) -> Command {
    // Deliberately clockwise, translated input: drawing direction is arbitrary.
    Command::Polygon {
        tangents: vec![],
        points: vec![
            DVec2::new(2., 3.),
            DVec2::new(2., 3.3),
            DVec2::new(2. + width, 3.3),
            DVec2::new(2. + width, 3.),
        ],
        meshing: MeshingSettings {
            target_edge_m: 0.05,
            ..Default::default()
        },
        fabric: "woven-reference".into(),
        replace: true,
    }
}
fn fixture() -> GarmentTemplate {
    authoring::apply(&presets::basic(GarmentKind::Tshirt, 0), rectangle(0.4))
        .unwrap()
        .template
}
fn edge(panel: &str, edge: &str) -> BoundaryRef {
    BoundaryRef {
        panel: panel.into(),
        side: Side::Top,
        edge: Some(edge.into()),
        range: [0., 1.],
    }
}

#[test]
fn drawing_roundtrip_preserves_material_shape_and_names() {
    let t = fixture();
    let g = t.compile().unwrap();
    let area: f64 = g.mesh.triangles.iter().map(|t| t.area).sum();
    assert!((area - 0.12).abs() < 1e-10);
    assert_eq!(t.panels[0].polygon.as_ref().unwrap().edges.len(), 4);
    let read: GarmentTemplate = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
    assert_eq!(read.compile().unwrap().mesh.uv, g.mesh.uv);
}

#[test]
fn invalid_edit_is_atomic_and_does_not_enter_history() {
    let t = fixture();
    let before = serde_json::to_vec(&t).unwrap();
    let Command::Polygon {
        meshing, fabric, ..
    } = rectangle(0.4)
    else {
        unreachable!()
    };
    let invalid = Command::Polygon {
        tangents: vec![],
        points: vec![DVec2::ZERO, DVec2::ONE, DVec2::Y, DVec2::X],
        meshing,
        fabric,
        replace: true,
    };
    assert!(authoring::apply(&t, invalid).is_err());
    assert_eq!(before, serde_json::to_vec(&t).unwrap());
    assert!(
        authoring::apply(
            &t,
            Command::RemovePanel {
                panel: t.panels[0].id.clone()
            }
        )
        .is_err()
    );
}

#[test]
fn rigid_gizmo_preserves_pivot_rest_mass_and_triangle_lengths() {
    let t = fixture();
    let old = t.compile().unwrap();
    let p = &t.panels[0];
    let pivot = DVec3::new(0.2, -0.15, 0.);
    let placement = authoring::rotate_about(&p.placement, pivot, DVec3::Y, 1.1);
    assert!(
        placement
            .transform(pivot)
            .distance(p.placement.transform(pivot))
            < 1e-12
    );
    let edit = authoring::apply(
        &t,
        Command::Place {
            panel: p.id.clone(),
            placement,
        },
    )
    .unwrap();
    assert_eq!(old.mesh.uv, edit.garment.mesh.uv);
    assert_eq!(old.mesh.masses, edit.garment.mesh.masses);
    for tri in &old.mesh.triangles {
        let [a, b, c] = tri.vertices;
        for (a, b) in [(a, b), (b, c), (c, a)] {
            let length = old.initial_state.x[a].distance(old.initial_state.x[b]);
            assert!(
                (length
                    - edit.garment.initial_state.x[a].distance(edit.garment.initial_state.x[b]))
                .abs()
                    < 1e-12
            );
        }
    }
}

#[test]
fn seams_survive_remaillage_and_panel_deletion_removes_references() {
    let t = fixture();
    let mut t = authoring::apply(
        &t,
        Command::Duplicate {
            panel: t.panels[0].id.clone(),
        },
    )
    .unwrap()
    .template;
    t.panels[1]
        .polygon
        .as_mut()
        .unwrap()
        .meshing
        .as_mut()
        .unwrap()
        .target_edge_m = 0.03;
    let a = t.panels[0].id.clone();
    let b = t.panels[1].id.clone();
    let edit = authoring::apply(
        &t,
        Command::Sew {
            a: edge(&a, "edge_1"),
            b: edge(&b, "edge_1"),
            reverse: true,
            ease: false,
        },
    )
    .unwrap();
    assert!(!edit.garment.seams.is_empty());
    assert!(
        authoring::apply(
            &edit.template,
            Command::Sew {
                a: edge(&a, "edge_1"),
                b: edge(&b, "edge_2"),
                reverse: true,
                ease: true
            }
        )
        .is_err()
    );
    let after = authoring::apply(&edit.template, Command::RemovePanel { panel: a }).unwrap();
    assert!(after.template.seams.is_empty());
    assert_eq!(after.template.panels.len(), 1);
}

#[test]
fn unequal_seam_lengths_require_explicit_ease() {
    let t = fixture();
    let t = authoring::apply(
        &t,
        Command::Duplicate {
            panel: t.panels[0].id.clone(),
        },
    )
    .unwrap()
    .template;
    let seam = |ease| Command::Sew {
        a: edge(&t.panels[0].id, "edge_1"),
        b: edge(&t.panels[1].id, "edge_2"),
        reverse: false,
        ease,
    };
    assert!(authoring::apply(&t, seam(false)).is_err());
    assert!(authoring::apply(&t, seam(true)).is_ok());
}

#[test]
fn undo_redo_restores_compilable_document_and_branch_clears_redo() {
    let mut history = History::default();
    let a = fixture();
    let b = authoring::apply(
        &a,
        Command::Duplicate {
            panel: a.panels[0].id.clone(),
        },
    )
    .unwrap()
    .template;
    history.record(a.clone());
    let undo = history.restore(&b, false).unwrap();
    assert_eq!(
        serde_json::to_vec(&undo.template).unwrap(),
        serde_json::to_vec(&a).unwrap()
    );
    let redo = history.restore(&undo.template, true).unwrap();
    assert_eq!(
        serde_json::to_vec(&redo.template).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    history.restore(&redo.template, false).unwrap();
    history.record(a);
    assert!(!history.can_redo());
}

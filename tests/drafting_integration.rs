//! Interactions between Firefly's point tools and ClothCraft's material domains.
use glam::{DVec2, DVec3};
use marvelous_cloth::{
    authoring::{self, Command},
    garment::{
        GarmentTemplate,
        domain::Hole,
        meshing::MeshingSettings,
        presets::{self, GarmentKind},
        shapes::{BooleanOp, MirrorAxis, Primitive},
    },
};

fn rectangle() -> GarmentTemplate {
    authoring::apply(
        &presets::basic(GarmentKind::Tshirt, 0),
        Command::Primitive {
            kind: Primitive::Rectangle,
            size: DVec2::splat(0.6),
            sweep_degrees: 180.,
            origin: DVec2::ZERO,
            meshing: MeshingSettings {
                target_edge_m: 0.06,
                ..Default::default()
            },
            fabric: "woven-reference".into(),
            replace: true,
        },
    )
    .unwrap()
    .template
}
fn lined() -> GarmentTemplate {
    let t = rectangle();
    authoring::apply(
        &t,
        Command::InternalLine {
            panel: t.panels[0].id.clone(),
            points: vec![DVec2::new(0.1, 0.2), DVec2::new(0.5, 0.2)],
            tangents: vec![DVec2::new(0.08, 0.03); 2],
        },
    )
    .unwrap()
    .template
}

#[test]
fn split_and_mirror_preserve_internal_sewing_and_targets() {
    let mut t = lined();
    let panel = t.panels[0].id.clone();
    let line = t.panels[0].polygon.as_ref().unwrap().lines[0].id.clone();
    t = authoring::apply(
        &t,
        Command::Ruffle {
            panel: panel.clone(),
            line: line.clone(),
            width: 0.06,
            ratio: 1.5,
        },
    )
    .unwrap()
    .template;
    t = authoring::apply(
        &t,
        Command::SplitBoundary {
            panel: panel.clone(),
            edge: "edge_1".into(),
            at_m: 0.3,
        },
    )
    .unwrap()
    .template;
    t.panels[0]
        .polygon
        .as_mut()
        .unwrap()
        .target_lengths
        .insert("edge_3".into(), 0.6);
    let before = serde_json::to_vec(&t).unwrap();
    for _ in 0..2 {
        t = authoring::apply(
            &t,
            Command::Mirror {
                panel: panel.clone(),
                axis: MirrorAxis::X,
                copy_side: None,
            },
        )
        .unwrap()
        .template;
    }
    assert_eq!(t.seams.len(), 1);
    assert_eq!(t.seams[0].a.edge.as_deref(), Some(line.as_str()));
    assert_eq!(
        t.panels[0]
            .polygon
            .as_ref()
            .unwrap()
            .target_length("edge_3"),
        Some(0.6)
    );
    let original: GarmentTemplate = serde_json::from_slice(&before).unwrap();
    let a = original.compile().unwrap();
    let b = t.compile().unwrap();
    assert_eq!(
        a.panels[0].boundaries.keys().collect::<Vec<_>>(),
        b.panels[0].boundaries.keys().collect::<Vec<_>>()
    );
    let area = |g: &marvelous_cloth::garment::CompiledGarment| {
        g.mesh.triangles.iter().map(|t| t.area).sum::<f64>()
    };
    assert!((area(&a) - area(&b)).abs() < 1e-10);
}

#[test]
fn numeric_resize_scales_holes_and_internal_paths_with_the_material() {
    let mut t = lined();
    t.panels[0].polygon.as_mut().unwrap().holes.push(Hole {
        points: vec![
            DVec2::new(0.2, 0.4),
            DVec2::new(0.2, 0.5),
            DVec2::new(0.3, 0.5),
            DVec2::new(0.3, 0.4),
        ],
        edges: (0..4).map(|i| format!("cut_{i}")).collect(),
    });
    let before = t.compile().unwrap();
    let changed = authoring::apply(
        &t,
        Command::Dimensions {
            panel: t.panels[0].id.clone(),
            width: 0.9,
            height: 0.45,
        },
    )
    .unwrap();
    let old = t.panels[0].polygon.as_ref().unwrap();
    let new = changed.template.panels[0].polygon.as_ref().unwrap();
    let scale = DVec2::new(1.5, 0.75);
    for (a, b) in old.holes[0].points.iter().zip(&new.holes[0].points) {
        assert!((*a * scale - *b).length() < 1e-14);
    }
    for (a, b) in old.lines[0].tangents.iter().zip(&new.lines[0].tangents) {
        assert!((*a * scale - *b).length() < 1e-14);
    }
    let mass = |g: &marvelous_cloth::garment::CompiledGarment| g.mesh.masses.iter().sum::<f64>();
    assert!((mass(&changed.garment) / mass(&before) - 1.125).abs() < 1e-3);
}

#[test]
fn moving_all_curve_anchors_is_a_rigid_translation_in_world_space() {
    let mut t = rectangle();
    let p = &mut t.panels[0];
    p.polygon.as_mut().unwrap().curves.insert(
        "edge_3".into(),
        [DVec2::new(0.4, 0.7), DVec2::new(0.2, 0.7)],
    );
    let old = p.clone();
    let delta = DVec2::new(0.04, 0.03);
    let result = authoring::apply(
        &t,
        Command::MovePoints {
            panel: old.id.clone(),
            points: vec![0, 1, 2, 3],
            delta,
        },
    )
    .unwrap();
    let new = &result.template.panels[0];
    let a = old.polygon.as_ref().unwrap();
    let b = new.polygon.as_ref().unwrap();
    for (uv_old, uv_new) in a
        .points
        .iter()
        .chain(a.curves.values().flatten())
        .zip(b.points.iter().chain(b.curves.values().flatten()))
    {
        let x = old
            .placement
            .transform(DVec3::new(uv_old.x + delta.x, -uv_old.y - delta.y, 0.));
        let y = new.placement.transform(DVec3::new(uv_new.x, -uv_new.y, 0.));
        assert!((x - y).length() < 1e-12);
    }
}

#[test]
fn boolean_rejects_discarding_a_target_specification() {
    let mut t = rectangle();
    let id = t.panels[0].id.clone();
    t = authoring::apply(&t, Command::Duplicate { panel: id.clone() })
        .unwrap()
        .template;
    t.panels[0]
        .polygon
        .as_mut()
        .unwrap()
        .target_lengths
        .insert("edge_1".into(), 0.6);
    let before = serde_json::to_vec(&t).unwrap();
    assert!(
        authoring::apply(
            &t,
            Command::Boolean {
                a: id,
                b: t.panels[1].id.clone(),
                op: BooleanOp::Union,
                keep_tool: false
            }
        )
        .is_err()
    );
    assert_eq!(serde_json::to_vec(&t).unwrap(), before);
}

#[test]
fn target_metadata_does_not_invalidate_a_saved_physical_state() {
    use marvelous_cloth::fitting::FittingProject;
    let avatar =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/makehuman/mannequin.obj");
    let mut project = FittingProject::basic(GarmentKind::Tshirt, 0, avatar);
    let scene = project.compile().unwrap();
    project.state = Some(scene.state.clone());
    let path = std::env::temp_dir().join(format!("cloth-target-cache-{}.json", std::process::id()));
    project.save(&path).unwrap();
    let mut restored = FittingProject::load(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let poly = restored.template.panels[0].polygon.as_mut().unwrap();
    poly.target_lengths.insert(poly.edges[0].clone(), 0.321);
    let loaded = restored.compile().unwrap();
    assert_eq!(loaded.state.x, scene.state.x);
    assert_eq!(loaded.state.v, scene.state.v);
}

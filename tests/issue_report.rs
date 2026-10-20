use glam::DVec3;
use marvelous_cloth::{
    fitting::FittingProject,
    garment::{GarmentTemplate, presets::GarmentKind},
    issue_report, picking,
};
use std::{collections::BTreeSet, fs};

#[test]
fn issue_capture_is_portable_material_based_and_preserves_the_accepted_state() {
    let dir = tempfile::tempdir().unwrap();
    let avatar = dir.path().join("source.obj");
    fs::write(&avatar, "v 10 0 0\nv 10 1 0\nv 10 0 1\nf 1 2 3\n").unwrap();
    let mut project = FittingProject::basic(GarmentKind::Tshirt, 1, avatar.clone());
    project.template = GarmentTemplate::two_panel_demo();
    let mut scene = project.compile().unwrap();
    scene.state.time = 7.5;
    scene.state.v[0] = DVec3::new(0.1, 0.2, 0.3);
    let before = scene.state.clone();
    let settings = marvelous_cloth::vbd::VbdSettings {
        iterations: 23,
        ..Default::default()
    };
    let marked = BTreeSet::from([0, scene.garment.mesh.triangles.len() - 1]);
    let folder = issue_report::export(
        dir.path(),
        issue_report::Capture {
            project: &project,
            scene: &scene,
            settings: &settings,
            marked: &marked,
            note: "This region stalls",
            backend: "CPU test",
            metrics: None,
        },
    )
    .unwrap();
    assert_eq!(scene.state.x, before.x);
    assert_eq!(scene.state.v, before.v);
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(folder.join("report.json")).unwrap()).unwrap();
    let faces = report["analysis"]["marked_faces"].as_array().unwrap();
    assert_eq!(faces.len(), 2);
    assert_eq!(faces[0]["panel"], scene.garment.panels[0].id);
    assert_eq!(
        faces[0]["material_triangle_m"],
        serde_json::json!(
            scene.garment.mesh.triangles[0]
                .vertices
                .map(|v| scene.garment.mesh.uv[v])
        )
    );
    assert!(faces[0].get("vertices").is_none());
    let moved = dir.path().join("moved");
    fs::rename(&folder, &moved).unwrap();
    fs::remove_file(&avatar).unwrap();
    let loaded = FittingProject::load(&moved.join("project.clothcraft.json")).unwrap();
    assert_eq!(loaded.simulation.as_ref(), Some(&settings));
    let replay = loaded.compile().unwrap();
    assert_eq!(replay.state.x, before.x);
    assert_eq!(replay.state.v, before.v);
    assert_eq!(replay.state.time, before.time);
    assert!(issue_report::analyze(&scene, &settings, &BTreeSet::from([usize::MAX])).is_err());
}

#[test]
fn diagnostic_picking_never_paints_cloth_through_the_avatar() {
    let dir = tempfile::tempdir().unwrap();
    let avatar = dir.path().join("avatar.obj");
    fs::write(&avatar, "v -10 -10 1\nv 10 -10 1\nv 0 10 1\nf 1 2 3\n").unwrap();
    let mut project = FittingProject::basic(GarmentKind::Tshirt, 1, avatar);
    project.template = GarmentTemplate::two_panel_demo();
    let mut scene = project.compile().unwrap();
    for (i, x) in scene.state.x.iter_mut().enumerate() {
        *x = scene.garment.mesh.uv[i].extend(0.);
    }
    let t = &scene.garment.mesh.triangles[0];
    let center = t
        .vertices
        .iter()
        .map(|v| scene.state.x[*v] / 3.)
        .sum::<DVec3>();
    assert!(picking::visible_cloth_face(&scene, center + DVec3::Z * 2., -DVec3::Z).is_none());
    let hit = picking::visible_cloth_face(&scene, center + DVec3::Z * 0.5, -DVec3::Z).unwrap();
    assert!(scene.garment.panels[0].triangles.contains(&hit));
    assert!(picking::visible_cloth_face(&scene, DVec3::new(20., 20., 0.5), -DVec3::Z).is_none());
}

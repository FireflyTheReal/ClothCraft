use marvelous_cloth::{document::atomic_write, fitting::FittingProject, garment::GarmentTemplate};
use std::{fs, path::PathBuf};

fn project() -> FittingProject {
    let mut p = FittingProject::basic(
        marvelous_cloth::garment::presets::GarmentKind::Tshirt,
        1,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/makehuman/mannequin.obj"),
    );
    p.template = GarmentTemplate::two_panel_demo();
    p
}

#[test]
fn portable_document_and_cached_state_survive_relocation() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.obj");
    fs::write(&source, "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
    let mut p = project();
    p.avatar_path = source.clone();
    let state = p.compile().unwrap().state;
    p.state = Some(state.clone());
    p.simulation = Some(marvelous_cloth::vbd::VbdSettings {
        iterations: 27,
        dt: 1. / 120.,
        ..Default::default()
    });
    let bundle = dir.path().join("original");
    fs::create_dir(&bundle).unwrap();
    let file = bundle.join("test.json");
    p.save(&file).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    assert!(PathBuf::from(json["avatar_path"].as_str().unwrap()).is_relative());
    let moved = dir.path().join("moved");
    fs::rename(&bundle, &moved).unwrap();
    fs::remove_file(source).unwrap();
    let loaded = FittingProject::load(&moved.join("test.json")).unwrap();
    assert_eq!(loaded.simulation, p.simulation);
    let reopened = loaded.compile().unwrap();
    assert_eq!(reopened.state.x, state.x);
    assert_eq!(reopened.state.v, state.v);
    assert_eq!(reopened.state.time, state.time);
}

#[test]
fn failed_save_preserves_previous_document() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("test.json");
    atomic_write(&file, b"previous valid bytes").unwrap();
    let mut p = project();
    p.avatar_path = dir.path().join("missing.obj");
    assert!(p.save(&file).is_err());
    assert_eq!(fs::read(&file).unwrap(), b"previous valid bytes");
    assert!(atomic_write(dir.path(), b"cannot replace directory").is_err());
    atomic_write(&file, b"replacement").unwrap();
    assert_eq!(fs::read(file).unwrap(), b"replacement");
}

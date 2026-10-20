//! CPU-only evidence export from a saved garment, without advancing simulation.
use marvelous_cloth::{fitting::FittingProject, issue_report};
use std::{collections::BTreeSet, path::Path};
fn main() -> Result<(), String> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build_global()
        .map_err(|e| e.to_string())?;
    let path = std::env::args()
        .nth(1)
        .ok_or("Usage: issue_probe PROJECT [OUTPUT]")?;
    let output = std::env::args()
        .nth(2)
        .unwrap_or("output/diagnostics".into());
    let project = FittingProject::load(Path::new(&path))?;
    let scene = project.compile()?;
    let settings = project.simulation.clone().unwrap_or_default();
    let folder = issue_report::export(
        Path::new(&output),
        issue_report::Capture {
            project: &project,
            scene: &scene,
            settings: &settings,
            marked: &BTreeSet::new(),
            note: "Saved scene inspection; no user-painted faces",
            backend: "saved-state / CPU contact probe",
            metrics: None,
        },
    )?;
    println!("{}", folder.display());
    Ok(())
}

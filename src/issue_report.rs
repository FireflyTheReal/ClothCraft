//! Local, portable simulation evidence. Marked faces use panel IDs and rest UVs.
//! No changes to the garment or solver; contact bounds are recomputed on demand.
use crate::{
    document::atomic_write,
    energy,
    fitting::{FittingProject, FittingScene},
    vbd::{VbdMetrics, VbdSettings, VbdSolver},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub fn analyze(
    scene: &FittingScene,
    settings: &VbdSettings,
    marked: &BTreeSet<usize>,
) -> Result<Value, String> {
    settings.validate()?;
    if marked
        .iter()
        .any(|i| *i >= scene.garment.mesh.triangles.len())
    {
        return Err("Marked faces belong to a different mesh; paint the region again".into());
    }
    let probe = VbdSolver::new(
        scene.garment.clone(),
        Some(&scene.collider.surface),
        &scene.state,
    )
    .and_then(|mut solver| {
        let x = solver.world.positions(&scene.state.x);
        solver
            .world
            .detect(&x, settings.contact.radius * 2., &settings.contact)
    });
    let (contact, contact_error) = match probe {
        Ok(c) => (Some(c), None),
        Err(e) => (None, Some(e)),
    };
    let garment = &scene.garment;
    // Material area remains the 2D rest area. These are observations only;
    // a flattened triangle must never silently change rest data or contacts.
    let shape: Vec<_> = garment
        .mesh
        .triangles
        .iter()
        .map(|t| triangle_shape(t.vertices.map(|v| scene.state.x[v]), t.area))
        .collect();
    let face = |index: usize| {
        let t = &garment.mesh.triangles[index];
        let panel = garment
            .panels
            .iter()
            .find(|p| p.triangles.contains(&index))
            .unwrap();
        let stretch = energy::stretches(&scene.state.x, t, &garment.materials[t.material]);
        json!({
            "panel": panel.id,
            "material_triangle_m": t.vertices.map(|v| garment.mesh.uv[v]),
            "positions_m": t.vertices.map(|v| scene.state.x[v]),
            "velocities_m_s": t.vertices.map(|v| scene.state.v[v]),
            "pinned": t.vertices.map(|v| garment.mesh.pins[v].is_some()),
            "mass_kg": t.vertices.map(|v| garment.mesh.masses[v]),
            "warp_weft_stretch_ratio": [stretch[0], stretch[1]],
            "positive_extension_percent": 100. * (stretch[0].max(stretch[1]) - 1.).max(0.),
            "area_to_rest_ratio": shape[index].0,
            "minimum_altitude_m": shape[index].1,
            "fresh_ogc_bound_m": contact.as_ref().map(|c| t.vertices.map(|v| c.radii[v])),
            "directional_half_space_counts": contact.as_ref().map(|c| t.vertices.map(|v| c.directional.at(v).len())),
            "nearby_contact_terms": contact.as_ref().map(|c| t.vertices.map(|v| c.incident[v].len()))
        })
    };
    let mut worst: Vec<_> = garment
        .mesh
        .triangles
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let s = energy::stretches(&scene.state.x, t, &garment.materials[t.material]);
            (i, s[0].max(s[1]))
        })
        .collect();
    worst.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut restricted: Vec<_> = contact
        .as_ref()
        .map(|c| {
            garment
                .mesh
                .triangles
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    (
                        i,
                        t.vertices
                            .map(|v| c.radii[v])
                            .into_iter()
                            .fold(f64::INFINITY, f64::min),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    restricted.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut compressed: Vec<_> = shape.iter().enumerate().collect();
    compressed.sort_by(|a, b| a.1.0.total_cmp(&b.1.0));
    Ok(json!({
        "time_s": scene.state.time,
        "vertices": scene.state.x.len(),
        "triangles": garment.mesh.triangles.len(),
        "marked_faces": marked.iter().copied().map(face).collect::<Vec<_>>(),
        "highest_extension_faces": worst.iter().take(20).map(|(i, _)| face(*i)).collect::<Vec<_>>(),
        "most_restricted_faces": restricted.iter().take(20).map(|(i, _)| face(*i)).collect::<Vec<_>>(),
        "most_compressed_faces": compressed.iter().take(20).map(|(i, _)| face(*i)).collect::<Vec<_>>(),
        "faces_below_one_percent_rest_area": shape.iter().filter(|s| s.0 < 0.01).count(),
        "extension_p95_and_max_percent": crate::garment::diagnostics::extension_percent(garment, &scene.state),
        "seams": garment.seams.iter().map(|s| json!({"id": s.id, "max_gap_m": s.max_gap(&scene.state.x)})).collect::<Vec<_>>(),
        "contact_probe_error": contact_error,
        "minimum_distance_m": contact.as_ref().map(|c| c.minimum_distance),
        "vertices_with_bound_below_10um": contact.as_ref().map(|c| c.radii.iter().filter(|r| **r < 1e-5).count()),
        "vertices_with_directional_bounds": contact.as_ref().map(|c| c.directional.vertex_count()),
        "contact_probe_scope": "Fresh CPU detection with a 2*radius query. Bounds are not the live worker's cached bounds. A vertex with directional half-spaces must satisfy those as well as its ball; the ball alone is not its feasible region. Small bounds may restrict motion; they do not prove a bug or convergence.",
        "shape_probe_scope": "Unsigned current triangle area divided by immutable material rest area, and minimum current altitude. The one-percent count is a diagnostic threshold only, not a solver rule. These quantities do not certify intersections or orientation."
    }))
}

fn triangle_shape([a, b, c]: [glam::DVec3; 3], rest_area: f64) -> (f64, f64) {
    let area2 = (b - a).cross(c - a).length();
    let longest = a.distance(b).max(b.distance(c)).max(c.distance(a));
    (
        area2 / (2. * rest_area),
        if longest > 0. { area2 / longest } else { 0. },
    )
}

pub struct Capture<'a> {
    pub project: &'a FittingProject,
    pub scene: &'a FittingScene,
    pub settings: &'a VbdSettings,
    pub marked: &'a BTreeSet<usize>,
    pub note: &'a str,
    pub backend: &'a str,
    pub metrics: Option<&'a VbdMetrics>,
}

/// Creates a unique subfolder. The report is written last as the completion marker.
pub fn export(root: &Path, capture: Capture<'_>) -> Result<PathBuf, String> {
    let analysis = analyze(capture.scene, capture.settings, capture.marked)?;
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let folder = tempfile::Builder::new()
        .prefix("issue-")
        .tempdir_in(root)
        .map_err(|e| e.to_string())?;
    let mut project = capture.project.clone();
    project.prearrange = false;
    project.state = Some(capture.scene.state.clone());
    project.simulation = Some(capture.settings.clone());
    project.save(&folder.path().join("project.clothcraft.json"))?;
    let report = json!({
        "format": "clothcraft-issue-report", "version": 1,
        "application_version": env!("CARGO_PKG_VERSION"),
        "backend": capture.backend, "note": capture.note,
        "project": "project.clothcraft.json", "settings": capture.settings,
        "last_step_metrics": capture.metrics,
        "analysis": analysis,
        "replay_limits": "Accepted displayed state and settings are saved. Worker prediction/contact caches are not serialized; a resumed trajectory need not be bit-identical. No data is uploaded."
    });
    atomic_write(
        &folder.path().join("report.json"),
        &serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )?;
    atomic_write(&folder.path().join("README.txt"), b"ClothCraft local issue capture\n\nOpen project.clothcraft.json with File > Open, or:\ncargo run --release --locked -j 2 --bin cloth-fit -- --project PATH/project.clothcraft.json --steps 0 --view\n\nreport.json stores marked faces by panel and material-space triangle, plus deformation/contact evidence. No persistent mesh indices. Keep this entire folder together. To ask for debugging, share the folder path or the whole folder. Exporting does not contact an AI or upload files.\n")?;
    Ok(folder.keep())
}

#[cfg(test)]
mod shape_tests {
    use super::*;
    use glam::{DQuat, DVec3};

    #[test]
    fn compression_diagnostic_is_objective_and_detects_rank_loss() {
        let rest = [DVec3::ZERO, DVec3::X * 0.02, DVec3::Y * 0.01];
        let area = 0.0001;
        assert!((triangle_shape(rest, area).0 - 1.).abs() < 1e-14);
        let mut flat = rest;
        flat[2] = DVec3::new(0.01, 2e-6, 0.);
        let expected = triangle_shape(flat, area);
        assert!((expected.0 - 0.0002).abs() < 1e-14);
        assert!((expected.1 - 2e-6).abs() < 1e-14);
        let r = DQuat::from_axis_angle(DVec3::new(1., 2., 3.).normalize(), 0.73);
        let moved = flat.map(|p| r * p + DVec3::new(2., -1., 0.3));
        let actual = triangle_shape(moved, area);
        assert!((actual.0 - expected.0).abs() < 1e-12);
        assert!((actual.1 - expected.1).abs() < 1e-12);
        assert_eq!(triangle_shape([DVec3::ZERO; 3], area), (0., 0.));
        assert_eq!(
            triangle_shape([DVec3::ZERO, DVec3::X, DVec3::X * 2.], area),
            (0., 0.)
        );
    }
}

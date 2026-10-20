//! CPU snapshot audit, not a contact potential or simulation benchmark.
use clap::Parser;
use marvelous_cloth::{fitting::FittingProject, model::State, vbd::VbdSolver};
use serde_json::json;
use std::{fs, path::PathBuf, time::Instant};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    state: Option<PathBuf>,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, value_delimiter = ',', default_value = "3,6,12")]
    radii_mm: Vec<f64>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.output.exists() {
        return Err("Choose a fresh output path".into());
    }
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build_global()?;
    let project = FittingProject::load(&args.project)?;
    let scene = project.compile()?;
    let state: State = match &args.state {
        Some(path) => serde_json::from_slice(&fs::read(path)?)?,
        None => scene.state.clone(),
    };
    if state.x.len() != scene.garment.mesh.masses.len() || state.x.iter().any(|p| !p.is_finite()) {
        return Err("Invalid snapshot".into());
    }
    // Preserve the original snapshot's references even for a later state.
    let mut solver = VbdSolver::new(scene.garment, Some(&scene.collider.surface), &scene.state)?;
    let positions = solver.world.positions(&state.x);
    let cfg = project.simulation.unwrap_or_default().contact;
    let before = solver.world.detect(&positions, 2. * cfg.radius, &cfg)?;
    let mut rows = Vec::new();
    for radius_mm in args.radii_mm {
        let radius = radius_mm * 0.001;
        let start = Instant::now();
        let hierarchy = solver.world.feature_hierarchy(&positions, 2. * radius)?;
        let build_s = start.elapsed().as_secs_f64();
        let start = Instant::now();
        let coverage = hierarchy.coverage(&solver.world.topology, &positions, radius, 0.01)?;
        let coverage_s = start.elapsed().as_secs_f64();
        let mut counts = [0usize; 4];
        for pair in &hierarchy.pairs {
            counts[pair.kind_index()] += 1;
        }
        rows.push(json!({
            "activation_radius_m":radius,"query_radius_m":hierarchy.radius_m,
            "candidate_counts_vf_ee_ve_vv":counts,
            "duplicate_visits":hierarchy.duplicate_visits,"coverage":coverage,
            "build_wall_s":build_s,"coverage_wall_s":coverage_s
        }));
    }
    let after = solver.world.detect(&positions, 2. * cfg.radius, &cfg)?;
    let unchanged = before.radii == after.radii
        && before.centers == after.centers
        && format!("{:?}", before.contacts) == format!("{:?}", after.contacts)
        && format!("{:?}", before.directional) == format!("{:?}", after.directional);
    if !unchanged {
        return Err("Snapshot audit changed production contacts or bounds".into());
    }
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        args.output,
        serde_json::to_vec_pretty(&json!({
            "scope":"Four-family snapshot coverage only; no directional weights, forces or stepping",
            "project":args.project,"state":args.state,"state_time_s":state.time,
            "cloth_vertices":solver.world.moving,"total_vertices":positions.len(),
            "production_contacts_and_bounds_unchanged":unchanged,
            "family_order":["vertex_face","edge_edge","vertex_edge","vertex_vertex"],
            "results":rows
        }))?,
    )?;
    Ok(())
}

//! CPU snapshot audit. No stepping, GPU, project writes, or installed force law.
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
    #[arg(long, value_delimiter = ',', default_value = "3,1.5,0.75,0.375")]
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
    let initial_state = scene.state;
    let state: State = if let Some(path) = &args.state {
        serde_json::from_slice(&fs::read(path)?)?
    } else {
        initial_state.clone()
    };
    if state.x.len() != scene.garment.mesh.masses.len() || state.x.iter().any(|p| !p.is_finite()) {
        return Err("Invalid snapshot".into());
    }
    // Preserve the checkpoint's geometric mollifier reference for an external
    // state, as in a continuous replay. Do not cold-reinitialize it at release.
    let solver = VbdSolver::new(scene.garment, Some(&scene.collider.surface), &initial_state)?;
    let positions = solver.world.positions(&state.x);
    let settings = project.simulation.unwrap_or_default().contact;
    let mut results = Vec::new();
    for radius in args.radii_mm {
        let mut cfg = settings.clone();
        cfg.radius = radius * 0.001;
        let start = Instant::now();
        let audit = solver.world.audit_wire_quadrature(&positions, &cfg)?;
        results.push(json!({"audit":audit,"audit_wall_s":start.elapsed().as_secs_f64()}));
    }
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        args.output,
        serde_json::to_vec_pretty(&json!({
            "scope":"Unmollified wire-density audit; no area weights, no solver integration",
            "project":args.project,"state":args.state,"state_time_s":state.time,
            "cloth_vertices":solver.world.moving,"total_vertices":positions.len(),
            "triangles":solver.world.topology.triangles.len(),"results":results
        }))?,
    )?;
    Ok(())
}

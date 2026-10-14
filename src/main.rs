use clap::Parser;
use marvelous_cloth::{
    model::Scene,
    project::{self, Project},
    solver::ReferenceSolver,
};
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
};

#[cfg(feature = "desktop")]
mod editor;
#[cfg(feature = "desktop")]
use marvelous_cloth::render;

#[derive(Parser)]
#[command(
    version,
    about = "MARVELOUS CLOTH — CPU FEM reference and native pattern editor"
)]
struct Args {
    /// Run CPU only. Does not initialize a window, graphics instance or GPU adapter.
    #[arg(long)]
    headless: bool,
    #[arg(long, default_value_t = 60)]
    steps: usize,
    #[arg(long, value_enum)]
    scene: Option<Scene>,
    /// Cells per side. Overrides project resolution.
    #[arg(long)]
    resolution: Option<usize>,
    #[arg(long)]
    grain: Option<f64>,
    #[arg(long)]
    dt: Option<f64>,
    #[arg(long)]
    project: Option<PathBuf>,
    #[arg(long, default_value = "output")]
    output: PathBuf,
    /// Number of CPU worker threads; capped by default to avoid monopolizing the host.
    #[arg(long, default_value_t = 4)]
    threads: usize,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();
    let args = Args::parse();
    if args.threads == 0 {
        return Err("--threads must be positive".into());
    }
    rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build_global()?;
    let mut p = if let Some(path) = args.project {
        Project::load(&path)?
    } else {
        Project::default()
    };
    if let Some(scene) = args.scene {
        p.scene = scene;
        p.sphere = if scene == Scene::Sphere {
            Some(Default::default())
        } else {
            None
        };
    }
    if let Some(n) = args.resolution {
        p.pattern.nx = n;
        p.pattern.ny = n;
    }
    if let Some(degrees) = args.grain {
        p.material.grain_degrees = degrees;
    }
    if let Some(dt) = args.dt {
        p.solver.dt = dt;
    }
    let (mesh, mut state) = p.build()?;
    if args.headless {
        if args.steps == 0 {
            return Err("--steps must be positive".into());
        }
        fs::create_dir_all(&args.output)?;
        let mut metrics = BufWriter::new(File::create(args.output.join("metrics.jsonl"))?);
        let mut converged = 0;
        let mut max_penetration: f64 = 0.0;
        let mut sum_ms = 0.0;
        let mut last = None;
        for _ in 0..args.steps {
            let m = ReferenceSolver::step(
                &mesh,
                &mut state,
                &p.material,
                &p.solver,
                p.sphere.as_ref(),
            )?;
            serde_json::to_writer(&mut metrics, &m)?;
            writeln!(metrics)?;
            converged += usize::from(m.converged);
            max_penetration = max_penetration.max(m.max_vertex_penetration_m);
            sum_ms += m.step_ms;
            last = Some(m);
        }
        metrics.flush()?;
        fs::write(args.output.join("cloth.obj"), project::obj(&mesh, &state))?;
        p.save(&args.output.join("project.json"))?;
        let summary = serde_json::json!({"backend":"cpu-f64","gpu_initialized":false,"threads":args.threads,
            "steps":args.steps,"converged_steps":converged,"mean_step_ms":sum_ms/args.steps as f64,
            "max_vertex_penetration_m":max_penetration,"last_step":last});
        fs::write(
            args.output.join("summary.json"),
            serde_json::to_string_pretty(&summary)?,
        )?;
        println!("{}", serde_json::to_string_pretty(&summary)?);
        if converged != args.steps {
            eprintln!(
                "WARNING: {} iteration-limited steps; inspect residuals in metrics.jsonl",
                args.steps - converged
            );
        }
        return Ok(());
    }
    #[cfg(feature = "desktop")]
    {
        editor::run(p, args.output).map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(feature = "desktop"))]
    {
        Err("Built without desktop support. Use --headless or build with default features.".into())
    }
}

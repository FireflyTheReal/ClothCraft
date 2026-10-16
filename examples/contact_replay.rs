//! CPU-only contact microbenchmark. Replays deterministic small displacements;
//! these are queries, not a simulated trajectory or a frame-rate benchmark.
use clap::Parser;
use marvelous_cloth::{fitting::FittingProject, ogc::OgcSettings, vbd::VbdSolver};
use serde::Serialize;
use std::{path::PathBuf, time::Instant};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value_t = 40)]
    repeats: usize,
}
#[derive(Serialize)]
struct Sample {
    total_ms: f64,
    profile: marvelous_cloth::ogc::DetectionProfile,
    contacts: usize,
    active: usize,
    rebuilt: bool,
    fingerprint: String,
}
fn hash(bytes: impl IntoIterator<Item = u8>, hash: &mut u64) {
    for b in bytes {
        *hash ^= u64::from(b);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build_global()?;
    let scene = FittingProject::load(&args.project)?.compile()?;
    let mut solver = VbdSolver::new(
        scene.garment.clone(),
        Some(&scene.collider.surface),
        &scene.state,
    )?;
    let settings = OgcSettings::default();
    let mut samples = Vec::new();
    for frame in 0..args.repeats {
        let mut moving = scene.state.x.clone();
        for (i, p) in moving.iter_mut().enumerate() {
            let phase = frame as f64 * 0.17 + i as f64 * 0.13;
            p.x += phase.sin() * 0.0002;
            p.z += phase.cos() * 0.0002;
        }
        let x = solver.world.positions(&moving);
        let start = Instant::now();
        let c = solver.world.detect(&x, settings.radius * 2., &settings)?;
        let total_ms = start.elapsed().as_secs_f64() * 1000.;
        let mut fingerprint = 0xcbf29ce484222325;
        for r in &c.radii {
            hash(r.to_le_bytes(), &mut fingerprint);
        }
        for contact in &c.contacts {
            hash([contact.kind as u8], &mut fingerprint);
            for v in contact.vertices {
                hash(v.to_le_bytes(), &mut fingerprint);
            }
            hash(contact.mollifier_epsilon.to_le_bytes(), &mut fingerprint);
        }
        samples.push(Sample {
            total_ms,
            profile: c.profile,
            contacts: c.contacts.len(),
            active: c.active_contacts,
            rebuilt: c.broad_phase_rebuilt,
            fingerprint: format!("{fingerprint:016x}"),
        });
    }
    if let Some(parent) = args.output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &args.output,
        serde_json::to_string_pretty(&serde_json::json!({
            "kind": "CPU contact query replay, not simulation or FPS", "threads": 2,
            "vertices": scene.state.x.len(), "samples": samples
        }))?,
    )?;
    println!("{}", args.output.display());
    Ok(())
}

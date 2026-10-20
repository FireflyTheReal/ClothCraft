//! Static planar-slide falsification gate: no friction, constant true gap.
use clap::Parser;
use glam::DVec3;
use marvelous_cloth::ogc::{OgcSettings, OgcWorld};
use serde_json::json;
use std::{fs, path::PathBuf};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.output.exists() {
        return Err("Choose a new output path".into());
    }
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build_global()?;
    let cfg = OgcSettings::default();
    let mut x = vec![
        DVec3::new(-0.004, 0.001, 0.),
        DVec3::new(-0.01, 0., -0.01),
        DVec3::new(0.01, 0., -0.01),
        DVec3::new(0.01, 0., 0.01),
        DVec3::new(-0.01, 0., 0.01),
    ];
    let mut original = OgcWorld::new(&x, vec![[1, 2, 3], [1, 3, 4]], 1, [])?;
    original.directional_bounds = true;
    let mut candidate = original.clone();
    candidate.experimental_primitive_distances = true;
    let mut samples = Vec::new();
    for i in 0..=160 {
        x[0].x = -0.004 + 0.008 * i as f64 / 160.;
        let old = original.detect(&x, 2. * cfg.radius, &cfg)?;
        let new = candidate.detect(&x, 2. * cfg.radius, &cfg)?;
        assert_eq!(old.radii, new.radii);
        assert_eq!(old.directional, new.directional);
        let gradient = |set: &marvelous_cloth::ogc::ContactSet| {
            set.contacts
                .iter()
                .map(|c| c.block(&x, 0, cfg.radius, cfg.stiffness).1)
                .sum::<DVec3>()
        };
        samples.push(json!({"x_m":x[0].x,"gap_m":x[0].y,
            "baseline_j":old.value(&x,&cfg),"primitive_j":new.value(&x,&cfg),
            "baseline_force_n":-gradient(&old),"primitive_force_n":-gradient(&new)}));
    }
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        args.output,
        serde_json::to_vec_pretty(&json!({"config":cfg,"samples":samples}))?,
    )?;
    Ok(())
}

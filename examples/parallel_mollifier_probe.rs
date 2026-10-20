//! CPU-only feature weight audit; does not assemble a force law or step cloth.
use clap::Parser;
use marvelous_cloth::{
    fitting::FittingProject,
    math::{Jet, Scalar},
    ogc::{
        feature_mollifier,
        potential::{Kind, activation},
        smooth_forest::{Link, evaluate},
    },
    vbd::VbdSolver,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::PathBuf};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    project: Option<PathBuf>,
}
fn parallel_sweep() -> Result<Vec<Value>, String> {
    let mut rows = Vec::new();
    for degrees in [
        0_f64, 0.001, 0.01, 0.1, 0.25, 0.5, 0.75, 1., 1.5, 2., 5., 90.,
    ] {
        let theta = degrees.to_radians();
        let a = [-0.01 * theta.cos(), 0.001, -0.01 * theta.sin()];
        let b = [0.01 * theta.cos(), 0.001, 0.01 * theta.sin()];
        let target = [[-0.02, 0., 0.], [0., 0., 0.], [0.02, 0., 0.]];
        let q = theta.sin().powi(2) / 0.001;
        let weight = if q < 1. { q * (2. - q) } else { 1. };
        let barrier = activation(Jet::<1>::variable(0.001, 0), 0.003, 100_000.);
        let c = Jet::c;
        let consistent = evaluate(
            &[c(weight) * barrier; 2],
            &[Link {
                nodes: [0, 1],
                barrier: c(weight * weight) * barrier,
            }],
            1e-11,
        )?
        .energy;
        let naive = c(2. * weight - 1.) * barrier;
        let feature = feature_mollifier::edge_edge([a, b, target[0], target[2]], 0.01)?;
        let halves = [
            feature_mollifier::edge_edge([a, b, target[0], target[1]], 0.01)?,
            feature_mollifier::edge_edge([a, b, target[1], target[2]], 0.01)?,
        ];
        let lower = feature_mollifier::vertex_edge([target[1], a, b], 0.01)?;
        rows.push(json!({"degrees":degrees,"ogc_parallel_weight":weight,
            "positive_only_signed_energy_j":naive.v,"product_link_energy_j":consistent.v,
            "product_link_normal_force_n":-consistent.g[0],
            "single_representative_force_n":-weight*barrier.g[0],
            "gcp_full_edge_factor":feature,"gcp_half_edge_factors":halves,
            "gcp_shared_vertex_edge_factor":lower}));
    }
    Ok(rows)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.output.exists() {
        return Err("Choose a fresh output path".into());
    }
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build_global()?;
    let mut counts: BTreeMap<String, [usize; 4]> = BTreeMap::new();
    let mut time = None;
    if let Some(path) = &args.project {
        let project = FittingProject::load(path)?;
        let scene = project.compile()?;
        time = Some(scene.state.time);
        let mut solver =
            VbdSolver::new(scene.garment, Some(&scene.collider.surface), &scene.state)?;
        let positions = solver.world.positions(&scene.state.x);
        let cfg = project.simulation.unwrap_or_default().contact;
        let contacts = solver.world.detect(&positions, cfg.radius * 2., &cfg)?;
        for term in contacts.contacts {
            if term.value(&positions, cfg.radius, cfg.stiffness) <= 0. {
                continue;
            }
            let p = term.vertices.map(|i| positions[i].to_array());
            let m = match term.kind {
                Kind::PointPoint => Ok(1.),
                Kind::PointEdge => feature_mollifier::vertex_edge([p[0], p[1], p[2]], 0.01),
                Kind::PointFace => feature_mollifier::vertex_face(p, 0.01),
                Kind::EdgeEdge => feature_mollifier::edge_edge(p, 0.01),
                _ => Err("Unexpected experimental contact".into()),
            };
            let row = counts.entry(format!("{:?}", term.kind)).or_default();
            match m {
                Err(_) => row[3] += 1,
                Ok(0.) => row[0] += 1,
                Ok(1.) => row[2] += 1,
                Ok(_) => row[1] += 1,
            }
        }
    }
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        args.output,
        serde_json::to_vec_pretty(&json!({
            "scope":"Scalar feature factors and a rejected product-weight forest trial; no GCP force law, collision or trajectory validation",
            "width":0.01,"project":args.project,"state_time_s":time,
            "parallel_sweep":parallel_sweep()?,
            "active_production_terms_factor_bins":counts,
            "bin_labels":["zero","fractional","one","evaluation_error"],
            "coverage_warning":"Counts over production terms do not establish the required GCP lower-feature contact coverage."
        }))?,
    )?;
    Ok(())
}

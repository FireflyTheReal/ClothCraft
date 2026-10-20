//! Replay an actual failed dense contact QP without advancing a simulation.
use clap::Parser;
use marvelous_cloth::solver_lab::{
    contact_qp::Settings,
    dense_contact::{self, DualMethod, Row},
};
use serde::Deserialize;
use serde_json::json;
use std::{fs, path::PathBuf, time::Instant};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value = "failed_repair_qp")]
    field: String,
    #[arg(long, value_delimiter = ',', default_value = "8192")]
    sweeps: Vec<usize>,
}
#[derive(Deserialize)]
struct CapturedRow {
    jacobian: Vec<f64>,
    offset: f64,
}
#[derive(Deserialize)]
struct Captured {
    metric: Vec<f64>,
    gradient: Vec<f64>,
    rows: Vec<CapturedRow>,
}
fn main() -> Result<(), String> {
    let args = Args::parse();
    let source: serde_json::Value =
        serde_json::from_slice(&fs::read(&args.input).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let captured: Captured = serde_json::from_value(source[&args.field].clone())
        .map_err(|e| format!("Invalid/missing captured QP: {e}"))?;
    let rows: Vec<_> = captured
        .rows
        .into_iter()
        .map(|r| Row {
            jacobian: r.jacobian,
            offset: r.offset,
        })
        .collect();
    let mut runs = vec![];
    for sweeps in args.sweeps {
        for method in [
            DualMethod::Coordinate,
            DualMethod::ActiveBlock,
            DualMethod::IndependentBlock,
            DualMethod::IndependentPivotBlock,
        ] {
            let start = Instant::now();
            let report = dense_contact::solve_with_method(
                &captured.metric,
                &captured.gradient,
                &rows,
                Settings {
                    max_sweeps: sweeps,
                    feasibility_m: 1e-11,
                    stationarity_n: 1e-7,
                    complementarity_j: 1e-12,
                },
                method,
            );
            let elapsed = start.elapsed().as_secs_f64() * 1000.;
            let result = match report {
                Ok(report) => {
                    json!({"method":method,"budget":sweeps,"elapsed_ms":elapsed,"report":report})
                }
                Err(error) => {
                    json!({"method":method,"budget":sweeps,"elapsed_ms":elapsed,"error":error})
                }
            };
            println!(
                "{method:?}, budget={sweeps}, converged={}",
                result["report"]["converged"]
            );
            runs.push(result);
        }
    }
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(args.output,serde_json::to_vec_pretty(&json!({
        "input":args.input,"field":args.field,"dofs":captured.gradient.len(),"rows":rows.len(),
        "scope":"One captured search QP, not a simulation or a collision-valid state","runs":runs
    })).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    Ok(())
}

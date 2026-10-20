//! Replay public constrained-search captures without running the cloth model.
use marvelous_cloth::solver_lab::{constrained_least_squares, dense_contact::Row};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("Usage: constrained_ls_replay INPUT.json OUTPUT.json".into());
    }
    let p: serde_json::Value = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let read = |key: &str| -> Result<Vec<f64>, Box<dyn std::error::Error>> {
        p[key]
            .as_array()
            .ok_or("Missing numeric array")?
            .iter()
            .map(|v| v.as_f64().ok_or_else(|| "Invalid number".into()))
            .collect()
    };
    let n = p["columns"].as_u64().ok_or("Missing columns")? as usize;
    let free = p["free_prefix"].as_u64().ok_or("Missing free prefix")? as usize;
    let mut rows = vec![];
    for row in p["geometry"].as_array().ok_or("Missing geometry")? {
        let mut j: Vec<_> = row["jacobian"]
            .as_array()
            .ok_or("Missing Jacobian")?
            .iter()
            .map(|v| v.as_f64().ok_or("Invalid Jacobian"))
            .collect::<Result<_, _>>()?;
        if j.len() != free || free > n {
            return Err("Invalid geometric dimensions".into());
        }
        j.resize(n, 0.);
        rows.push(Row {
            jacobian: j,
            offset: row["offset"].as_f64().ok_or("Missing offset")?,
        });
    }
    for i in free..n {
        let mut j = vec![0.; n];
        j[i] = 1.;
        rows.push(Row {
            jacobian: j,
            offset: 0.,
        });
    }
    let report = constrained_least_squares::solve(
        &read("a")?,
        &read("b")?,
        n,
        &rows,
        &read("initial")?,
        512,
    )?;
    std::fs::write(&args[2], serde_json::to_vec_pretty(&report)?)?;
    println!(
        "converged={}, residual={}, feasibility={}, iterations={}",
        report.converged, report.residual_norm, report.normalized_feasibility, report.iterations
    );
    Ok(())
}

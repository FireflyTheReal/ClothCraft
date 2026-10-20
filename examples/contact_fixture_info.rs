//! Export compiled material connectivity for passive synthetic motion analysis.
use clap::Parser;
use serde_json::json;
use std::path::PathBuf;
#[path = "support/deforming_fixture.rs"]
#[allow(dead_code)] // Metadata export does not construct the analytic floor.
mod fixture;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    sewn_ease: Option<f64>,
    #[arg(long, default_value_t = 20.)]
    fold_degrees: f64,
}
fn main() -> Result<(), String> {
    let args = Args::parse();
    let g = fixture::template(args.sewn_ease, args.fold_degrees)?.compile()?;
    let result = json!({
        "vertices":g.mesh.masses.len(),"sewn_ease":args.sewn_ease,
        "fold_degrees":args.sewn_ease.map(|_|args.fold_degrees),
        "masses_kg":g.mesh.masses,
        "triangles":g.mesh.triangles.iter().map(|t|t.vertices).collect::<Vec<_>>(),
        "seam_supports":g.seams.iter().flat_map(|s|s.samples.iter().map(|p|
            p.point.terms.iter().filter(|(_,w)|*w!=0.).map(|(i,_)|*i).collect::<Vec<_>>()
        )).collect::<Vec<_>>(),
        "initial_positions":g.initial_state.x,
    });
    if let Some(parent) = args.output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        args.output,
        serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

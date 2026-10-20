//! Offline work audit on captured reactions. Does not integrate or predict
//! a corrected trajectory: multipliers and positions remain those of the input.
use clap::Parser;
use glam::DVec3;
use marvelous_cloth::{ogc::feature_hierarchy::Pair, solver_lab::discrete_contact, vbd::VbdSolver};
use serde::Deserialize;
use serde_json::json;
use std::{fs, path::PathBuf, sync::Arc};
#[path = "support/deforming_fixture.rs"]
mod fixture;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    output: PathBuf,
}
#[derive(Deserialize)]
struct Contact {
    kind: String,
    indices: [usize; 2],
    multiplier: f64,
}
#[derive(Deserialize)]
struct Reactions {
    previous_x: Vec<DVec3>,
    floor_multipliers: Vec<f64>,
    contacts: Vec<Contact>,
}
#[derive(Deserialize)]
struct Frame {
    step: usize,
    x: Vec<DVec3>,
    reactions: Option<Reactions>,
    energy_balance: Option<serde_json::Value>,
}
#[derive(Deserialize)]
struct Trace {
    force_law: Option<String>,
    sewn_ease: Option<f64>,
    fold_degrees: Option<f64>,
    vertices: usize,
    frames: Vec<Frame>,
}
fn main() -> Result<(), String> {
    let args = Args::parse();
    let trace: Trace = serde_json::from_slice(&fs::read(&args.input).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let g =
        Arc::new(fixture::template(trace.sewn_ease, trace.fold_degrees.unwrap_or(20.))?.compile()?);
    if g.mesh.masses.len() != trace.vertices {
        return Err("Trace fixture mismatch".into());
    }
    let reference = VbdSolver::new(g.clone(), Some(&fixture::floor()), &g.initial_state)?;
    let top = &reference.world.topology;
    let mut frames = vec![];
    let mut maximum_chain_error: f64 = 0.;
    let mut maximum_force_resultant: f64 = 0.;
    let mut maximum_midpoint_torque: f64 = 0.;
    for frame in trace.frames {
        let Some(reactions) = frame.reactions else {
            continue;
        };
        if frame.x.len() != trace.vertices
            || reactions.previous_x.len() != trace.vertices
            || reactions.floor_multipliers.len() != trace.vertices
            || reactions
                .floor_multipliers
                .iter()
                .any(|v| !v.is_finite() || *v < 0.)
        {
            return Err("Invalid captured state/reactions".into());
        }
        let old = reference.world.positions(&reactions.previous_x);
        let new = reference.world.positions(&frame.x);
        let floor_work: f64 = reactions
            .floor_multipliers
            .iter()
            .enumerate()
            .map(|(i, l)| l * (new[i].y - old[i].y))
            .sum();
        let mut endpoint_work = floor_work;
        let mut discrete_work = floor_work;
        let mut frozen_work = floor_work;
        let mut contacts = vec![];
        for c in reactions.contacts {
            if !c.multiplier.is_finite() || c.multiplier < 0. {
                return Err("Invalid contact multiplier".into());
            }
            let pair = match c.kind.as_str() {
                "vf" => Pair::VertexFace(c.indices),
                "ee" => Pair::EdgeEdge(c.indices),
                _ => return Err("Unsupported captured contact".into()),
            };
            let row = discrete_contact::row(pair, top, &old, &new)?;
            let work = |gradient: &[DVec3; 4]| -> f64 {
                c.multiplier
                    * row
                        .vertices
                        .iter()
                        .zip(gradient)
                        .map(|(&i, f)| f.dot(new[i] - old[i]))
                        .sum::<f64>()
            };
            let end = work(&row.endpoint_gradient);
            let discrete = work(&row.gradient);
            let frozen = work(&row.frozen_witness_gradient);
            endpoint_work += end;
            discrete_work += discrete;
            frozen_work += frozen;
            maximum_chain_error = maximum_chain_error.max(row.chain_error_m.abs());
            let resultant = row.gradient.iter().copied().sum::<DVec3>().length();
            maximum_force_resultant = maximum_force_resultant.max(resultant);
            let origin = 0.5 * (old[row.vertices[0]] + new[row.vertices[0]]);
            let torque: DVec3 = row
                .vertices
                .iter()
                .zip(&row.gradient)
                .map(|(&i, f)| (0.5 * (old[i] + new[i]) - origin).cross(*f))
                .sum();
            maximum_midpoint_torque = maximum_midpoint_torque.max(torque.length());
            contacts.push(json!({"kind":c.kind,"indices":c.indices,"multiplier_n":c.multiplier,
                "distance_before_m":row.distance_before,"distance_after_m":row.distance_after,
                "endpoint_work_j":end,"discrete_work_j":discrete,"frozen_witness_work_j":frozen,
                "chain_error_m":row.chain_error_m,"near_invariant_coincidence":row.near_invariant_coincidence}));
        }
        frames.push(
            json!({"step":frame.step,"input_energy_balance":frame.energy_balance,
            "floor_work_j":floor_work,"endpoint_contact_work_j":endpoint_work,
            "discrete_contact_work_at_input_multipliers_j":discrete_work,
            "frozen_witness_work_at_input_multipliers_j":frozen_work,"contacts":contacts}),
        );
    }
    if frames.is_empty() {
        return Err("No captured reaction frames; use --capture-reactions".into());
    }
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let report = json!({"scope":"Offline force-row audit; unchanged input positions/multipliers, no trajectory prediction",
        "input":args.input,"input_force_law":trace.force_law.unwrap_or_else(||"endpoint".into()),"production_changed":false,"gpu_used":false,"maximum_chain_error_m":maximum_chain_error,
        "maximum_unit_reaction_resultant":maximum_force_resultant,"maximum_unit_reaction_midpoint_torque_m":maximum_midpoint_torque,
        "frames":frames});
    fs::write(&args.output, serde_json::to_vec_pretty(&report).unwrap())
        .map_err(|e| e.to_string())?;
    println!(
        "audited {} frames, maximum chain error {} m",
        report["frames"].as_array().unwrap().len(),
        maximum_chain_error
    );
    Ok(())
}

//! Small sewn-cloth floor experiment, using production material/seam energies.
//! Hard floor contact + unchanged OGC guards. Dense research solve, no GPU.
use clap::Parser;
use glam::DVec3;
use marvelous_cloth::{
    avatar::SurfaceMesh,
    garment::GarmentTemplate,
    model::Pattern,
    solver_lab::{
        contact_qp::Settings,
        coupled_patch,
        dense_contact::{self, DualMethod, Row},
    },
    vbd::VbdSolver,
};
use serde_json::json;
use std::{fs, path::PathBuf, sync::Arc, time::Instant};
const GAP: f64 = 0.001;
const DT: f64 = 1. / 60.;
#[derive(Parser)]
struct Args {
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value_t = 360)]
    steps: usize,
    #[arg(long)]
    active_block: bool,
}
fn run(
    ease: f64,
    held: bool,
    steps: usize,
    method: DualMethod,
) -> Result<serde_json::Value, String> {
    let mut t = GarmentTemplate::two_panel_demo();
    for (i, p) in t.panels.iter_mut().enumerate() {
        p.pins.clear();
        p.pattern = Pattern {
            width: 0.06,
            height: 0.08 * if i == 1 { 1. + ease } else { 1. },
            nx: 1,
            ny: 2,
        };
        p.placement.rotation_degrees = DVec3::new(90., 0., 0.);
        p.placement.translation = DVec3::new(if i == 0 { -0.061 } else { 0.001 }, GAP, -0.04);
    }
    t.seams[0].allow_ease = true;
    let g = Arc::new(t.compile()?);
    let floor = SurfaceMesh {
        vertices: vec![
            DVec3::new(-1., 0., -1.),
            DVec3::new(1., 0., -1.),
            DVec3::new(0., 0., 1.),
        ],
        triangles: vec![[0, 1, 2]],
    };
    let mut state = g.initial_state.clone();
    let mut reference = VbdSolver::new(g.clone(), Some(&floor), &state)?;
    let cfg = marvelous_cloth::ogc::OgcSettings::default();
    let n = state.x.len();
    let dofs = 3 * n;
    let inertia: Vec<_> = g.mesh.masses.iter().map(|m| m / (DT * DT)).collect();
    let release = if held { 180 } else { 120 };
    let control_vertex = 0;
    let control_origin = state.x[control_vertex];
    let mut trace = vec![];
    let mut failure = None;
    let mut previous = 0.;
    let mut max_rise: f64 = 0.;
    let start = Instant::now();
    'steps: for step in 0..steps {
        let old = state.x.clone();
        let target: Vec<_> = old
            .iter()
            .zip(&state.v)
            .map(|(p, v)| *p + DT * (*v - DT * 9.81 * DVec3::Y))
            .collect();
        let grabbing = (60..release).contains(&step);
        let control = control_origin
            + ((step as f64 - 59.) / 60.).clamp(0., 1.) * DVec3::new(0.02, 0.015, 0.);
        let mut converged = false;
        let mut iterations = 0;
        let mut correction = f64::INFINITY;
        let mut residual = f64::INFINITY;
        let mut complementarity = f64::INFINITY;
        let mut sweeps = 0;
        let mut block_steps = 0;
        let mut singular_blocks = 0;
        let objective = |x: &[DVec3]| -> Result<f64, String> {
            Ok(coupled_patch::assemble(&g, x, &target, &inertia)?.energy
                + if grabbing {
                    0.5 * 10. * x[control_vertex].distance_squared(control)
                } else {
                    0.
                })
        };
        for iteration in 0..96 {
            iterations = iteration + 1;
            let mut s = coupled_patch::assemble(&g, &state.x, &target, &inertia)?;
            if grabbing {
                let delta = state.x[control_vertex] - control;
                s.energy += 5. * delta.length_squared();
                for axis in 0..3 {
                    let k = 3 * control_vertex + axis;
                    s.gradient[k] += 10. * delta[axis];
                    s.metric[k * dofs + k] += 10.;
                }
            }
            let rows: Vec<_> = (0..n)
                .map(|i| {
                    let mut jacobian = vec![0.; dofs];
                    jacobian[3 * i + 1] = 1.;
                    Row {
                        jacobian,
                        offset: GAP - state.x[i].y,
                    }
                })
                .collect();
            let q = dense_contact::solve_with_method(
                &s.metric,
                &s.gradient,
                &rows,
                Settings {
                    max_sweeps: 8192,
                    feasibility_m: 1e-11,
                    stationarity_n: 1e-7,
                    complementarity_j: 1e-12,
                },
                method,
            )?;
            sweeps += q.sweeps;
            block_steps += q.active_block_steps;
            singular_blocks += q.singular_active_blocks;
            if !q.converged {
                failure = Some(format!(
                    "QP at step {step}: feasibility {}, complementarity {}",
                    q.feasibility_m, q.complementarity_j
                ));
                break 'steps;
            }
            correction = q.displacement.iter().map(|v| v * v).sum::<f64>().sqrt();
            let mut balance = s.gradient.clone();
            complementarity = 0.;
            for (row, lambda) in rows.iter().zip(&q.multipliers) {
                for (b, j) in balance.iter_mut().zip(&row.jacobian) {
                    *b -= lambda * j;
                }
                complementarity = complementarity.max((lambda * row.offset).abs());
            }
            residual = balance.iter().map(|v| v * v).sum::<f64>().sqrt();
            if correction < 1e-9 && residual < 1e-7 && complementarity < 1e-12 {
                converged = true;
                break;
            }
            let x = reference.world.positions(&state.x);
            let guards = reference.world.detect(&x, 2. * cfg.radius, &cfg)?;
            let directions: Vec<_> = q
                .displacement
                .as_chunks::<3>()
                .0
                .iter()
                .map(|p| DVec3::from_array(*p))
                .collect();
            let mut alpha: f64 = 1.;
            for (i, d) in directions.iter().enumerate() {
                if d.length() > 1e-15 {
                    let (end, _) = guards.truncate_step(i, state.x[i], state.x[i] + *d);
                    alpha = alpha.min(end.distance(state.x[i]) / d.length());
                }
            }
            let slope: f64 = q
                .displacement
                .iter()
                .zip(&s.gradient)
                .map(|(a, b)| a * b)
                .sum();
            let mut accepted = false;
            for _ in 0..32 {
                let trial: Vec<_> = state
                    .x
                    .iter()
                    .zip(&directions)
                    .map(|(p, d)| *p + alpha * *d)
                    .collect();
                if trial.iter().all(|p| p.y >= GAP - 1e-11)
                    && objective(&trial).is_ok_and(|e| e <= s.energy + 1e-4 * alpha * slope + 1e-14)
                {
                    state.x = trial;
                    accepted = true;
                    break;
                }
                alpha *= 0.5;
            }
            if !accepted {
                failure = Some(format!("Line search at step {step}, iteration {iteration}"));
                break 'steps;
            }
        }
        reference
            .world
            .validate_start(&reference.world.positions(&state.x))?;
        for ((v, p), before) in state.v.iter_mut().zip(&state.x).zip(&old) {
            *v = (*p - *before) / DT;
        }
        state.time += DT;
        let material = coupled_patch::assemble(&g, &state.x, &state.x, &inertia)?.energy;
        let kinetic: f64 = state
            .v
            .iter()
            .zip(&g.mesh.masses)
            .map(|(v, m)| 0.5 * m * v.length_squared())
            .sum();
        let mass: f64 = g.mesh.masses.iter().sum();
        let momentum: DVec3 = state
            .v
            .iter()
            .zip(&g.mesh.masses)
            .map(|(v, m)| *m * *v)
            .sum();
        let relative_kinetic: f64 = state
            .v
            .iter()
            .zip(&g.mesh.masses)
            .map(|(v, m)| 0.5 * m * (*v - momentum / mass).length_squared())
            .sum();
        let gravity: f64 = state
            .x
            .iter()
            .zip(&g.mesh.masses)
            .map(|(p, m)| m * 9.81 * p.y)
            .sum();
        let energy = material + kinetic + gravity;
        if step > release {
            max_rise = max_rise.max(energy - previous);
        }
        previous = energy;
        trace.push(
            json!({"step":step,"iterations":iterations,"qp_sweeps":sweeps,"active_block_steps":block_steps,"singular_blocks":singular_blocks,"correction_m":correction,"residual_n":residual,"complementarity_j":complementarity,
            "converged":converged,"material_j":material,"kinetic_j":kinetic,"relative_kinetic_j":relative_kinetic,"momentum":momentum,"energy_j":energy,
            "min_height_m":state.x.iter().map(|p|p.y).fold(f64::INFINITY,f64::min),
            "max_seam_gap_m":g.max_seam_gap(&state.x),"x":state.x,"v":state.v}),
        );
    }
    Ok(
        json!({"method":method,"ease":ease,"held":held,"vertices":n,"elapsed_ms":start.elapsed().as_secs_f64()*1000.,
        "failure":failure,"max_post_release_energy_rise_j":max_rise,"frames":trace}),
    )
}
fn main() -> Result<(), String> {
    let args = Args::parse();
    let mut runs = vec![];
    for ease in [0., 0.1] {
        for held in [false, true] {
            let result = run(
                ease,
                held,
                args.steps,
                if args.active_block {
                    DualMethod::ActiveBlock
                } else {
                    DualMethod::Coordinate
                },
            )?;
            println!(
                "ease={ease} held={held}: frames={}, failure={}",
                result["frames"].as_array().unwrap().len(),
                result["failure"]
            );
            runs.push(result);
        }
    }
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(args.output, serde_json::to_vec_pretty(&runs).unwrap()).map_err(|e| e.to_string())
}

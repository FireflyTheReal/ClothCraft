//! CPU-only settling experiments. No sleep, velocity clamp or collision bypass.
//! A quiet observation is not a proof of equilibrium; see docs/settling-zoo.md.
use clap::Parser;
use glam::DVec3;
use marvelous_cloth::{
    avatar::SurfaceMesh,
    garment::{GarmentTemplate, template::Corner},
    model::State,
    vbd::{Grab, VbdSettings, VbdSolver},
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{collections::VecDeque, fs, io::Write, path::Path, sync::Arc};

#[derive(Parser, Serialize)]
struct Args {
    #[arg(long, default_value = "output/settling-zoo")]
    output: std::path::PathBuf,
    /// Comma-separated fixture names; omitted means all fixtures.
    #[arg(long, value_delimiter = ',')]
    cases: Vec<String>,
    #[arg(long, default_value_t = 20.)]
    settle_seconds: f64,
    /// Each continuation lasts this long, including 1 s pull + 0.5 s hold.
    #[arg(long, default_value_t = 10.)]
    continuation_seconds: f64,
    #[arg(long, default_value_t = 16)]
    iterations: usize,
    /// Zero is the canonical geometry; other seeds vary size and seam easing.
    #[arg(long, default_value_t = 0)]
    seed: u64,
    #[arg(long, default_value_t = 3)]
    resolution: usize,
    /// Research only: avoid a Gershgorin shift when the actual block is SPD.
    #[arg(long)]
    unshifted_spd: bool,
    /// Research only: maximum line-search trials (production uses ten).
    #[arg(long, default_value_t = 10)]
    backtracks: usize,
    /// Reuse each fixture's world-space grab anchor from an earlier zoo run.
    #[arg(long)]
    controls_from: Option<std::path::PathBuf>,
}

const CASES: &[&str] = &[
    "zero-gravity",
    "free-fall",
    "hanging",
    "seam-equal",
    "seam-eased",
    "floor-face",
    "floor-diagonal",
    "slope",
];
const DT: f64 = 1. / 60.;
const WINDOW: f64 = 2.;
// Measurement thresholds only. They never affect a simulation step.
const QUIET_SPAN: f64 = 10e-6;
const QUIET_SPEED: f64 = 1e-4;

fn classify(full_window: bool, span: f64, speed: f64, unbalanced: bool) -> &'static str {
    if !full_window {
        "insufficient-observation"
    } else if span > QUIET_SPAN || speed > QUIET_SPEED {
        "still-moving"
    } else if unbalanced {
        "quiet-with-unbalanced-forces"
    } else {
        "quiet-observed"
    }
}

fn fixture(name: &str, seed: u64, resolution: usize) -> (GarmentTemplate, Option<SurfaceMesh>) {
    let mut bits = seed;
    let mut random = || {
        bits = bits
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (bits >> 11) as f64 / (1_u64 << 53) as f64
    };
    let width = if seed == 0 {
        0.15
    } else {
        0.12 + 0.06 * random()
    };
    let height = if seed == 0 {
        0.30
    } else {
        0.24 + 0.12 * random()
    };
    let ease = if seed == 0 {
        1.15
    } else {
        1.05 + 0.20 * random()
    };
    let mut t = GarmentTemplate::two_panel_demo();
    t.name = format!("Settling zoo / {name} / seed {seed}");
    for (i, p) in t.panels.iter_mut().enumerate() {
        p.pattern.width = width;
        p.pattern.height = height
            * if i == 1 && name == "seam-eased" {
                ease
            } else {
                1.
            };
        p.pattern.nx = resolution;
        p.pattern.ny = resolution + 1 + usize::from(i == 1 && name == "seam-eased");
        p.placement.rotation_degrees = DVec3::ZERO;
        p.placement.translation = if i == 0 {
            DVec3::new(-width, 1., 0.)
        } else {
            DVec3::new(0.004, 1., 0.006)
        };
        p.pins = vec![if i == 0 {
            Corner::TopLeft
        } else {
            Corner::TopRight
        }];
    }
    t.seams[0].allow_ease = true;
    t.seams[0].notches.clear();
    if !name.starts_with("seam-") {
        t.panels.truncate(1);
        t.seams.clear();
        t.panels[0].pins = if name == "hanging" {
            vec![Corner::TopLeft, Corner::TopRight]
        } else {
            vec![]
        };
    }
    let floor = if name.starts_with("floor-") || name == "slope" {
        let p = &mut t.panels[0];
        p.pattern.height = width;
        p.pattern.ny = resolution;
        p.placement.rotation_degrees.x = 90.;
        p.placement.translation = DVec3::new(-width / 2., 0.00305, width / 2.);
        let mut floor = if name == "floor-face" {
            SurfaceMesh {
                vertices: vec![
                    DVec3::new(-2., 0., -2.),
                    DVec3::new(2., 0., -2.),
                    DVec3::new(0., 0., 2.),
                ],
                triangles: vec![[0, 1, 2]],
            }
        } else {
            SurfaceMesh {
                vertices: vec![
                    DVec3::new(-2., 0., -2.),
                    DVec3::new(2., 0., -2.),
                    DVec3::new(2., 0., 2.),
                    DVec3::new(-2., 0., 2.),
                ],
                triangles: vec![[0, 1, 2], [0, 2, 3]],
            }
        };
        if name == "slope" {
            // No friction: this is a negative settling control, not a failure.
            let rotation = glam::DQuat::from_rotation_z(5_f64.to_radians());
            for p in &mut floor.vertices {
                // Keep the sliding control away from the finite support edge
                // even in the longest allowed run. Do not confuse falling off
                // the fixture with a tangential contact failure.
                *p = rotation * (*p * 100.);
            }
            p.placement.translation = rotation * p.placement.translation;
            // R = Rx*Ry*Rz: use Ry after Rx to rotate about world Z.
            p.placement.rotation_degrees.y = 5.;
        }
        Some(floor)
    } else {
        None
    };
    (t, floor)
}

fn write_json(
    path: impl AsRef<Path>,
    data: &impl Serialize,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(path, serde_json::to_vec_pretty(data)?)?;
    Ok(())
}

#[derive(Clone, Serialize)]
struct Observation {
    time_s: f64,
    phase: &'static str,
    mass_rms_speed_m_s: f64,
    max_speed_m_s: f64,
    highpass_rms_m: f64,
    physical_force_rms_n: f64,
    fresh_contact_force_jump_rms_n: f64,
    contact_refresh_j: f64,
    audit: marvelous_cloth::vbd::audit::StepEnergy,
    metrics: marvelous_cloth::vbd::VbdMetrics,
}

fn attachment(state: &State, vertices: [usize; 3]) -> DVec3 {
    vertices.iter().map(|&v| state.x[v] / 3.).sum()
}

fn stats(mut v: Vec<f64>) -> Value {
    v.sort_by(f64::total_cmp);
    json!({"mean": v.iter().sum::<f64>() / v.len() as f64,
        "p95": v[(v.len()-1)*95/100], "max": v.last().unwrap()})
}

fn run(
    solver: &mut VbdSolver,
    state: &mut State,
    settings: &VbdSettings,
    seconds: f64,
    pull: bool,
    path: &Path,
    control_anchor: Option<DVec3>,
) -> Result<Value, Box<dyn std::error::Error>> {
    fs::create_dir(path)?;
    write_json(path.join("initial-state.json"), state)?;
    let garment = solver.garment.clone();
    let moving: Vec<_> = (0..state.x.len())
        .filter(|&v| garment.mesh.pins[v].is_none())
        .collect();
    let total_mass: f64 = moving.iter().map(|&v| garment.mesh.masses[v]).sum();
    let vertices = garment.mesh.triangles.last().unwrap().vertices;
    let anchor = control_anchor.unwrap_or_else(|| attachment(state, vertices));
    write_json(
        path.join("control-anchor.json"),
        &json!({"active":pull,"position_m":anchor}),
    )?;
    let mut detector = solver.world.clone();
    let mut contacts = detector.detect(
        &detector.positions(&state.x),
        4. * settings.contact.radius,
        &settings.contact,
    )?;
    let mut log = std::io::BufWriter::new(fs::File::create(path.join("observations.jsonl"))?);
    let mut tail = VecDeque::<(State, Observation)>::new();
    let mut max_interaction_displacement = 0_f64;
    let mut max_balance_error = 0_f64;
    let mut min_distance = f64::INFINITY;
    let start_time = state.time;
    let mut all_costs = Vec::new();
    let mut step_fail = None;
    for step in 0..(seconds / settings.dt).round() as usize {
        let elapsed = step as f64 * settings.dt;
        let phase = if !pull {
            "rest"
        } else if elapsed < 1. {
            "pull"
        } else if elapsed < 1.5 {
            "hold"
        } else {
            "release"
        };
        let grab = if pull && elapsed < 1.5 {
            let a = ((elapsed + settings.dt) / 1.).min(1.);
            Some(Grab {
                patch: Vec::new(),
                vertices,
                weights: [1. / 3.; 3],
                // Includes upward and tangential motion on supported cases.
                target: anchor + DVec3::new(0.02, 0.025, 0.04) * a * a * (3. - 2. * a),
                stiffness: 100.,
            })
        } else {
            None
        };
        let old = state.clone();
        let metrics = match solver.step(state, settings, grab.as_ref()) {
            Ok(m) => m,
            Err(e) => {
                step_fail = Some(format!("Step {step}: {e}"));
                break;
            }
        };
        all_costs.push(metrics.step_ms);
        let audit = solver.audit_step(&old, state, settings, &contacts, grab.as_ref())?;
        let fresh = detector.detect(
            &detector.positions(&state.x),
            4. * settings.contact.radius,
            &settings.contact,
        )?;
        let energy = solver.audit_energy(state, settings, &fresh, grab.as_ref())?;
        let forces = solver.audit_vertex_forces(state, settings, &fresh, grab.as_ref(), &moving)?;
        let old_feature_forces =
            solver.audit_vertex_forces(state, settings, &contacts, grab.as_ref(), &moving)?;
        let rms = |sum: f64| (sum / moving.len() as f64).sqrt();
        let o = Observation {
            time_s: state.time,
            phase,
            mass_rms_speed_m_s: (moving
                .iter()
                .map(|&v| garment.mesh.masses[v] * state.v[v].length_squared())
                .sum::<f64>()
                / total_mass)
                .sqrt(),
            max_speed_m_s: moving
                .iter()
                .map(|&v| state.v[v].length())
                .fold(0., f64::max),
            highpass_rms_m: rms(moving
                .iter()
                .map(|&v| ((state.v[v] - old.v[v]) * settings.dt * 0.5).length_squared())
                .sum()),
            physical_force_rms_n: rms(forces.iter().map(|f| f.length_squared()).sum()),
            fresh_contact_force_jump_rms_n: rms(forces
                .iter()
                .zip(old_feature_forces)
                .map(|(a, b)| (*a - b).length_squared())
                .sum()),
            contact_refresh_j: energy.contact_j - audit.after.contact_j,
            audit,
            metrics,
        };
        max_balance_error = max_balance_error.max(o.audit.balance_error_j.abs());
        min_distance = min_distance.min(fresh.minimum_distance);
        if pull && elapsed < 1.5 {
            max_interaction_displacement =
                max_interaction_displacement.max(attachment(state, vertices).distance(anchor));
        }
        serde_json::to_writer(&mut log, &o)?;
        writeln!(log)?;
        tail.push_back((state.clone(), o));
        while tail.len() > (WINDOW / settings.dt).round() as usize + 1 {
            tail.pop_front();
        }
        contacts = fresh;
    }
    log.flush()?;
    let intersection_check = detector.validate_start(&detector.positions(&state.x));
    write_json(path.join("final-state.json"), state)?;
    if tail.len() < 2 {
        return Err(step_fail
            .unwrap_or("No accepted observations".into())
            .into());
    }
    let mut max_span = 0_f64;
    let mut max_net = 0_f64;
    for &v in &moving {
        let mut lo = DVec3::splat(f64::INFINITY);
        let mut hi = DVec3::splat(f64::NEG_INFINITY);
        for (s, _) in &tail {
            lo = lo.min(s.x[v]);
            hi = hi.max(s.x[v]);
        }
        max_span = max_span.max((hi - lo).length());
        max_net = max_net.max(tail.back().unwrap().0.x[v].distance(tail.front().unwrap().0.x[v]));
    }
    let max_speed = tail.iter().map(|(_, o)| o.max_speed_m_s).fold(0., f64::max);
    let full_window = state.time - start_time >= WINDOW;
    let weight_rms = (moving
        .iter()
        .map(|&v| (garment.mesh.masses[v] * settings.gravity.length()).powi(2))
        .sum::<f64>()
        / moving.len() as f64)
        .sqrt();
    let force_threshold = (0.01 * weight_rms).max(1e-9);
    let unbalanced = tail
        .iter()
        .any(|(_, o)| o.physical_force_rms_n > force_threshold);
    let final_forces = solver.audit_vertex_forces(state, settings, &contacts, None, &moving)?;
    write_json(path.join("final-vertices.json"), &moving.iter().zip(final_forces).map(|(&v, force)|
        json!({"vertex":v,"force_n":force,"speed_m_s":state.v[v].length(),"radius_m":contacts.radii[v],
            "position_m":state.x[v]})).collect::<Vec<_>>())?;
    let summary = json!({
        "completed":step_fail.is_none(), "failure": step_fail,
        "fresh_final_intersection_check": intersection_check.is_ok(),
        "intersection_error": intersection_check.err(),
        "classification": classify(full_window, max_span, max_speed, unbalanced),
        "unbalanced_force_threshold_n":force_threshold,
        "equilibrium_certified": false,
        "observed_seconds": state.time-start_time,
        "tail_seconds": tail.back().unwrap().0.time-tail.front().unwrap().0.time,
        "tail_max_vertex_aabb_diagonal_m":max_span, "tail_max_net_displacement_m":max_net,
        "tail_max_speed_m_s":max_speed,
        "tail_mass_rms_speed_m_s":stats(tail.iter().map(|(_,o)|o.mass_rms_speed_m_s).collect()),
        "tail_highpass_rms_m":stats(tail.iter().map(|(_,o)|o.highpass_rms_m).collect()),
        "tail_physical_force_rms_n":stats(tail.iter().map(|(_,o)|o.physical_force_rms_n).collect()),
        "tail_implicit_residual_rms_n":stats(tail.iter().map(|(_,o)|o.audit.residual_rms_n).collect()),
        "tail_contact_force_jump_rms_n":stats(tail.iter().map(|(_,o)|o.fresh_contact_force_jump_rms_n).collect()),
        "tail_abs_contact_refresh_j":stats(tail.iter().map(|(_,o)|o.contact_refresh_j.abs()).collect()),
        "tail_positive_fixed_energy_steps":tail.iter().filter(|(_,o)|o.audit.delta_mechanical_j>1e-12).count(),
        "tail_truncation_events":tail.iter().map(|(_,o)|o.metrics.truncated_vertices).sum::<usize>(),
        "tail_step_ms":stats(tail.iter().map(|(_,o)|o.metrics.step_ms).collect()),
        "all_step_ms":stats(all_costs),
        "peak_grab_displacement_m":max_interaction_displacement,
        "minimum_fresh_separation_m":min_distance,
        "max_energy_identity_error_j":max_balance_error,
    });
    write_json(path.join("summary.json"), &summary)?;
    Ok(summary)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if !args.settle_seconds.is_finite()
        || !(2.0..=120.).contains(&args.settle_seconds)
        || !args.continuation_seconds.is_finite()
        || !(4.0..=120.).contains(&args.continuation_seconds)
        || !(2..=12).contains(&args.resolution)
        || !(10..=32).contains(&args.backtracks)
        || args.cases.iter().any(|s| !CASES.contains(&s.as_str()))
    {
        return Err("Invalid duration, resolution or fixture name".into());
    }
    let settings = VbdSettings {
        dt: DT,
        iterations: args.iterations,
        ..Default::default()
    };
    settings.validate()?;
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build_global()?;
    // Refuse overwriting evidence, including a failed previous run.
    fs::create_dir(&args.output)?;
    write_json(
        args.output.join("manifest.json"),
        &json!({"args":args,"settings":settings,
        "backend":"CPU f64 VBD/OGC", "threads":2,
        "quiet_thresholds":{"window_s":WINDOW,"max_vertex_span_m":QUIET_SPAN,"max_speed_m_s":QUIET_SPEED},
        "restart_scope":"cold rest rebuilds all solver caches; fresh-contacts replaces only the contact world; continuous rest and pull clone all history",
        "energy_scope":"fixed controls within each step; contact refresh separate; do not sum across changing grabs"}),
    )?;
    let mut summaries = serde_json::Map::new();
    for &name in CASES {
        if !args.cases.is_empty() && !args.cases.iter().any(|s| s == name) {
            continue;
        }
        let path = args.output.join(name);
        fs::create_dir(&path)?;
        let (template, floor) = fixture(name, args.seed, args.resolution);
        write_json(path.join("template.json"), &template)?;
        if let Some(floor) = &floor {
            write_json(
                path.join("support.json"),
                &json!({"vertices":floor.vertices,"triangles":floor.triangles}),
            )?;
        }
        let garment = Arc::new(template.compile()?);
        let mut state = garment.initial_state.clone();
        let mut solver = VbdSolver::new(garment.clone(), floor.as_ref(), &state)?;
        solver.set_local_solve_probe(args.unshifted_spd, args.backtracks)?;
        let mut settings = settings.clone();
        if name == "zero-gravity" {
            settings.gravity = DVec3::ZERO;
        }
        write_json(path.join("settings.json"), &settings)?;
        eprintln!("{name}: {} vertices, settling", state.x.len());
        let settled = run(
            &mut solver,
            &mut state,
            &settings,
            args.settle_seconds,
            false,
            &path.join("settle"),
            None,
        )?;
        let mut modes = serde_json::Map::new();
        modes.insert("settle".into(), settled);
        for mode in [
            "continuous",
            "cold-restart",
            "fresh-contacts",
            "pull-release",
        ] {
            eprintln!("{name}: {mode}");
            let mut branch = if mode == "cold-restart" {
                VbdSolver::new(garment.clone(), floor.as_ref(), &state)?
            } else {
                solver.clone()
            };
            if mode == "fresh-contacts" {
                branch.world = VbdSolver::new(garment.clone(), floor.as_ref(), &state)?.world;
            }
            branch.set_local_solve_probe(args.unshifted_spd, args.backtracks)?;
            let control_anchor = if mode == "pull-release" {
                if let Some(root) = &args.controls_from {
                    let source_template: Value =
                        serde_json::from_slice(&fs::read(root.join(name).join("template.json"))?)?;
                    if source_template != serde_json::to_value(&template)? {
                        return Err(
                            "Control source template does not match this seeded fixture".into()
                        );
                    }
                    if let Some(surface) = &floor {
                        let source_support: Value = serde_json::from_slice(&fs::read(
                            root.join(name).join("support.json"),
                        )?)?;
                        if source_support
                            != json!({"vertices":surface.vertices,"triangles":surface.triangles})
                        {
                            return Err("Control source support does not match this fixture".into());
                        }
                    }
                    let source: State = serde_json::from_slice(&fs::read(
                        root.join(name).join("pull-release/initial-state.json"),
                    )?)?;
                    if source.x.len() != state.x.len() || source.x.iter().any(|p| !p.is_finite()) {
                        return Err("Invalid control source state".into());
                    }
                    Some(attachment(
                        &source,
                        garment.mesh.triangles.last().unwrap().vertices,
                    ))
                } else {
                    None
                }
            } else {
                None
            };
            let result = run(
                &mut branch,
                &mut state.clone(),
                &settings,
                args.continuation_seconds,
                mode == "pull-release",
                &path.join(mode),
                control_anchor,
            )?;
            modes.insert(mode.into(), result);
        }
        summaries.insert(name.into(), Value::Object(modes));
        write_json(args.output.join("summary.json"), &summaries)?;
    }
    if summaries.values().any(|v| {
        v.as_object()
            .unwrap()
            .values()
            .any(|r| r["completed"] != true || r["fresh_final_intersection_check"] != true)
    }) {
        return Err("One or more zoo runs failed; inspect summary.json".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_gate_rejects_drift_fast_small_oscillations_and_unbalanced_rest() {
        assert_eq!(classify(false, 0., 0., false), "insufficient-observation");
        assert_eq!(classify(true, 2. * QUIET_SPAN, 0., false), "still-moving");
        assert_eq!(classify(true, 0., 2. * QUIET_SPEED, false), "still-moving");
        assert_eq!(classify(true, 0., 0., true), "quiet-with-unbalanced-forces");
        assert_eq!(classify(true, 0., 0., false), "quiet-observed");
    }

    #[test]
    fn generated_fixtures_are_separated_and_sloped_support_is_parallel() {
        for seed in [0, 1, 7] {
            for &name in CASES {
                let (t, floor) = fixture(name, seed, 3);
                let g = Arc::new(t.compile().unwrap());
                VbdSolver::new(g.clone(), floor.as_ref(), &g.initial_state).unwrap();
                if name == "slope" {
                    let floor = floor.unwrap();
                    let normal = (floor.vertices[1] - floor.vertices[0])
                        .cross(floor.vertices[2] - floor.vertices[0])
                        .normalize();
                    for p in &g.initial_state.x {
                        assert!((normal.dot(*p - floor.vertices[0]).abs() - 0.00305).abs() < 1e-12);
                    }
                }
            }
        }
    }
}

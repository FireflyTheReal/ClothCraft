//! CPU-only, opt-in energy audit and scripted pull/hold/release. Not a benchmark
//! of interactive performance. Never modifies the input document or executable.
use clap::{Parser, ValueEnum};
use glam::DVec3;
use marvelous_cloth::{
    fitting::FittingProject,
    garment::{GarmentTemplate, template::Corner},
    model::State,
    vbd::{Grab, VbdSettings, VbdSolver},
};
use serde_json::json;
use std::{
    fs,
    io::{BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
    sync::Arc,
};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Fixture {
    Sewn,
    Free,
}
#[derive(Parser)]
struct Args {
    #[arg(long, requires = "trace")]
    project: Option<PathBuf>,
    /// Replay previously accepted states on CPU; does not run their GPU solver.
    #[arg(long, requires = "project")]
    trace: Option<PathBuf>,
    #[arg(long, default_value = "output/energy-audit")]
    output: PathBuf,
    #[arg(long, default_value_t = 360)]
    steps: usize,
    #[arg(long, conflicts_with = "trace", default_value_t = 16)]
    iterations: usize,
    #[arg(long, value_enum, default_value_t = Fixture::Sewn)]
    fixture: Fixture,
    #[arg(long, default_value_t = 1.15)]
    ease_ratio: f64,
}

fn fixture(args: &Args) -> Result<GarmentTemplate, String> {
    if !args.ease_ratio.is_finite() || !(1.0..=1.5).contains(&args.ease_ratio) {
        return Err("Ease ratio must lie in [1, 1.5]".into());
    }
    let mut t = GarmentTemplate::two_panel_demo();
    t.name = "Energy audit / unequal sewing / pull and release".into();
    for (i, p) in t.panels.iter_mut().enumerate() {
        p.pattern.width = 0.15;
        p.pattern.height = 0.30 * if i == 1 { args.ease_ratio } else { 1. };
        p.pattern.nx = 3;
        p.pattern.ny = 4;
        p.placement.rotation_degrees = DVec3::ZERO;
        p.placement.translation = if i == 0 {
            DVec3::new(-0.15, 1., 0.)
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
    if matches!(args.fixture, Fixture::Free) {
        t.panels.truncate(1);
        t.panels[0].pins.clear();
        t.seams.clear();
    }
    Ok(t)
}

fn attachment(state: &State, vertices: [usize; 3]) -> DVec3 {
    vertices.iter().map(|&i| state.x[i] / 3.).sum()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build_global()?;
    fs::create_dir_all(&args.output)?;
    let (garment, mut state, avatar, settings) = if let Some(path) = &args.project {
        let p = FittingProject::load(path)?;
        let scene = p.compile()?;
        (
            scene.garment,
            scene.state,
            Some(scene.collider.surface.clone()),
            p.simulation.unwrap_or(VbdSettings {
                dt: 1. / 60.,
                ..Default::default()
            }),
        )
    } else {
        let t = fixture(&args)?;
        fs::write(
            args.output.join("fixture.json"),
            serde_json::to_vec_pretty(&t)?,
        )?;
        let g = Arc::new(t.compile()?);
        (
            g.clone(),
            g.initial_state.clone(),
            None,
            VbdSettings {
                dt: 1. / 60.,
                iterations: args.iterations,
                ..Default::default()
            },
        )
    };
    settings.validate()?;
    let mut solver = VbdSolver::new(garment.clone(), avatar.as_deref(), &state)?;
    // A separate detector cannot perturb the simulation's temporal caches.
    let mut world = solver.world.clone();
    let mut replay = args
        .trace
        .as_ref()
        .map(|p| fs::File::open(p).map(|f| BufReader::new(f).lines()))
        .transpose()?;
    if let Some(lines) = &mut replay {
        let initial: State = serde_json::from_str(&lines.next().ok_or("Empty trace")??)?;
        if initial.x != state.x || initial.v != state.v || initial.time != state.time {
            return Err("Trace initial state does not match the project".into());
        }
    }
    let vertices =
        garment.mesh.triangles[garment.panels.last().unwrap().triangles.end - 1].vertices;
    let mut grab_start = DVec3::ZERO;
    let mut previous_grab: Option<Grab> = None;
    let mut records = BufWriter::new(fs::File::create(args.output.join("energy.jsonl"))?);
    let mut rows = Vec::new();
    for step in 0..args.steps {
        let replayed = if let Some(lines) = &mut replay {
            let Some(line) = lines.next() else { break };
            Some(serde_json::from_str::<State>(&line?)?)
        } else {
            None
        };
        let phase = if replay.is_some() {
            "archived"
        } else if step < 120 {
            "settle"
        } else if step < 180 {
            "pull"
        } else if step < 210 {
            "hold"
        } else {
            "release"
        };
        if step == 120 {
            grab_start = attachment(&state, vertices);
        }
        let grab = if replay.is_none() && (120..210).contains(&step) {
            let a = ((step - 119) as f64 / 60.).min(1.);
            Some(Grab {
                patch: Vec::new(),
                vertices,
                weights: [1. / 3.; 3],
                target: grab_start + DVec3::Z * 0.04 * a * a * (3. - 2. * a),
                stiffness: 100.,
            })
        } else {
            None
        };
        let old = state.clone();
        let contacts = world.detect(
            &world.positions(&old.x),
            4. * settings.contact.radius,
            &settings.contact,
        )?;
        let old_control_energy = solver
            .audit_energy(&old, &settings, &contacts, previous_grab.as_ref())?
            .grab_j;
        let new_control_energy = solver
            .audit_energy(&old, &settings, &contacts, grab.as_ref())?
            .grab_j;
        let solver_metrics = if let Some(next) = replayed {
            state = next;
            None
        } else {
            Some(solver.step(&mut state, &settings, grab.as_ref())?)
        };
        let audit = solver.audit_step(&old, &state, &settings, &contacts, grab.as_ref())?;
        let refreshed = world.detect(
            &world.positions(&state.x),
            4. * settings.contact.radius,
            &settings.contact,
        )?;
        let current = solver.audit_energy(&state, &settings, &refreshed, grab.as_ref())?;
        let refresh = current.contact_j - audit.after.contact_j;
        let row = json!({
            "step": step, "time_s": state.time, "phase": phase,
            "audit": audit, "contact_refresh_j": refresh,
            "control_change_j": new_control_energy - old_control_energy,
            "mechanical_change_excluding_control_j": audit.delta_mechanical_j + refresh,
            "fresh_mechanical_energy_j": current.total_j(),
            "attachment_m": attachment(&state, vertices),
            "contact_terms": refreshed.contacts.len(), "minimum_distance_m": refreshed.minimum_distance,
            "solver_metrics": solver_metrics,
        });
        serde_json::to_writer(&mut records, &row)?;
        writeln!(records)?;
        rows.push(row);
        previous_grab = grab;
    }
    records.flush()?;
    let summary = json!({
        "mode": if replay.is_some() { "cpu_replay_of_accepted_states" } else { "cpu_pull_hold_release" },
        "cpu_workers": 2, "gpu_used": false, "settings": settings,
        "vertices": state.x.len(), "steps": rows.len(),
        "positive_change_steps": rows.iter().filter(|r| r["mechanical_change_excluding_control_j"].as_f64().unwrap() > 1e-12).count(),
        "max_accounting_error_j": rows.iter().map(|r| r["audit"]["balance_error_j"].as_f64().unwrap().abs()).fold(0., f64::max),
        "warning": "Diagnostic only. Fixed-feature accounting separates contact refresh and control changes; it is not a convergence or collision certificate. CPU replay does not reproduce GPU iterations.",
    });
    fs::write(
        args.output.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    fs::write(
        args.output.join("final-state.json"),
        serde_json::to_vec(&state)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

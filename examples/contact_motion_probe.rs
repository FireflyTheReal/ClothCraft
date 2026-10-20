//! CPU replay: contact churn/force jumps at motion hotspots and an interleaved
//! scalar/SIMD detector comparison. No simulation, GPU, or document writes.
use clap::Parser;
use glam::DVec3;
use marvelous_cloth::{
    fitting::FittingProject,
    model::State,
    ogc::{ContactSet, CpuContactKernel},
    vbd::{VbdSettings, VbdSolver},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    trace: PathBuf,
    #[arg(long, default_value = "output/cpu-contact-probe")]
    output: PathBuf,
    #[arg(long, default_value_t = 360)]
    steps: usize,
    #[arg(long, default_value_t = 60)]
    warmup: usize,
}
fn states(path: &Path) -> std::io::Result<impl Iterator<Item = Result<State, String>>> {
    Ok(BufReader::new(fs::File::open(path)?)
        .lines()
        .map(|s| serde_json::from_str(&s.map_err(|e| e.to_string())?).map_err(|e| e.to_string())))
}
fn equal(a: &ContactSet, b: &ContactSet) -> bool {
    let key = |c: &marvelous_cloth::ogc::potential::Contact| {
        (c.kind, c.vertices, c.count, c.mollifier_epsilon.to_bits())
    };
    a.contacts.iter().map(key).eq(b.contacts.iter().map(key))
        && a.radii
            .iter()
            .map(|x| x.to_bits())
            .eq(b.radii.iter().map(|x| x.to_bits()))
        && a.centers == b.centers
        && a.incident == b.incident
        && a.minimum_distance.to_bits() == b.minimum_distance.to_bits()
        && a.active_contacts == b.active_contacts
        && a.vertex_face_candidates == b.vertex_face_candidates
        && a.edge_edge_candidates == b.edge_edge_candidates
}
fn distribution(mut xs: Vec<f64>) -> Value {
    xs.sort_by(f64::total_cmp);
    json!({"mean": xs.iter().sum::<f64>() / xs.len() as f64,
        "p50": xs[xs.len()/2], "p95": xs[(xs.len()-1)*95/100]})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.steps <= args.warmup {
        return Err("steps must exceed warmup".into());
    }
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build_global()?;
    let project = FittingProject::load(&args.project)?;
    let scene = project.compile()?;
    let settings = project.simulation.unwrap_or(VbdSettings {
        dt: 1. / 60.,
        ..Default::default()
    });
    settings.validate()?;
    let g = &scene.garment;
    let mut previous = scene.state.clone();
    let n = previous.x.len();
    let mut hp = vec![0.; n];
    let mut reversals = vec![0_usize; n];
    let mut samples = 0;
    for (i, state) in states(&args.trace)?.take(args.steps + 1).enumerate() {
        let state = state?;
        if state.x.len() != n
            || state.v.len() != n
            || state.x.iter().chain(&state.v).any(|p| !p.is_finite())
        {
            return Err("Invalid trace dimensions/positions/velocities".into());
        }
        if i == 0 {
            if state.x != previous.x || state.v != previous.v || state.time != previous.time {
                return Err("Trace initial state differs from the compiled project".into());
            }
        } else {
            if (state.time - previous.time - settings.dt).abs() > settings.dt * 1e-6 {
                return Err("Trace is not consecutive at the saved timestep".into());
            }
            if i > args.warmup {
                samples += 1;
                for v in 0..n {
                    hp[v] += ((state.v[v] - previous.v[v]) * (0.5 * settings.dt)).length_squared();
                    reversals[v] += usize::from(state.v[v].dot(previous.v[v]) < 0.);
                }
            }
        }
        previous = state;
    }
    if samples == 0 {
        return Err("Trace ends before the analysis window".into());
    }
    let mut ranking: Vec<_> = (0..n).filter(|&v| g.mesh.pins[v].is_none()).collect();
    ranking.sort_by(|&a, &b| hp[b].total_cmp(&hp[a]));
    ranking.truncate(12);
    let describe = |v: usize| {
        json!({
            "panel": g.panels.iter().find(|p| p.vertices.contains(&v)).unwrap().id,
            "material_point_m": g.mesh.uv[v],
            "highpass_rms_m": (hp[v]/samples as f64).sqrt(),
            "reversal_fraction": reversals[v] as f64/samples as f64,
            "seams": g.seams.iter().filter(|s| s.samples.iter().any(|q| q.point.terms.iter().any(|&(i,_)| i==v))).map(|s| &s.id).collect::<Vec<_>>()
        })
    };
    let solver = VbdSolver::new(g.clone(), Some(&scene.collider.surface), &scene.state)?;
    let mut scalar = solver.world.clone();
    scalar.cpu_kernel = CpuContactKernel::Scalar;
    let mut vector = solver.world.clone();
    vector.cpu_kernel = CpuContactKernel::Auto;
    fs::create_dir_all(&args.output)?;
    let mut file = BufWriter::new(fs::File::create(args.output.join("contacts.jsonl"))?);
    let mut old_contacts: Option<ContactSet> = None;
    let mut old_keys = vec![BTreeSet::new(); ranking.len()];
    let mut old_forces = vec![DVec3::ZERO; ranking.len()];
    let mut scalar_ms = Vec::new();
    let mut vector_ms = Vec::new();
    let mut scalar_geometry = Vec::new();
    let mut vector_geometry = Vec::new();
    let mut checked = 0;
    let mut previous_state = scene.state.clone();
    for (step, state) in states(&args.trace)?.take(args.steps + 1).enumerate() {
        let state = state?;
        let x = vector.positions(&state.x);
        // Four radii cover the solver's query range. This is an offline
        // detection, not a reconstruction of in-step GPU active sets.
        let rq = 4. * settings.contact.radius;
        let mut detect = |which: bool| -> Result<_, String> {
            let now = Instant::now();
            let c = if which {
                vector.detect(&x, rq, &settings.contact)?
            } else {
                scalar.detect(&x, rq, &settings.contact)?
            };
            Ok((c, now.elapsed().as_secs_f64() * 1000.))
        };
        let ((a, ta), (b, tb)) = if step % 2 == 0 {
            (detect(false)?, detect(true)?)
        } else {
            let b = detect(true)?;
            (detect(false)?, b)
        };
        if !equal(&a, &b) {
            return Err(format!("Scalar/SIMD contact mismatch at state {step}").into());
        }
        checked += 1;
        if step > args.warmup {
            scalar_ms.push(ta);
            vector_ms.push(tb);
            scalar_geometry.push(a.profile.geometry_ms);
            vector_geometry.push(b.profile.geometry_ms);
        }
        let mut seam_force = vec![DVec3::ZERO; n];
        for seam in &g.seams {
            for q in &seam.samples {
                let force = -seam.stiffness * q.weight_m * q.point.gap(&state.x);
                for &(v, w) in &q.point.terms {
                    seam_force[v] += w * force;
                }
            }
        }
        let mut hotspots = Vec::new();
        let physical_forces = solver.audit_vertex_forces(&state, &settings, &a, None, &ranking)?;
        for (h, &v) in ranking.iter().enumerate() {
            let mut force = DVec3::ZERO;
            let mut keys = BTreeSet::new();
            for &i in &a.incident[v] {
                let c = &a.contacts[i];
                if c.value(&x, settings.contact.radius, settings.contact.stiffness) > 0. {
                    keys.insert((c.kind, c.vertices));
                    force -= c
                        .block(&x, v, settings.contact.radius, settings.contact.stiffness)
                        .1;
                }
            }
            let frozen_force = old_contacts.as_ref().map(|old| {
                old.incident[v]
                    .iter()
                    .map(|&i| {
                        -old.contacts[i]
                            .block(&x, v, settings.contact.radius, settings.contact.stiffness)
                            .1
                    })
                    .sum::<DVec3>()
            });
            hotspots.push(json!({
                "hotspot": h, "speed_m_s": state.v[v].length(), "ball_radius_m": a.radii[v],
                "active_terms": keys.len(), "active_keys_changed": keys.symmetric_difference(&old_keys[h]).count(),
                "contact_force_n": force, "sewing_force_n": seam_force[v],
                "contact_force_step_delta_n": (force-old_forces[h]).length(),
                "contact_refresh_force_delta_n": frozen_force.filter(|f| f.is_finite()).map(|f| (force-f).length()),
                "fresh_physical_force_n": physical_forces[h],
                "fresh_balance_residual_n": if step == 0 { None } else { Some((g.mesh.masses[v] * ((state.v[v]-previous_state.v[v])/settings.dt + settings.damping*state.v[v]) - physical_forces[h]).length()) },
                "highpass_m": ((state.v[v]-previous_state.v[v])*(settings.dt*0.5)).length(),
            }));
            old_keys[h] = keys;
            old_forces[h] = force;
        }
        let row = json!({"step":step, "time_s":state.time, "scalar_ms":ta, "simd_ms":tb,
            "scalar_profile":a.profile, "simd_profile":b.profile,
            "contacts":a.contacts.len(), "active_contacts":a.active_contacts,
            "minimum_distance_m":a.minimum_distance, "hotspots":hotspots,
            "seams":g.seams.iter().map(|s| json!({"id":s.id,"energy_j":s.energy(&state.x),"max_gap_m":s.max_gap(&state.x)})).collect::<Vec<_>>()});
        serde_json::to_writer(&mut file, &row)?;
        writeln!(file)?;
        old_contacts = Some(a);
        previous_state = state;
    }
    file.flush()?;
    let summary = json!({"mode":"cpu_replay_and_contact_comparison", "gpu_used":false, "cpu_workers":2,
        "avx_used":CpuContactKernel::Auto.uses_avx(), "states_compared_bitwise":checked, "warmup":args.warmup,
        "vertices":n, "scalar_detector_ms":distribution(scalar_ms), "simd_detector_ms":distribution(vector_ms),
        "scalar_geometry_ms":distribution(scalar_geometry), "simd_geometry_ms":distribution(vector_geometry),
        "hotspots":ranking.into_iter().map(describe).collect::<Vec<_>>(),
        "seams":g.seams.iter().map(|s| json!({"id":s.id,"rest_lengths_m":s.lengths,"stiffness_n_m2":s.stiffness})).collect::<Vec<_>>(),
        "limits":"Archived accepted states, fresh CPU contact diagnostics. Highpass motion/churn/force jumps are correlations, not proof of a cause. Bounds are fresh snapshots, not actual in-step clipping. No GPU or whole-step performance claim."});
    fs::write(
        args.output.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

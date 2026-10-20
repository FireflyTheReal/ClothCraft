//! Matched-control baseline/block-refinement experiment. CPU by default; GPU
//! explicitly opt-in. Private input geometry and simulation settings are kept.
use clap::Parser;
use glam::{DVec2, DVec3};
use marvelous_cloth::{
    fitting::FittingProject,
    garment::{GarmentTemplate, template::Corner},
    model::State,
    vbd::{Grab, VbdSettings, VbdSolver},
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
    sync::Arc,
};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    project: Option<PathBuf>,
    #[arg(long, default_value = "output/seam-block-probe")]
    output: PathBuf,
    #[arg(long, default_value_t = 360)]
    steps: usize,
    #[arg(long, default_value_t = 1)]
    sweeps: usize,
    #[arg(long)]
    iterations: Option<usize>,
    #[arg(long)]
    reference_iterations: Option<usize>,
    #[arg(long)]
    gpu: bool,
    /// Replay a single production solver, without the coupled comparison.
    #[arg(long, conflicts_with = "reference_iterations")]
    baseline_only: bool,
    /// Experimental sparse separating planes in the production solve.
    #[arg(long)]
    directional_bounds: bool,
    /// Replay the exact world-space targets from another run's controls.jsonl.
    #[arg(long)]
    controls_from: Option<PathBuf>,
    /// Compare GPU candidate filtering with fresh f64 CPU detection each step.
    /// Validation only: these timings are not performance measurements.
    #[arg(long, requires = "gpu")]
    check_gpu_contacts: bool,
    #[arg(long)]
    rest_only: bool,
    #[arg(long, default_value_t = 1.15)]
    ease_ratio: f64,
    #[arg(long)]
    nonmatching: bool,
    #[arg(long)]
    free: bool,
    #[arg(long)]
    panel: Option<String>,
    #[arg(long, value_delimiter = ',', num_args = 2)]
    point: Option<Vec<f64>>,
    #[arg(long, default_value_t = 0.04)]
    pull_m: f64,
}
fn attachment(s: &State, vertices: [usize; 3]) -> DVec3 {
    vertices.iter().map(|&v| s.x[v] / 3.).sum()
}
fn stats(mut values: Vec<f64>) -> Value {
    values.sort_by(f64::total_cmp);
    if values.is_empty() {
        return Value::Null;
    }
    json!({"mean":values.iter().sum::<f64>()/values.len() as f64,"p95":values[(values.len()-1)*95/100],"max":values.last()})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.steps == 0
        || !args.ease_ratio.is_finite()
        || !(1.0..=1.5).contains(&args.ease_ratio)
        || !args.pull_m.is_finite()
    {
        return Err("Invalid experiment parameters".into());
    }
    #[derive(serde::Deserialize)]
    struct Control {
        step: usize,
        phase: String,
        target_m: Option<DVec3>,
    }
    let replay: Option<Vec<Control>> = args
        .controls_from
        .as_ref()
        .map(|path| -> Result<_, Box<dyn std::error::Error>> {
            let rows: Vec<Control> = BufReader::new(fs::File::open(path)?)
                .lines()
                .map(|line| -> Result<_, Box<dyn std::error::Error>> {
                    Ok(serde_json::from_str(&line?)?)
                })
                .collect::<Result<_, _>>()?;
            if rows.len() != args.steps
                || rows
                    .iter()
                    .enumerate()
                    .any(|(i, r)| r.step != i || r.target_m.is_some_and(|p| !p.is_finite()))
            {
                return Err(
                    "Control replay must have one finite, consecutively numbered row per step"
                        .into(),
                );
            }
            Ok(rows)
        })
        .transpose()?;
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build_global()?;
    let (garment, initial, avatar, mut settings) = if let Some(path) = &args.project {
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
        let mut t = GarmentTemplate::two_panel_demo();
        for (i, p) in t.panels.iter_mut().enumerate() {
            p.pattern.width = 0.15;
            p.pattern.height = 0.3 * if i == 0 { 1. } else { args.ease_ratio };
            p.pattern.nx = 3;
            p.pattern.ny = if args.nonmatching && i == 1 { 6 } else { 4 };
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
        if args.free {
            t.seams.clear();
            t.panels.truncate(1);
            t.panels[0].pins.clear();
        }
        let g = Arc::new(t.compile()?);
        (
            g.clone(),
            g.initial_state.clone(),
            None,
            VbdSettings {
                dt: 1. / 60.,
                iterations: 16,
                ..Default::default()
            },
        )
    };
    if let Some(i) = args.iterations {
        settings.iterations = i;
    }
    settings.validate()?;
    let panel = if let Some(id) = &args.panel {
        garment
            .panels
            .iter()
            .find(|p| &p.id == id)
            .ok_or("Unknown grab panel")?
    } else {
        garment.panels.last().ok_or("Empty garment")?
    };
    let desired = args.point.as_ref().map(|p| DVec2::new(p[0], p[1]));
    let triangle = if let Some(p) = desired {
        panel
            .triangles
            .clone()
            .min_by(|&a, &b| {
                let center = |t: usize| {
                    garment.mesh.triangles[t]
                        .vertices
                        .iter()
                        .map(|&i| garment.mesh.uv[i] / 3.)
                        .sum::<DVec2>()
                };
                center(a)
                    .distance_squared(p)
                    .total_cmp(&center(b).distance_squared(p))
            })
            .unwrap()
    } else {
        panel.triangles.end - 1
    };
    let vertices = garment.mesh.triangles[triangle].vertices;
    let mut solvers = vec![VbdSolver::new(
        garment.clone(),
        avatar.as_deref(),
        &initial,
    )?];
    let mut modes = vec!["baseline"];
    let mut configs = vec![settings.clone()];
    if !args.baseline_only {
        let mut coupled = VbdSolver::new(garment.clone(), avatar.as_deref(), &initial)?;
        coupled.set_seam_block_sweeps(args.sweeps)?;
        solvers.push(coupled);
        modes.push("coupled");
        configs.push(settings.clone());
    }
    if let Some(it) = args.reference_iterations {
        solvers.push(VbdSolver::new(
            garment.clone(),
            avatar.as_deref(),
            &initial,
        )?);
        let mut s = settings.clone();
        s.iterations = it;
        s.validate()?;
        configs.push(s);
        modes.push("reference");
    }
    for solver in &mut solvers {
        solver.world.directional_bounds = args.directional_bounds;
    }
    #[cfg(not(feature = "desktop"))]
    if args.gpu {
        return Err("Build with desktop for explicit --gpu".into());
    }
    #[cfg(feature = "desktop")]
    let gpu = if args.gpu {
        use eframe::wgpu;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))?;
        if adapter.get_info().device_type == wgpu::DeviceType::Cpu {
            return Err("Hardware GPU required".into());
        }
        eprintln!("{:?}", adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                required_features: adapter.features()
                    & (wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::SUBGROUP),
                ..Default::default()
            }))?;
        let mut gpu = marvelous_cloth::gpu_vbd::GpuVbd::new(device, queue);
        gpu.check_contacts = args.check_gpu_contacts;
        Some(gpu)
    } else {
        None
    };
    let mut states = vec![initial.clone(); modes.len()];
    let mut detectors: Vec<_> = solvers.iter().map(|s| s.world.clone()).collect();
    fs::create_dir_all(&args.output)?;
    let mut diagnostic = solvers[0].world.clone();
    diagnostic.directional_bounds = false;
    let initial_bounds = diagnostic.detect(
        &diagnostic.positions(&initial.x),
        settings.contact.radius * 2.,
        &settings.contact,
    )?;
    // Ephemeral indices only for these exact traces, never document references.
    fs::write(
        args.output.join("annotations.json"),
        serde_json::to_vec_pretty(&json!({
            "initial_isotropic_restricted_vertices": initial_bounds.radii.iter().enumerate().filter_map(|(i,r)| (*r < 1e-5).then_some(i)).collect::<Vec<_>>(),
            "vertices": garment.panels.iter().flat_map(|p| p.vertices.clone().map(|v| json!({"index":v,"panel":p.id,"material_point_m":garment.mesh.uv[v]}))).collect::<Vec<_>>(),
            "seams": garment.seams.iter().map(|s| json!({"id":s.id,"vertices":s.samples.iter().flat_map(|p|p.point.terms.iter().map(|&(v,_)|v)).collect::<std::collections::BTreeSet<_>>()})).collect::<Vec<_>>()
        }))?,
    )?;
    let mut traces = Vec::new();
    let mut metric_files = Vec::new();
    let mut energy_files = Vec::new();
    for mode in &modes {
        let path = args.output.join(mode);
        fs::create_dir_all(&path)?;
        let mut f = BufWriter::new(fs::File::create(path.join("trajectory.jsonl"))?);
        serde_json::to_writer(&mut f, &initial)?;
        writeln!(f)?;
        traces.push(f);
        metric_files.push(BufWriter::new(fs::File::create(
            path.join("metrics.jsonl"),
        )?));
        energy_files.push(BufWriter::new(fs::File::create(path.join("energy.jsonl"))?));
    }
    let mut controls = BufWriter::new(fs::File::create(args.output.join("controls.jsonl"))?);
    let mut measured: Vec<Vec<Value>> = vec![Vec::new(); modes.len()];
    let mut anchor = DVec3::ZERO;
    for step in 0..args.steps {
        if step == 120 {
            anchor = attachment(&states[0], vertices);
        }
        let phase = if args.rest_only || step < 120 {
            "settle"
        } else if step < 180 {
            "pull"
        } else if step < 210 {
            "hold"
        } else {
            "release"
        };
        let u = ((step as f64 - 119.) / 60.).clamp(0., 1.);
        let mut grab = (!args.rest_only && (120..210).contains(&step)).then_some(Grab {
            patch: Vec::new(),
            vertices,
            weights: [1. / 3.; 3],
            target: anchor + DVec3::Z * (args.pull_m * u * u * (3. - 2. * u)),
            stiffness: 100.,
        });
        if let Some(rows) = &replay {
            if rows[step].phase != phase || rows[step].target_m.is_some() != grab.is_some() {
                return Err("Control replay phase differs from this experiment".into());
            }
            if let Some(g) = &mut grab {
                g.target = rows[step].target_m.unwrap();
            }
        }
        serde_json::to_writer(
            &mut controls,
            &json!({"step":step,"phase":phase,"target_m":grab.as_ref().map(|g|g.target)}),
        )?;
        writeln!(controls)?;
        // Rotate timing order; all branches receive the identical target.
        for order in 0..modes.len() {
            let i = (order + step) % modes.len();
            #[cfg(feature = "desktop")]
            let m = if let Some(gpu) = &gpu {
                gpu.step(&mut solvers[i], &mut states[i], &configs[i], grab.as_ref())?
            } else {
                solvers[i].step(&mut states[i], &configs[i], grab.as_ref())?
            };
            #[cfg(not(feature = "desktop"))]
            let m = solvers[i].step(&mut states[i], &configs[i], grab.as_ref())?;
            let x = detectors[i].positions(&states[i].x);
            let c = detectors[i].detect(&x, settings.contact.radius * 4., &settings.contact)?;
            let energy = solvers[i].audit_energy(&states[i], &configs[i], &c, grab.as_ref())?;
            let areas: Vec<_> = garment
                .mesh
                .triangles
                .iter()
                .map(|t| {
                    let [a, b, c] = t.vertices.map(|v| states[i].x[v]);
                    (b - a).cross(c - a).length() / (2. * t.area)
                })
                .collect();
            let row = json!({"step":step,"phase":phase,"energy":energy,"total_j":energy.total_j(),"attachment_m":attachment(&states[i],vertices),"fresh_minimum_distance_m":c.minimum_distance,"minimum_area_ratio":areas.iter().copied().fold(f64::INFINITY,f64::min),"faces_below_one_percent_rest_area":areas.iter().filter(|a| **a < 0.01).count(),"step_ms":m.step_ms,"refinement_ms":m.seam_blocks.elapsed_ms,"highpass_rms_m":m.motion.as_ref().map(|m|m.highpass_rms_m)});
            serde_json::to_writer(&mut traces[i], &states[i])?;
            writeln!(&mut traces[i])?;
            serde_json::to_writer(&mut metric_files[i], &m)?;
            writeln!(&mut metric_files[i])?;
            serde_json::to_writer(&mut energy_files[i], &row)?;
            writeln!(&mut energy_files[i])?;
            measured[i].push(row);
        }
        if step % 60 == 0 {
            eprintln!("{step}/{} complete", args.steps);
        }
    }
    for f in traces
        .iter_mut()
        .chain(&mut metric_files)
        .chain(&mut energy_files)
    {
        f.flush()?;
    }
    controls.flush()?;
    // Fresh intersection check independently rebuilds the detector at the end.
    for (solver, state) in solvers.iter().zip(&states) {
        let mut world = solver.world.clone();
        world.validate_start(&world.positions(&state.x))?;
    }
    let summary = json!({"method":if args.baseline_only { "single-solver replay" } else { "sewing components with coupled membrane Gauss-Newton curvature" }, "directional_bounds":args.directional_bounds,"controls_from":args.controls_from,"gpu_used":args.gpu,"gpu_contact_oracle":args.check_gpu_contacts,"cpu_workers":2,"vertices":initial.x.len(),"steps":args.steps,"sweeps":if args.baseline_only { 0 } else { args.sweeps },"settings":configs,"shared_control":true,
        "grab_panel":panel.id,"grab_material_point_m":vertices.iter().map(|&v|garment.mesh.uv[v]/3.).sum::<DVec2>(),
        "results":modes.iter().enumerate().map(|(i,name)| {
            let rows=&measured[i];let post=rows.iter().skip(60);
            json!({"mode":name,"step_ms":stats(post.clone().map(|r|r["step_ms"].as_f64().unwrap()).collect()),"refinement_ms":stats(post.clone().map(|r|r["refinement_ms"].as_f64().unwrap()).collect()),
                "minimum_distance_m":rows.iter().map(|r|r["fresh_minimum_distance_m"].as_f64().unwrap()).fold(f64::INFINITY,f64::min),
                "autonomous_positive_energy_steps":rows.windows(2).filter(|r|r[0]["phase"]==r[1]["phase"] && (r[1]["phase"]=="settle" || r[1]["phase"]=="release") && r[1]["total_j"].as_f64().unwrap()-r[0]["total_j"].as_f64().unwrap()>1e-12).count(),
                "final_motion":rows.last().unwrap()["highpass_rms_m"],"final_attachment_m":attachment(&states[i],vertices)})
        }).collect::<Vec<_>>(),"fresh_final_intersection_check":"passed","limits":"Extra refinement work changes finite-iteration convergence, not the constitutive model. Contact endpoint energies are diagnostics; full performance includes refinement but excludes offline audits."});
    fs::write(
        args.output.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

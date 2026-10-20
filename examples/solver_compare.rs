//! Reproducible CPU backend comparison. Rendering and auditing are outside step timings.
use clap::Parser;
use glam::DVec3;
use marvelous_cloth::{
    fitting::FittingProject,
    solver_lab::{Algorithm, Backend},
    vbd::Grab,
};
use serde_json::json;
use std::{fs, io::Write, path::PathBuf, time::Instant};
#[derive(Parser)]
struct Args {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, value_enum, default_value = "vbd")]
    backend: Algorithm,
    #[arg(long, default_value_t = 180)]
    steps: usize,
    #[arg(long, default_value_t = 16)]
    iterations: usize,
    /// Same world-space trajectory for every backend; start is the initial barycenter.
    #[arg(long)]
    pull: bool,
    #[arg(long, default_value_t = 30)]
    pull_start: usize,
    #[arg(long, default_value_t = 30)]
    pull_steps: usize,
    #[arg(long, default_value_t = 15)]
    hold_steps: usize,
    #[arg(long, default_value_t = 0)]
    triangle: usize,
    #[arg(long, default_value_t = 0.04)]
    pull_m: f64,
    #[arg(long)]
    export_lattice: bool,
    /// Audit an external backend's State JSONL using the original cloth geometry.
    #[arg(long)]
    audit_trace: Option<PathBuf>,
    /// Explicit opt-in for the existing hybrid VBD control, never for CPU contenders.
    #[arg(long)]
    gpu_control: bool,
    /// Record target-objective residual and stiffness coverage after every sweep.
    #[arg(long)]
    convergence: bool,
    /// Audit cloth surface intersections after every accepted timestep.
    #[arg(long)]
    audit_every_step: bool,
    /// Explicit integration experiment. Step/pull counts are in this timestep.
    #[arg(long)]
    dt: Option<f64>,
    /// CPU-only EE energy experiment; not enabled in the editor.
    #[arg(long)]
    interior_edges: bool,
    /// CPU finite-primitive contact experiment; changes the contact law.
    #[arg(long, conflicts_with = "interior_edges")]
    primitive_distances: bool,
    /// Export render-only geometry and every accepted position frame, outside timings.
    #[arg(long)]
    record_states: bool,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a = Args::parse();
    if a.gpu_control && (a.interior_edges || a.primitive_distances) {
        return Err("Experimental contact laws are CPU-only".into());
    }
    if a.gpu_control && !matches!(a.backend, Algorithm::Vbd) {
        return Err("GPU control is available only for the existing VBD adapter".into());
    }
    #[cfg(not(feature = "desktop"))]
    if a.gpu_control {
        return Err("Build with desktop features for --gpu-control".into());
    }
    if a.steps == 0 || a.pull_steps == 0 {
        return Err("Steps and pull steps must be positive".into());
    }
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build_global()?;
    if a.output.exists() {
        return Err("Output already exists; choose a fresh directory".into());
    }
    let mut project = FittingProject::load(&a.project)?;
    let scene = project.compile()?;
    let mut state = scene.state;
    let mut settings = project.simulation.clone().unwrap_or_default();
    settings.iterations = a.iterations;
    if let Some(dt) = a.dt {
        settings.dt = dt;
    }
    settings.validate()?;
    let garment = scene.garment;
    let vertices = garment
        .mesh
        .triangles
        .get(a.triangle)
        .ok_or("Invalid grab triangle")?
        .vertices;
    let anchor = vertices.iter().map(|i| state.x[*i]).sum::<DVec3>() / 3.;
    fs::create_dir_all(&a.output)?;
    project.avatar_path = marvelous_cloth::document::portable_avatar(
        &project.avatar_path,
        &a.output.join("source.json"),
    )?;
    fs::write(
        a.output.join("source.json"),
        serde_json::to_vec_pretty(&project)?,
    )?;
    fs::write(
        a.output.join("initial-state.json"),
        serde_json::to_vec(&state)?,
    )?;
    if let Some(trace) = &a.audit_trace {
        use std::io::BufRead;
        let mut detector = Backend::new(
            Algorithm::Vbd,
            garment.clone(),
            Some(&scene.collider.surface),
            &state,
        )?;
        detector.reference.world.experimental_interior_edges = a.interior_edges;
        detector.reference.world.experimental_primitive_distances = a.primitive_distances;
        let mut log = fs::File::create(a.output.join("audit.jsonl"))?;
        let mut first_intersection = None;
        let mut count = 0;
        let mut last = json!(null);
        for line in std::io::BufReader::new(fs::File::open(trace)?).lines() {
            state = serde_json::from_str(&line?)?;
            if state.x.len() != garment.mesh.masses.len()
                || state.x.iter().chain(&state.v).any(|p| !p.is_finite())
            {
                return Err("Invalid external state".into());
            }
            let x = detector.reference.world.positions(&state.x);
            let intersections = detector.reference.world.validate_start(&x);
            if intersections.is_err() && first_intersection.is_none() {
                first_intersection = Some(count);
            }
            let c = detector.reference.world.detect(
                &x,
                settings.contact.radius * 2.,
                &settings.contact,
            )?;
            let [p95, max] =
                marvelous_cloth::garment::diagnostics::extension_percent(&garment, &state);
            last = json!({"step":count,"intersection_check":intersections,"minimum_distance_m":c.minimum_distance,"max_seam_gap_m":garment.max_seam_gap(&state.x),"extension_p95_percent":p95,"extension_max_percent":max});
            writeln!(log, "{}", last)?;
            count += 1;
        }
        fs::write(
            a.output.join("summary.json"),
            serde_json::to_vec_pretty(
                &json!({"audited_steps":count,"first_surface_intersection_step":first_intersection,"last":last}),
            )?,
        )?;
        return Ok(());
    }
    fs::write(
        a.output.join("protocol.json"),
        serde_json::to_vec_pretty(
            &json!({"backend":a.backend,"gpu":a.gpu_control,"experimental_interior_edges":a.interior_edges,"experimental_primitive_distances":a.primitive_distances,"settings":settings,"steps":a.steps,"threads":2,"pull":a.pull,"pull_start":a.pull_start,"pull_steps":a.pull_steps,"hold_steps":a.hold_steps,"pull_m":a.pull_m,"grab_vertices":vertices,"grab_anchor":anchor,"cold_restart":true,"warmup_steps":10,"source":a.project,"convergence_trace":a.convergence,"audit_every_step":a.audit_every_step,"vertices":state.x.len(),"triangles":garment.mesh.triangles.len(),"panels":garment.panels.iter().map(|p|json!({"id":p.id,"vertices":p.vertices.clone()})).collect::<Vec<_>>()}),
        )?,
    )?;
    if a.export_lattice {
        lattice::export(
            &a.output.join("lattice.txt"),
            &garment,
            &state,
            &scene.collider.surface,
            &settings,
        )?;
    }
    let mut solver = Backend::new(
        a.backend,
        garment.clone(),
        Some(&scene.collider.surface),
        &state,
    )?;
    solver.trace_convergence(a.convergence);
    if a.convergence && !a.gpu_control && matches!(a.backend, Algorithm::Vbd) {
        solver.reference.contact_refresh_audit = Some(Vec::new());
    }
    solver.reference.world.experimental_interior_edges = a.interior_edges;
    solver.reference.world.experimental_primitive_distances = a.primitive_distances;
    let mut trace = fs::File::create(a.output.join("convergence.jsonl"))?;
    #[cfg(feature = "desktop")]
    let gpu = if a.gpu_control {
        use eframe::wgpu;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))?;
        if adapter.get_info().device_type == wgpu::DeviceType::Cpu {
            return Err("A hardware GPU is required".into());
        }
        fs::write(
            a.output.join("gpu-adapter.txt"),
            format!("{:?}", adapter.get_info()),
        )?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                required_features: adapter.features()
                    & (wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::SUBGROUP),
                ..Default::default()
            }))?;
        Some(marvelous_cloth::gpu_vbd::GpuVbd::new(device, queue))
    } else {
        None
    };
    let mut detector = solver.reference.world.clone();
    let mut log = fs::File::create(a.output.join("steps.jsonl"))?;
    let mut costs = Vec::new();
    let mut positions = if a.record_states {
        fs::write(
            a.output.join("render-geometry.json"),
            serde_json::to_vec(&json!({
                "triangles": garment.mesh.triangles.iter().map(|t| t.vertices).collect::<Vec<_>>(),
                "panels": garment.panels.iter().map(|p| json!({"id":p.id,"vertices":p.vertices.clone()})).collect::<Vec<_>>(),
                "avatar_vertices": scene.collider.surface.vertices,
                "avatar_triangles": scene.collider.surface.triangles,
                "grab_vertices": vertices,
                "grab_anchor": anchor,
            }))?,
        )?;
        Some(std::io::BufWriter::new(fs::File::create(
            a.output.join("positions.jsonl"),
        )?))
    } else {
        None
    };
    let mut failure = None;
    let mut last = json!(null);
    let mass = garment.mesh.masses.iter().sum::<f64>();
    let mut previous_velocity = state.v.clone();
    for step in 0..a.steps {
        let release = a.pull_start + a.pull_steps + a.hold_steps;
        let grab = if a.pull && step >= a.pull_start && step < release {
            let fraction = ((step + 1 - a.pull_start) as f64 / a.pull_steps as f64).min(1.);
            Some(Grab {
                patch: Vec::new(),
                vertices,
                weights: [1. / 3.; 3],
                target: anchor + DVec3::Z * (a.pull_m * fraction),
                stiffness: 1000.,
            })
        } else {
            None
        };
        let phase = if !a.pull || step < a.pull_start {
            "quiet"
        } else if step < a.pull_start + a.pull_steps {
            "pull"
        } else if step < release {
            "hold"
        } else {
            "release"
        };
        let start = Instant::now();
        #[cfg(feature = "desktop")]
        let result = if let Some(gpu) = &gpu {
            gpu.step(&mut solver.reference, &mut state, &settings, grab.as_ref())
        } else {
            solver.step(&mut state, &settings, grab.as_ref())
        };
        #[cfg(not(feature = "desktop"))]
        let result = solver.step(&mut state, &settings, grab.as_ref());
        let m = match result {
            Ok(m) => m,
            Err(e) => {
                failure = Some(e);
                break;
            }
        };
        let elapsed = start.elapsed().as_secs_f64() * 1000.;
        let x = detector.positions(&state.x);
        let c = detector.detect(&x, settings.contact.radius * 2., &settings.contact)?;
        if a.audit_every_step
            && let Err(e) = detector.validate_start(&x)
        {
            failure = Some(format!(
                "Surface intersection after accepted step {step}: {e}"
            ));
        }
        if a.convergence {
            if let Some(event) = &solver.reference.contact_refresh_capture {
                fs::write(
                    a.output.join(format!("vbd-refresh-{step:05}.json")),
                    serde_json::to_vec(event)?,
                )?;
            }
            writeln!(
                trace,
                "{}",
                json!({"step":step,"sweeps":solver.trace,"vbd_contact_refresh_delta_j":solver.reference.contact_refresh_audit})
            )?;
            if let Some(event) = &solver.worst_refresh {
                fs::write(
                    a.output.join(format!("refresh-{step:05}.json")),
                    serde_json::to_vec(event)?,
                )?;
            }
        }
        let energy = solver
            .reference
            .audit_energy(&state, &settings, &c, grab.as_ref())?;
        let velocity = (state
            .v
            .iter()
            .zip(&garment.mesh.masses)
            .map(|(v, m)| m * v.length_squared())
            .sum::<f64>()
            / mass)
            .sqrt();
        let panel_speed:Vec<_>=garment.panels.iter().map(|p|{
            let ids=p.vertices.clone();let mass=ids.clone().map(|i|garment.mesh.masses[i]).sum::<f64>();
            json!({"id":p.id,"highpass_rms_m":(ids.clone().map(|i| ((state.v[i]-previous_velocity[i])*(0.5*settings.dt)).length_squared()).sum::<f64>()/ids.len() as f64).sqrt(),"mass_rms_speed_m_s":(ids.map(|i|garment.mesh.masses[i]*state.v[i].length_squared()).sum::<f64>()/mass).sqrt()})
        }).collect();
        last = json!({"step":step,"phase":phase,"full_step_ms":elapsed,"metrics":m,"fresh_minimum_distance_m":c.minimum_distance,"mass_rms_speed_m_s":velocity,"panel_speed":panel_speed,"energy":energy,"grab_position":vertices.iter().map(|i|state.x[*i]).sum::<DVec3>()/3.});
        previous_velocity.clone_from(&state.v);
        writeln!(log, "{}", last)?;
        if let Some(stream) = positions.as_mut() {
            serde_json::to_writer(
                &mut *stream,
                &json!({"step":step,"time":state.time,"x":state.x}),
            )?;
            writeln!(stream)?;
        }
        costs.push(elapsed);
        if failure.is_some() {
            break;
        }
        if step % 30 == 0 {
            eprintln!(
                "{:?}: {}/{} steps, {:.1} ms",
                a.backend,
                step + 1,
                a.steps,
                elapsed
            );
        }
    }
    let intersection = detector.validate_start(&detector.positions(&state.x));
    if let Some(stream) = positions.as_mut() {
        stream.flush()?;
    }
    fs::write(
        a.output.join("final-state.json"),
        serde_json::to_vec(&state)?,
    )?;
    let mut measured: Vec<_> = costs.iter().skip(10).copied().collect();
    if measured.is_empty() {
        measured = costs.clone();
    }
    measured.sort_by(f64::total_cmp);
    let percentile = |f: f64| {
        measured
            .get(((measured.len().saturating_sub(1)) as f64 * f).ceil() as usize)
            .copied()
    };
    let report = json!({"backend":a.backend,"gpu":a.gpu_control,"accepted_steps":costs.len(),"requested_steps":a.steps,"failure":failure,"final_intersection_check":intersection,"median_ms":percentile(0.5),"p95_ms":percentile(0.95),"max_ms":percentile(1.),"mean_ms":if measured.is_empty(){None}else{Some(measured.iter().sum::<f64>()/measured.len()as f64)},"last":last});
    fs::write(
        a.output.join("summary.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", report);
    if failure.is_some() {
        return Err("Backend failed; evidence retained".into());
    }
    Ok(())
}

mod lattice {
    use super::*;
    pub fn export(
        path: &std::path::Path,
        g: &marvelous_cloth::garment::CompiledGarment,
        s: &marvelous_cloth::model::State,
        avatar: &marvelous_cloth::avatar::SurfaceMesh,
        settings: &marvelous_cloth::vbd::VbdSettings,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut edges = std::collections::BTreeMap::new();
        for t in &g.mesh.triangles {
            for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                let i = t.vertices[a].min(t.vertices[b]);
                let j = t.vertices[a].max(t.vertices[b]);
                let l = g.mesh.uv[i].distance(g.mesh.uv[j]);
                let m = &g.materials[t.material];
                let k = (m.warp + m.weft) * 0.5 * t.area / l.powi(2);
                let entry = edges.entry((i, j)).or_insert((l, 0.));
                entry.1 += k;
            }
        }
        // Deliberately approximate barycentric seams as pairwise positive-weight links.
        for seam in &g.seams {
            for sample in &seam.samples {
                for (i, wa) in &sample.point.terms {
                    for (j, wb) in &sample.point.terms {
                        if *wa > 0. && *wb < 0. {
                            let key = ((*i).min(*j), (*i).max(*j));
                            let e = edges.entry(key).or_insert((0., 0.));
                            e.1 += seam.stiffness * sample.weight_m * wa * (-wb);
                        }
                    }
                }
            }
        }
        let mut f = fs::File::create(path)?;
        writeln!(
            f,
            "{} {} {} {} {} {}",
            s.x.len(),
            edges.len(),
            avatar.vertices.len(),
            avatar.triangles.len(),
            settings.dt,
            settings.damping
        )?;
        writeln!(
            f,
            "{} {} {}",
            settings.gravity.x, settings.gravity.y, settings.gravity.z
        )?;
        for i in 0..s.x.len() {
            let p = s.x[i];
            let v = s.v[i];
            writeln!(
                f,
                "{} {} {} {} {} {} {} {}",
                p.x,
                p.y,
                p.z,
                v.x,
                v.y,
                v.z,
                g.mesh.masses[i],
                usize::from(g.mesh.pins[i].is_some())
            )?;
        }
        for ((i, j), (l, k)) in edges {
            writeln!(f, "{i} {j} {l} {k}")?;
        }
        for p in &avatar.vertices {
            writeln!(f, "{} {} {}", p.x, p.y, p.z)?;
        }
        for t in &avatar.triangles {
            writeln!(f, "{} {} {}", t[0], t[1], t[2])?;
        }
        Ok(())
    }
}

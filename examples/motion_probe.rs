//! Opt-in accepted-state trace for diagnosing vibration, not a render benchmark.
use clap::Parser;
use marvelous_cloth::{
    fitting::FittingProject,
    vbd::{VbdSettings, VbdSolver},
};
use std::{
    fs,
    io::{BufWriter, Write},
    path::PathBuf,
};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value_t = 180)]
    steps: usize,
    #[arg(long, default_value_t = 16)]
    iterations: usize,
    #[arg(long, default_value_t = 1. / 60.)]
    dt: f64,
    #[arg(long, default_value_t = 1.5)]
    damping: f64,
    /// Explicit hardware opt-in; default remains CPU.
    #[arg(long)]
    gpu: bool,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build_global()?;
    let mut project = FittingProject::load(&args.project)?;
    let mut scene = project.compile()?;
    let mut solver = VbdSolver::new(
        scene.garment.clone(),
        Some(&scene.collider.surface),
        &scene.state,
    )?;
    let settings = VbdSettings {
        dt: args.dt,
        iterations: args.iterations,
        damping: args.damping,
        ..Default::default()
    };
    settings.validate()?;
    #[cfg(not(feature = "desktop"))]
    if args.gpu {
        return Err("Rebuild with desktop for --gpu".into());
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
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
        Some(marvelous_cloth::gpu_vbd::GpuVbd::new(device, queue))
    } else {
        None
    };
    fs::create_dir_all(&args.output)?;
    let mut trace = BufWriter::new(fs::File::create(args.output.join("trajectory.jsonl"))?);
    serde_json::to_writer(&mut trace, &scene.state)?;
    writeln!(trace)?;
    let mut metrics = BufWriter::new(fs::File::create(args.output.join("metrics.jsonl"))?);
    for i in 0..args.steps {
        #[cfg(feature = "desktop")]
        let m = match &gpu {
            Some(gpu) => gpu.step(&mut solver, &mut scene.state, &settings, None)?,
            None => solver.step(&mut scene.state, &settings, None)?,
        };
        #[cfg(not(feature = "desktop"))]
        let m = solver.step(&mut scene.state, &settings, None)?;
        serde_json::to_writer(&mut trace, &scene.state)?;
        writeln!(trace)?;
        serde_json::to_writer(&mut metrics, &m)?;
        writeln!(metrics)?;
        if i % 60 == 0 {
            eprintln!(
                "{i}/{}: {:.2}ms, {} contacts",
                args.steps, m.step_ms, m.contacts
            );
        }
    }
    trace.flush()?;
    metrics.flush()?;
    project.state = Some(scene.state);
    project.save(&args.output.join("fitting-project.json"))?;
    fs::write(
        args.output.join("settings.json"),
        serde_json::to_string_pretty(&settings)?,
    )?;
    Ok(())
}

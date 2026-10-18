//! A CPU-only frontend; never imports eframe/wgpu or opens a window.
use clap::{Parser, Subcommand};
use marvelous_cloth::{
    garment::{GarmentTemplate, export, print},
    solver::{ReferenceSolver, SolverSettings},
};
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
};

#[derive(Parser)]
#[command(
    about = "Validate, compile and assemble garment templates entirely on CPU",
    version
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
/// Paper size of a printed pattern sheet.
#[derive(Clone, Copy, clap::ValueEnum)]
enum PaperArg {
    A4,
    A3,
    Letter,
}
/// Printed size of one material metre.
#[derive(Clone, Copy, clap::ValueEnum)]
enum ScaleArg {
    #[value(name = "1:1")]
    Full,
    #[value(name = "1:2")]
    Half,
    #[value(name = "1:4")]
    Quarter,
}
#[derive(Subcommand)]
enum Command {
    /// Write a small editable example. Refuses to overwrite an existing file.
    Example {
        output: PathBuf,
    },
    Validate {
        template: PathBuf,
    },
    /// Produce rest OBJ, annotated 2D SVG and continuum fiber frames.
    Bake {
        template: PathBuf,
        #[arg(long, default_value = "output/template-bake")]
        output: PathBuf,
    },
    /// Write physically scaled SVG pattern sheets, one file per printed page.
    /// A multi-piece template also gets a leading cut-list page.
    Print {
        template: PathBuf,
        #[arg(long, default_value = "output/print")]
        output: PathBuf,
        #[arg(long, value_enum, default_value = "a4")]
        paper: PaperArg,
        #[arg(long, value_enum, default_value = "1:1")]
        scale: ScaleArg,
    },
    Simulate {
        template: PathBuf,
        #[arg(long, default_value = "output/template-simulation")]
        output: PathBuf,
        #[arg(long, default_value_t = 30)]
        steps: usize,
        #[arg(long,default_value_t=1.0/60.0)]
        dt: f64,
        #[arg(long, default_value_t = 2)]
        threads: usize,
    },
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Args::parse().command {
        Command::Example { output } => {
            let t = GarmentTemplate::two_panel_demo();
            t.compile()?;
            let mut file = File::create_new(output)?;
            file.write_all(serde_json::to_string_pretty(&t)?.as_bytes())?;
        }
        Command::Validate { template } => {
            let t = GarmentTemplate::load(&template)?;
            let g = t.compile()?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                "name":t.name,"panels":g.panels.len(),"vertices":g.mesh.uv.len(),"triangles":g.mesh.triangles.len(),
                "seams":g.seams.len(),"initial_gap_m":g.max_seam_gap(&g.initial_state.x),"warnings":g.warnings}))?
            );
        }
        Command::Bake { template, output } => {
            let t = GarmentTemplate::load(&template)?;
            let g = t.compile()?;
            export::write_artifacts(&t, &g, &g.initial_state, &output)?;
            println!("CPU artifacts written to {}", output.display());
        }
        Command::Print {
            template,
            output,
            paper,
            scale,
        } => {
            let t = GarmentTemplate::load(&template)?;
            t.compile()?;
            let paper = match paper {
                PaperArg::A4 => print::Paper::A4,
                PaperArg::A3 => print::Paper::A3,
                PaperArg::Letter => print::Paper::Letter,
            };
            let scale = match scale {
                ScaleArg::Full => print::PrintScale::Full,
                ScaleArg::Half => print::PrintScale::Half,
                ScaleArg::Quarter => print::PrintScale::Quarter,
            };
            let paths = print::write_sheets(&t, &output, paper, scale)?;
            println!(
                "{:.0}% scale on {} · {} SVG sheet(s) in {}",
                scale.millimetres_per_metre() / 10.,
                paper.name(),
                paths.len(),
                output.display()
            );
        }
        Command::Simulate {
            template,
            output,
            steps,
            dt,
            threads,
        } => {
            if steps == 0 || !(1..=64).contains(&threads) {
                return Err("Require steps > 0 and 1..64 CPU threads".into());
            }
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build_global()?;
            let settings = SolverSettings {
                dt,
                ..Default::default()
            };
            settings.validate()?;
            let t = GarmentTemplate::load(&template)?;
            let g = t.compile()?;
            let mut state = g.initial_state.clone();
            let initial_gap = g.max_seam_gap(&state.x);
            fs::create_dir_all(&output)?;
            fs::write(output.join("initial.obj"), export::obj(&g, &state))?;
            let mut log = BufWriter::new(File::create(output.join("metrics.jsonl"))?);
            let mut converged = 0;
            let mut last = None;
            for _ in 0..steps {
                let metrics = ReferenceSolver::step_garment(&g, &mut state, &settings, None)?;
                converged += usize::from(metrics.converged);
                serde_json::to_writer(&mut log, &metrics)?;
                writeln!(log)?;
                last = Some(metrics);
            }
            log.flush()?;
            export::write_artifacts(&t, &g, &state, &output)?;
            let report = serde_json::json!({"backend":"cpu-f64","gpu_initialized":false,"threads":threads,"solver":settings,
                "steps":steps,"converged_steps":converged,"initial_seam_gap_m":initial_gap,"final_seam_gap_m":g.max_seam_gap(&state.x),
                "last_step":last,"warnings":g.warnings});
            fs::write(
                output.join("run.json"),
                serde_json::to_string_pretty(&report)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if converged != steps {
                return Err(
                    "Some steps did not converge; inspect metrics.jsonl before using the result"
                        .into(),
                );
            }
        }
    }
    Ok(())
}

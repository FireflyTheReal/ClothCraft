//! Structural factorization timing and exact-feature diagnostics; CPU only.
use clap::Parser;
use marvelous_cloth::{
    math::{Jet, Scalar},
    ogc::smooth_forest::{Link, evaluate, evaluate_monolithic, wire},
};
use serde_json::{Value, json};
use std::{fs, hint::black_box, path::PathBuf, time::Instant};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    output: PathBuf,
}

fn timing(mut f: impl FnMut() -> f64) -> Value {
    black_box(f());
    let mut samples = Vec::new();
    // Fixed repetitions, same thread, alternating callers is handled below.
    for _ in 0..7 {
        let start = Instant::now();
        black_box(f());
        samples.push(start.elapsed().as_secs_f64());
    }
    samples.sort_by(f64::total_cmp);
    json!({"samples_s":samples,"median_s":samples[3]})
}

fn bench<S: Scalar>(n: usize, connected: bool, c: S) -> Result<Value, String> {
    let nodes = vec![S::c(0.3); n];
    let mut links = Vec::new();
    for i in 0..n / 2 {
        links.push(Link {
            nodes: [i, n - 1 - i],
            barrier: c,
        });
        if connected && i > 0 {
            links.push(Link {
                nodes: [i - 1, i],
                barrier: S::c(0.),
            });
        }
    }
    let a = evaluate(&nodes, &links, 1e-7)?;
    let b = evaluate_monolithic(&nodes, &links, 1e-7)?;
    let split = timing(|| {
        black_box(evaluate(black_box(&nodes), black_box(&links), 1e-7).unwrap())
            .energy
            .value()
    });
    let dense = timing(|| {
        black_box(evaluate_monolithic(black_box(&nodes), black_box(&links), 1e-7).unwrap())
            .energy
            .value()
    });
    // Reverse order to expose warm-up/order effects in this microbenchmark.
    let dense_second = timing(|| {
        black_box(evaluate_monolithic(black_box(&nodes), black_box(&links), 1e-7).unwrap())
            .energy
            .value()
    });
    let split_second = timing(|| {
        black_box(evaluate(black_box(&nodes), black_box(&links), 1e-7).unwrap())
            .energy
            .value()
    });
    Ok(
        json!({"nodes":n,"zero_bridges":connected,"energy_difference_j":a.energy.value()-b.energy.value(),
        "split_first":split,"dense_first":dense,"dense_second":dense_second,"split_second":split_second}),
    )
}

fn feature(x: f64, mode: usize) -> Jet<3> {
    let c = Jet::c;
    let p = [
        Jet::<3>::variable(x, 0),
        Jet::variable(0.001, 1),
        Jet::variable(0., 2),
    ];
    let positions = [
        [p[0], p[1], p[2] - c(0.01)],
        [p[0], p[1], p[2] + c(0.01)],
        [c(-0.1), c(0.), c(0.)],
        [c(0.), c(0.), c(0.)],
        [c(0.1), c(0.), c(0.)],
        [c(0.2), c(0.), c(0.)],
        [c(0.3), c(0.), c(0.)],
    ];
    let mut targets = vec![[2, 3], [3, 4]];
    if mode == 1 {
        targets.push([5, 6]);
    }
    if mode == 2 {
        targets.push([4, 5]);
    }
    wire(&positions, [0, 1], &targets, 0.003, 100_000., 1e-7)
        .unwrap()
        .energy
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.output.exists() {
        return Err("Choose a fresh output path".into());
    }
    let mut rows = Vec::new();
    for n in [8, 32, 96] {
        for connected in [false, true] {
            rows.push(json!({"scalar":"f64","result":bench(n,connected,0.1)?}));
            rows.push(
                json!({"scalar":"Jet3","result":bench(n,connected,Jet::<3>::variable(0.1,0))?}),
            );
        }
    }
    let features: Vec<_> = (0..3)
        .map(|mode| {
            let mid = feature(0., mode);
            let fd = (feature(1e-8, mode).g[0] - feature(-1e-8, mode).g[0]) / 2e-8;
            json!({"mode":(["pair","isolated_padding","connected_inactive_extension"][mode]),
            "energy_j":mid.v,"gradient_n":mid.g,"hxx_n_m":mid.h[0][0],"central_hxx_n_m":fd})
        })
        .collect();
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        args.output,
        serde_json::to_vec_pretty(&json!({
            "scope":"Single-thread CPU microbenchmark; not cloth throughput or GPU performance",
            "timings":rows,"features":features
        }))?,
    )?;
    Ok(())
}

//! Static law and conditioning probes; optional frozen-dress graph samples.
use clap::Parser;
use marvelous_cloth::{
    fitting::FittingProject,
    math::{Jet, Scalar},
    model::State,
    ogc::{
        potential::activation,
        smooth_forest::{self, Link},
    },
    vbd::VbdSolver,
};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    project: Option<PathBuf>,
    #[arg(long, requires = "project")]
    state: Option<PathBuf>,
}
fn geometry(x: f64, cusp: bool, tau: f64) -> Result<Jet<3>, String> {
    let p = [
        Jet::variable(x, 0),
        Jet::variable(0.001, 1),
        Jet::variable(if cusp { -0.0002 } else { 0. }, 2),
    ];
    let h = if cusp { 0.00005 } else { 0.01 };
    let extent = if cusp { 0.001 } else { 0.1 };
    let positions = [
        [p[0], p[1], p[2] - Jet::c(h)],
        [p[0], p[1], p[2] + Jet::c(h)],
        [Jet::c(-extent), Jet::c(0.), Jet::c(0.)],
        [
            Jet::c(if cusp { extent } else { 0. }),
            Jet::c(0.),
            Jet::c(0.),
        ],
        [
            Jet::c(if cusp { 0. } else { extent }),
            Jet::c(0.),
            Jet::c(if cusp { 0.0002 } else { 0. }),
        ],
    ];
    let targets = if cusp {
        vec![[2, 3], [3, 4], [4, 2]]
    } else {
        vec![[2, 3], [3, 4]]
    };
    Ok(smooth_forest::wire(&positions, [0, 1], &targets, 0.003, 100_000., tau)?.energy)
}
fn static_probes(taus: &[f64]) -> Result<Vec<Value>, String> {
    let mut rows = Vec::new();
    for &tau in taus {
        let mut samples: Vec<f64> = (0..=160).map(|i| -0.004 + i as f64 * 0.00005).collect();
        // Resolve the narrow force layer near barrier activation; a fixed
        // spatial grid alone would miss its maximum for small tau.
        for i in 0..=400 {
            let u = 10_f64.powf(-4. + 8. * i as f64 / 400.);
            let d = 0.003 - (2. * tau / 100_000.).sqrt() * u;
            if d >= 0.001 {
                let x = (d * d - 0.001_f64.powi(2)).sqrt();
                samples.extend([x, -x]);
            }
        }
        let mut max_force: f64 = 0.;
        let mut max_excess: f64 = 0.;
        let mut minimum_normal = f64::INFINITY;
        let reference = activation(0.001, 0.003, 100_000.);
        for x in samples {
            let j = geometry(x, false, tau)?;
            max_force = max_force.max(j.g[0].abs());
            max_excess = max_excess.max(j.v - reference);
            minimum_normal = minimum_normal.min(-j.g[1]);
        }
        let tie = geometry(0., false, tau)?;
        let expected_tie_curvature = -225_000. * tau.powi(2) / (tau + 2. * reference).powi(2);
        let cusp: Vec<_> = [1e-7, 1e-8, 1e-9]
            .into_iter()
            .map(|h| {
                let left = geometry(-h, true, tau)?;
                let right = geometry(h, true, tau)?;
                Ok(json!({"h_m":h,"gradient_difference_n":right.g[0]-left.g[0]}))
            })
            .collect::<Result<_, String>>()?;
        rows.push(json!({"tau_j":tau,"max_slide_force_n":max_force,
            "max_slide_energy_excess_j":max_excess,"minimum_normal_force_n":minimum_normal,
            "tie_curvature_n_m":tie.h[0][0],"expected_tie_curvature_n_m":expected_tie_curvature,
            "cusp":cusp}));
    }
    Ok(rows)
}
fn conditioning() -> Vec<Value> {
    let mut rows = Vec::new();
    for n in [4, 8, 16, 32] {
        for tau in [1e-3, 1e-6, 1e-9, 1e-12, 1e-14, 1e-16] {
            let nodes = vec![0.3; n];
            let mut links = Vec::new();
            let mut expected = 0.3 * n as f64;
            let mut expected_probabilities = Vec::new();
            for i in 0..n / 2 {
                let c = 0.05 + i as f64 * 0.001;
                links.push(Link {
                    nodes: [2 * i, 2 * i + 1],
                    barrier: c,
                });
                let p = 2. * c / (tau + 2. * c);
                expected -= c * p;
                expected_probabilities.push(p);
                if i > 0 {
                    links.push(Link {
                        nodes: [2 * i - 1, 2 * i],
                        barrier: 0.,
                    });
                    expected_probabilities.push(0.);
                }
            }
            for shuffled in [false, true] {
                let permuted: Vec<_> = links
                    .iter()
                    .map(|l| Link {
                        nodes: l.nodes.map(|i| if shuffled { (5 * i + 3) % n } else { i }),
                        barrier: l.barrier,
                    })
                    .collect();
                rows.push(match smooth_forest::evaluate(&nodes, &permuted, tau) {
                    Err(error) => json!({"nodes":n,"tau_j":tau,"shuffled":shuffled,"error":error}),
                    Ok(actual) => json!({"nodes":n,"tau_j":tau,"shuffled":shuffled,
                        "energy_error_j":(actual.energy-expected).abs(),
                        "max_probability_error":actual.inclusion_probabilities.iter()
                            .zip(&expected_probabilities).map(|(a,b)|(*a-*b).abs()).fold(0_f64,f64::max)}),
                });
            }
        }
    }
    rows
}
fn dense_cycle(taus: &[f64]) -> Result<Vec<Value>, String> {
    let mut rows = Vec::new();
    for &tau in taus {
        for y in [0.0001, 0.0003, 0.001, 0.002, 0.0029, 0.0031] {
            let dy = Jet::<1>::variable(y, 0);
            let c = Jet::c;
            let x = [
                [c(-0.01), dy, c(0.)],
                [c(0.01), dy, c(0.0001)],
                [c(-0.00001), c(0.), c(0.)],
                [c(0.00001), c(0.), c(0.)],
                [c(0.), c(0.00001), c(0.)],
                [c(0.), c(0.), c(0.00001)],
            ];
            let targets = [[2, 3], [2, 4], [2, 5], [3, 4], [3, 5], [4, 5]];
            let e = smooth_forest::wire(&x, [0, 1], &targets, 0.003, 100_000., tau)?.energy;
            rows.push(json!({"tau_j":tau,"gap_y_m":y,"energy_j":e.v,"normal_force_n":-e.g[0]}));
        }
    }
    Ok(rows)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.output.exists() {
        return Err("Choose a fresh output path".into());
    }
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build_global()?;
    let taus = [1e-3, 1e-5, 1e-7, 1e-9, 1e-11];
    let mut dress = Vec::new();
    let mut state_time = None;
    if let Some(path) = &args.project {
        let project = FittingProject::load(path)?;
        let scene = project.compile()?;
        let state: State = if let Some(path) = &args.state {
            serde_json::from_slice(&fs::read(path)?)?
        } else {
            scene.state.clone()
        };
        if state.x.len() != scene.state.x.len() || state.x.iter().any(|p| !p.is_finite()) {
            return Err("Invalid diagnostic snapshot".into());
        }
        state_time = Some(state.time);
        let solver = VbdSolver::new(scene.garment, Some(&scene.collider.surface), &scene.state)?;
        let x = solver.world.positions(&state.x);
        let values: Vec<_> = x.iter().map(|p| p.to_array()).collect();
        for radius in [0.003, 0.006, 0.012] {
            let mut cfg = project.simulation.clone().unwrap_or_default().contact;
            cfg.radius = radius;
            let audit = solver.world.audit_wire_quadrature(&x, &cfg)?;
            let mut rows = audit.worst_rows;
            rows.extend(audit.largest_graph_rows);
            rows.sort_by_key(|r| r.source_edge);
            rows.dedup_by_key(|r| r.source_edge);
            for row in rows {
                let targets: Vec<_> = row
                    .target_edges
                    .iter()
                    .map(|i| solver.world.topology.edges[*i])
                    .collect();
                for tau in taus {
                    let evaluation = match smooth_forest::wire(
                        &values,
                        row.source_vertices,
                        &targets,
                        radius,
                        cfg.stiffness,
                        tau,
                    ) {
                        Err(error) => json!({"error":error}),
                        Ok(e) => {
                            json!({"energy_j":e.energy,"maximum_primitive_j":row.maximum_primitive_j,
                            "lower_bound_deficit_j":(row.maximum_primitive_j-e.energy).max(0.),
                            "minimum_probability":e.inclusion_probabilities.iter().copied().reduce(f64::min),
                            "maximum_probability":e.inclusion_probabilities.iter().copied().reduce(f64::max)})
                        }
                    };
                    dress.push(json!({"radius_m":radius,"source_edge":row.source_edge,"nodes":targets.len(),
                        "tau_j":tau,"evaluation":evaluation}));
                }
            }
        }
    }
    let report = json!({"scope":"CPU law probes and selected dress graph snapshots; no cloth stepping",
        "project":args.project,"state":args.state,"state_time_s":state_time,
        "slide_and_cusp":static_probes(&taus)?,"conditioning":conditioning(),
        "dense_cycle":dense_cycle(&taus)?,"dress":dress});
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(args.output, serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}

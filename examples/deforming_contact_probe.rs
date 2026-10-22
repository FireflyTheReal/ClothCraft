//! Coupled two-layer cloth research. Hard VF/EE reactions on both surfaces,
//! original OGC movement bounds, and conservative swept-clearance certificates.
use clap::Parser;
use glam::DVec3;
use marvelous_cloth::{
    ogc::{OgcSettings, feature_hierarchy::Pair},
    solver_lab::{
        contact_qp::Settings,
        contact_residual, coupled_patch,
        deforming_contact::{self, Certificate},
        dense_contact::{self, DualMethod, Row},
    },
    vbd::VbdSolver,
};
use serde_json::json;
use std::{fs, path::PathBuf, sync::Arc, time::Instant};
#[path = "support/discrete_newton.rs"]
mod discrete_newton;
#[path = "support/deforming_fixture.rs"]
mod fixture;
#[path = "support/residual_samples.rs"]
mod residual_samples;
use fixture::{GAP, floor, template};
const TOL: f64 = 1e-10;
const DT: f64 = 1. / 60.;
struct AcceptedTrial {
    positions: Vec<DVec3>,
    multipliers: Vec<f64>,
    alpha: f64,
    start_alpha: f64,
    backtracks: usize,
    guard_vertex: Option<usize>,
    merit: f64,
    method: &'static str,
}
struct ForceAudit {
    material: Vec<f64>,
    contact: Vec<f64>,
    residual: Vec<f64>,
}
#[derive(Parser)]
struct Args {
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value_t = 180)]
    steps: usize,
    #[arg(long)]
    held: bool,
    /// Experimental sequential-convex correction of violated swept constraints.
    #[arg(long)]
    swept_repair: bool,
    /// Alternative: retain every old path linearization instead of refreshing.
    #[arg(long, requires = "swept_repair")]
    accumulate_repair: bool,
    #[arg(long, default_value_t = 96)]
    iterations: usize,
    #[arg(long)]
    independent_block: bool,
    #[arg(long, requires = "independent_block")]
    no_schur_cache: bool,
    /// Add two sewn, initially folded panels over a third deformable panel.
    /// Zero means matching lengths; 0.1 means ten percent extra seam length.
    #[arg(long)]
    sewn_ease: Option<f64>,
    #[arg(long, default_value_t = 20.)]
    fold_degrees: f64,
    #[arg(long)]
    progress: bool,
    /// Keep the failed frame and end the pilot before another time step.
    #[arg(long)]
    stop_on_unconverged: bool,
    /// Reference path: assemble unused derivatives during energy-only queries.
    #[arg(long)]
    full_energy_assembly: bool,
    #[arg(long)]
    audit_iterations: bool,
    /// Resume a converged synthetic frame; material rest is compiled afresh.
    #[arg(long, requires = "resume_after")]
    resume: Option<PathBuf>,
    #[arg(long, requires = "resume")]
    resume_after: Option<usize>,
    #[arg(
        long,
        requires = "independent_block",
        conflicts_with = "no_schur_cache"
    )]
    pivot_block: bool,
    /// Decompose mechanical-energy changes; never modifies positions/velocities.
    #[arg(long)]
    audit_energy: bool,
    /// Record physical reaction multipliers for offline force-row experiments.
    #[arg(long)]
    capture_reactions: bool,
    /// Research force correction; geometric inequalities remain unchanged.
    #[arg(long)]
    discrete_forces: bool,
    /// Finite-difference nonsymmetric Newton search on physical active contacts.
    #[arg(long, requires = "discrete_forces")]
    newton_forces: bool,
    /// Rank-revealing least-squares Newton proposal; all physical gates remain.
    #[arg(long, requires = "newton_forces")]
    rank_newton: bool,
    /// Pivot contact equalities while retaining every geometric inequality.
    #[arg(long, requires = "rank_newton")]
    unilateral_newton: bool,
    /// Use the current natural-complementarity residual branch for Newton.
    #[arg(long, requires = "unilateral_newton")]
    natural_newton: bool,
    /// Compare unilateral and natural-residual directions with identical gates.
    #[arg(
        long,
        requires = "unilateral_newton",
        conflicts_with = "natural_newton"
    )]
    combined_newton: bool,
    /// Bound reaction variables in natural-residual least squares.
    #[arg(
        long,
        requires = "unilateral_newton",
        conflicts_with = "natural_newton"
    )]
    bounded_newton: bool,
    /// Try bounded search only if ordinary candidates find no acceptable step.
    #[arg(long, requires_all = ["bounded_newton", "combined_newton"])]
    fallback_bounded: bool,
    /// Include hard linearized geometric constraints in residual minimization.
    #[arg(long, requires = "bounded_newton")]
    constrained_newton: bool,
    /// Add swept-path linearizations to the constrained SEARCH, not its forces.
    #[arg(
        long,
        requires = "constrained_newton",
        conflicts_with = "fallback_bounded"
    )]
    newton_path_repair: bool,
    /// Reuse identical residual evaluations within one Newton linearization.
    #[arg(long, requires = "newton_forces")]
    reuse_newton_samples: bool,
    /// Also certify affine separating normals with quadratic Bernstein bounds.
    #[arg(long)]
    moving_plane_certificates: bool,
}
fn main() -> Result<(), String> {
    let args = Args::parse();
    let method = if args.pivot_block {
        DualMethod::IndependentPivotBlock
    } else if args.no_schur_cache {
        DualMethod::IndependentBlockUncached
    } else if args.independent_block {
        DualMethod::IndependentBlock
    } else {
        DualMethod::ActiveBlock
    };
    if args.iterations == 0 {
        return Err("Iteration budget must be positive".into());
    }
    let g = Arc::new(template(args.sewn_ease, args.fold_degrees)?.compile()?);
    let mut state = g.initial_state.clone();
    let floor = floor();
    let mut reference = VbdSolver::new(g.clone(), Some(&floor), &state)?;
    let x = reference.world.positions(&state.x);
    let radius = 4. * x.iter().map(|p| p.length()).fold(1., f64::max);
    // This radius covers the entire initial universe. Retain ALL eligible
    // VF/EE identities, not a temporal near-contact cache. Topology is fixed.
    let pairs: Vec<_> = reference
        .world
        .feature_hierarchy(&x, radius)?
        .pairs
        .into_iter()
        .filter(|pair| {
            matches!(pair, Pair::VertexFace(_) | Pair::EdgeEdge(_))
                && pair
                    .vertices(&reference.world.topology)
                    .0
                    .into_iter()
                    .all(|i| i < state.x.len())
        })
        .collect();
    let top = reference.world.topology.clone();
    let certify = |pair, from: &[DVec3], to: &[DVec3]| {
        if args.moving_plane_certificates {
            deforming_contact::certify_moving_planes(pair, &top, from, to, GAP, TOL, 10)
        } else {
            deforming_contact::certify(pair, &top, from, to, GAP, TOL, 10)
        }
    };
    if pairs.iter().any(|p| p.distance(&top, &x) < GAP - TOL) {
        return Err("Initially infeasible cloth clearance".into());
    }
    let n = state.x.len();
    let dofs = n * 3;
    let inertia: Vec<_> = g.mesh.masses.iter().map(|m| m / (DT * DT)).collect();
    let energy_only = |p: &[DVec3], target: &[DVec3]| -> Result<f64, String> {
        if args.full_energy_assembly {
            Ok(coupled_patch::assemble(&g, p, target, &inertia)?.energy)
        } else {
            coupled_patch::energy(&g, p, target, &inertia)
        }
    };
    let control_vertex = g.panels[1].vertices.start + 1;
    let origin = state.x[control_vertex];
    let mut first_step = 0;
    if let Some(path) = &args.resume {
        let saved: serde_json::Value =
            serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let after = args.resume_after.unwrap();
        if saved["vertices"] != json!(n)
            || saved["sewn_ease"] != json!(args.sewn_ease)
            || saved["fold_degrees"] != json!(args.sewn_ease.map(|_| args.fold_degrees))
            || saved["held"] != json!(args.held)
        {
            return Err("Resume fixture/protocol does not match".into());
        }
        let frame = saved["frames"]
            .as_array()
            .and_then(|frames| frames.iter().find(|f| f["step"] == json!(after)))
            .ok_or("Resume frame missing")?;
        if frame["converged"] != json!(true) {
            return Err("Resume requires a converged frame".into());
        }
        state.x = serde_json::from_value(frame["x"].clone()).map_err(|e| e.to_string())?;
        state.v = serde_json::from_value(frame["v"].clone()).map_err(|e| e.to_string())?;
        if state.x.len() != n
            || state.v.len() != n
            || state.x.iter().chain(&state.v).any(|p| !p.is_finite())
        {
            return Err("Invalid resume state".into());
        }
        first_step = after.checked_add(1).ok_or("Resume step overflow")?;
        if first_step >= args.steps {
            return Err("Resume must precede final step".into());
        }
        state.time = first_step as f64 * DT;
        reference
            .world
            .validate_start(&reference.world.positions(&state.x))?;
    }
    let release = if args.held { 130 } else { 100 };
    let mut trace = vec![];
    let mut failure = None;
    let mut failed_qp = None;
    let mut failed_newton = None;
    let mut failed_nonlinear = None;
    let mut failed_repair_qp = None;
    let mut material_previous = energy_only(&state.x, &state.x)?;
    let mut energy_previous = material_previous
        + state
            .x
            .iter()
            .zip(&state.v)
            .zip(&g.mesh.masses)
            .map(|((p, v), m)| m * (0.5 * v.length_squared() + 9.81 * p.y))
            .sum::<f64>();
    let mut max_rise: f64 = 0.;
    let cfg = OgcSettings::default();
    let start = Instant::now();
    'steps: for step in first_step..args.steps {
        let old = state.x.clone();
        let old_v = state.v.clone();
        let target: Vec<_> = old
            .iter()
            .zip(&state.v)
            .map(|(p, v)| *p + DT * (*v - DT * 9.81 * DVec3::Y))
            .collect();
        let grabbing = (60..release).contains(&step);
        let control =
            origin + ((step as f64 - 59.) / 40.).clamp(0., 1.) * DVec3::new(0.025, 0.006, 0.);
        let objective = |p: &[DVec3]| -> Result<f64, String> {
            Ok(energy_only(p, &target)?
                + if grabbing {
                    5. * p[control_vertex].distance_squared(control)
                } else {
                    0.
                })
        };
        let mut converged = false;
        let mut residual = f64::INFINITY;
        let mut complementarity = f64::INFINITY;
        let mut iterations = 0;
        let mut qp_sweeps = 0;
        let mut inexact_search_qps = 0;
        let mut fallbacks = 0;
        let mut reduced_blocks = 0;
        let mut boundary_pivots = 0;
        let mut schur_computed = 0;
        let mut schur_hits = 0;
        let mut schur_max_bytes = 0;
        let mut added_rows = 0;
        let mut certificate_intervals = 0;
        let mut certificate_rejections = 0;
        let mut reaction_counts = [0usize; 2];
        let mut repair_solves = 0;
        let mut repair_failures = 0;
        let mut minimum_alpha: f64 = 1.;
        let mut iteration_trace = vec![];
        let mut final_forces = None;
        let mut reaction_trace = None;
        let mut newton_steps = 0;
        let mut newton_rejections = Vec::new();
        let old_world = reference.world.positions(&old);
        let contact_problem = contact_residual::Problem {
            topology: &top,
            pairs: &pairs,
            old: &old_world,
            cloth_vertices: n,
            clearance: GAP,
        };
        let mut lambda_history = vec![0.; n + pairs.len()];
        for iteration in 0..args.iterations {
            iterations = iteration + 1;
            let mut s = coupled_patch::assemble(&g, &state.x, &target, &inertia)?;
            if grabbing {
                let d = state.x[control_vertex] - control;
                s.energy += 5. * d.length_squared();
                for axis in 0..3 {
                    let k = 3 * control_vertex + axis;
                    s.gradient[k] += 10. * d[axis];
                    s.metric[k * dofs + k] += 10.;
                }
            }
            let x = reference.world.positions(&state.x);
            let mut search_gradient = s.gradient.clone();
            let old_merit = if args.discrete_forces {
                let discrete = contact_problem.evaluate(&x, &lambda_history, true)?;
                let endpoint = contact_problem.evaluate(&x, &lambda_history, false)?;
                for ((g, d), e) in search_gradient
                    .iter_mut()
                    .zip(&discrete.force)
                    .zip(&endpoint.force)
                {
                    *g -= d - e;
                }
                let balance: Vec<_> = s
                    .gradient
                    .iter()
                    .zip(&discrete.force)
                    .map(|(g, f)| g - f)
                    .collect();
                Some(contact_residual::merit(
                    &balance,
                    &discrete.gaps,
                    &lambda_history,
                    1e-7,
                    TOL,
                )?)
            } else {
                None
            };
            let mut active: Vec<_> = (0..pairs.len())
                .filter(|&i| {
                    pairs[i].distance(&top, &x) <= GAP + 1e-8
                        || (args.discrete_forces && lambda_history[n + i] > 0.)
                })
                .collect();
            let (q, rows) = loop {
                let mut rows: Vec<_> = (0..n)
                    .map(|i| {
                        let mut jacobian = vec![0.; dofs];
                        jacobian[3 * i + 1] = 1.;
                        Row {
                            jacobian,
                            offset: GAP - state.x[i].y,
                        }
                    })
                    .collect();
                for &i in &active {
                    let mut row = deforming_contact::linearize(pairs[i], &top, &x, GAP)?;
                    row.jacobian.truncate(dofs);
                    rows.push(row);
                }
                let q = dense_contact::solve_with_method(
                    &s.metric,
                    &search_gradient,
                    &rows,
                    Settings {
                        max_sweeps: 8192,
                        feasibility_m: 1e-11,
                        stationarity_n: 1e-7,
                        complementarity_j: 1e-12,
                    },
                    method,
                )?;
                qp_sweeps += q.sweeps;
                fallbacks += q.singular_active_blocks;
                reduced_blocks += q.reduced_active_blocks;
                boundary_pivots += q.boundary_pivots;
                schur_computed += q.schur_entries_computed;
                schur_hits += q.schur_cache_hits;
                schur_max_bytes = schur_max_bytes.max(q.schur_cache_bytes);
                if !q.converged {
                    failed_qp = Some(json!({"step":step,"iteration":iteration,"metric":s.metric,
                        "gradient":search_gradient,"rows":rows.iter().map(|r|json!({"jacobian":r.jacobian,"offset":r.offset})).collect::<Vec<_>>(),
                        "report":q,"x":state.x,"target":target}));
                    if args.discrete_forces {
                        // This QP is only a search model for a different force
                        // law. Its status is diagnostic, never final acceptance.
                        // All trials still need full residual merit and swept
                        // feasibility; physical convergence is checked below.
                        inexact_search_qps += 1;
                    } else {
                        failure = Some(format!(
                            "QP failed at step {step}, iteration {iteration}: residual {}, gap {}, complementarity {}",
                            q.stationarity_n, q.feasibility_m, q.complementarity_j
                        ));
                        break 'steps;
                    }
                }
                let mut proposed = x.clone();
                for (p, d) in proposed[..n]
                    .iter_mut()
                    .zip(q.displacement.as_chunks::<3>().0.iter())
                {
                    *p += DVec3::from_slice(d);
                }
                let mut missing = vec![];
                for (i, &pair) in pairs.iter().enumerate() {
                    if !active.contains(&i)
                        && !matches!(certify(pair, &x, &proposed)?, Certificate::Safe { .. })
                    {
                        missing.push(i);
                    }
                }
                if missing.is_empty() {
                    break (q, rows);
                }
                added_rows += missing.len();
                active.extend(missing);
            };
            let physical_multipliers = if args.discrete_forces {
                (0..rows.len())
                    .map(|i| lambda_history[if i < n { i } else { n + active[i - n] }])
                    .collect::<Vec<_>>()
            } else {
                q.multipliers.clone()
            };
            let mut counts = [0usize; 2];
            for (&id, &lambda) in active.iter().zip(&physical_multipliers[n..]) {
                if lambda > 1e-10 {
                    match pairs[id] {
                        Pair::VertexFace(_) => counts[0] += 1,
                        Pair::EdgeEdge(_) => counts[1] += 1,
                        _ => unreachable!(),
                    }
                }
            }
            for i in 0..2 {
                reaction_counts[i] = reaction_counts[i].max(counts[i]);
            }
            let mut balance = s.gradient.clone();
            complementarity = 0.;
            for (r, l) in rows.iter().zip(&q.multipliers) {
                for (b, j) in balance.iter_mut().zip(&r.jacobian) {
                    *b -= l * j;
                }
                complementarity = complementarity.max((l * r.offset).abs());
            }
            let mut lambda_target = vec![0.; n + pairs.len()];
            lambda_target[..n].copy_from_slice(&q.multipliers[..n]);
            for (&i, &lambda) in active.iter().zip(&q.multipliers[n..]) {
                lambda_target[n + i] = lambda;
            }
            let mut physical_geometry_valid = true;
            if args.discrete_forces {
                let reaction = contact_problem.evaluate(&x, &lambda_history, true)?;
                physical_geometry_valid = reaction.gaps.iter().all(|g| *g >= -TOL);
                balance = s
                    .gradient
                    .iter()
                    .zip(&reaction.force)
                    .map(|(g, f)| g - f)
                    .collect();
                complementarity = reaction.complementarity_j;
            }
            residual = balance.iter().map(|v| v * v).sum::<f64>().sqrt();
            let correction = q.displacement.iter().map(|v| v * v).sum::<f64>().sqrt();
            if args.audit_iterations {
                iteration_trace.push(json!({"iteration":iteration,"objective_j":s.energy,
                    "force_residual_n":residual,"complementarity_j":complementarity,
                    "search_norm_m":correction,"residual_merit":old_merit,"qp_sweeps":q.sweeps,"active_pairs":active,
                    "positive_reactions_vf_ee":counts}));
            }
            if (args.discrete_forces || correction < 1e-9)
                && residual < 1e-7
                && complementarity < 1e-12
                && physical_geometry_valid
            {
                if args.capture_reactions {
                    let contacts: Vec<_> = active
                        .iter()
                        .zip(&physical_multipliers[n..])
                        .filter(|(_, l)| **l > 0.)
                        .map(|(&i, &lambda)| {
                            let (kind, indices) = match pairs[i] {
                                Pair::VertexFace(ids) => ("vf", ids),
                                Pair::EdgeEdge(ids) => ("ee", ids),
                                _ => unreachable!(),
                            };
                            json!({"kind":kind,"indices":indices,"multiplier":lambda})
                        })
                        .collect();
                    reaction_trace = Some(
                        json!({"previous_x":old,"floor_multipliers":&physical_multipliers[..n],"contacts":contacts}),
                    );
                }
                if args.audit_energy {
                    let mut material_gradient = s.gradient.clone();
                    for i in 0..n {
                        for k in 0..3 {
                            material_gradient[3 * i + k] -=
                                inertia[i] * (state.x[i][k] - target[i][k]);
                        }
                    }
                    if grabbing {
                        for k in 0..3 {
                            material_gradient[3 * control_vertex + k] -=
                                10. * (state.x[control_vertex][k] - control[k]);
                        }
                    }
                    let contact = s
                        .gradient
                        .iter()
                        .zip(&balance)
                        .map(|(g, r)| g - r)
                        .collect();
                    final_forces = Some(ForceAudit {
                        material: material_gradient,
                        contact,
                        residual: balance.clone(),
                    });
                }
                converged = true;
                break;
            }
            let mut directions: Vec<_> = q
                .displacement
                .as_chunks::<3>()
                .0
                .iter()
                .map(|p| DVec3::from_array(*p))
                .collect();
            // The physical KKT test above uses CURRENT geometry. Extra rows
            // below constrain the SEARCH PATH, not the physical force model.
            // Every proposal must still pass original OGC and swept checks.
            if args.swept_repair {
                let mut repair_rows = rows.clone();
                let mut latest: std::collections::BTreeMap<Pair, Row> =
                    std::collections::BTreeMap::new();
                for _ in 0..8 {
                    let mut proposed = x.clone();
                    for i in 0..n {
                        proposed[i] += directions[i];
                    }
                    let mut new_rows = vec![];
                    let mut violations = 0;
                    for &pair in &pairs {
                        let time = match certify(pair, &x, &proposed)? {
                            Certificate::Violated { time, .. } if time > 1e-8 => Some(time),
                            Certificate::Unresolved { begin, end, .. } => Some(0.5 * (begin + end)),
                            _ => None,
                        };
                        if let Some(time) = time {
                            violations += 1;
                            let at: Vec<_> = x
                                .iter()
                                .zip(&proposed)
                                .map(|(a, b)| a.lerp(*b, time))
                                .collect();
                            let mut r = deforming_contact::linearize(pair, &top, &at, GAP)?;
                            r.jacobian.truncate(dofs);
                            let projection: f64 = r
                                .jacobian
                                .as_chunks::<3>()
                                .0
                                .iter()
                                .zip(&directions)
                                .map(|(j, d)| DVec3::from_slice(j).dot(*d))
                                .sum();
                            r.offset = r.offset / time + projection;
                            if args.accumulate_repair {
                                new_rows.push(r);
                            } else {
                                latest.insert(pair, r);
                            }
                        }
                    }
                    if violations == 0 {
                        break;
                    }
                    if args.accumulate_repair {
                        repair_rows.extend(new_rows);
                    } else {
                        repair_rows = rows
                            .iter()
                            .cloned()
                            .chain(latest.values().cloned())
                            .collect();
                    }
                    let repaired = dense_contact::solve_with_method(
                        &s.metric,
                        &search_gradient,
                        &repair_rows,
                        Settings {
                            max_sweeps: 8192,
                            feasibility_m: 1e-11,
                            stationarity_n: 1e-7,
                            complementarity_j: 1e-12,
                        },
                        method,
                    )?;
                    repair_solves += 1;
                    qp_sweeps += repaired.sweeps;
                    fallbacks += repaired.singular_active_blocks;
                    reduced_blocks += repaired.reduced_active_blocks;
                    boundary_pivots += repaired.boundary_pivots;
                    schur_computed += repaired.schur_entries_computed;
                    schur_hits += repaired.schur_cache_hits;
                    schur_max_bytes = schur_max_bytes.max(repaired.schur_cache_bytes);
                    if !repaired.converged {
                        if failed_repair_qp.is_none() {
                            failed_repair_qp = Some(json!({"step":step,"iteration":iteration,
                                "metric":s.metric,"gradient":search_gradient,"physical_rows":rows.len(),
                                "rows":repair_rows.iter().map(|r|json!({"jacobian":r.jacobian,"offset":r.offset})).collect::<Vec<_>>(),
                                "report":repaired,"x":state.x,"target":target}));
                        }
                        repair_failures += 1;
                        break;
                    }
                    directions = repaired
                        .displacement
                        .as_chunks::<3>()
                        .0
                        .iter()
                        .map(|p| DVec3::from_array(*p))
                        .collect();
                }
            }
            let mut proposals = vec![(directions.clone(), lambda_target.clone(), "qp")];
            if args.newton_forces {
                let indexed: Vec<_> = rows
                    .iter()
                    .enumerate()
                    .map(|(i, r)| (if i < n { i } else { n + active[i - n] }, r.clone()))
                    .collect();
                let methods = if args.combined_newton {
                    vec![
                        (discrete_newton::Method::Unilateral, "newton"),
                        (
                            if args.constrained_newton {
                                discrete_newton::Method::Constrained
                            } else if args.bounded_newton {
                                discrete_newton::Method::Bounded
                            } else {
                                discrete_newton::Method::Natural
                            },
                            if args.bounded_newton {
                                "bounded"
                            } else {
                                "natural"
                            },
                        ),
                    ]
                } else {
                    vec![(
                        if args.constrained_newton {
                            discrete_newton::Method::Constrained
                        } else if args.bounded_newton {
                            discrete_newton::Method::Bounded
                        } else if args.natural_newton {
                            discrete_newton::Method::Natural
                        } else if args.unilateral_newton {
                            discrete_newton::Method::Unilateral
                        } else if args.rank_newton {
                            discrete_newton::Method::Rank
                        } else {
                            discrete_newton::Method::Strict
                        },
                        "newton",
                    )]
                };
                for (method, name) in methods {
                    let proposal_start = Instant::now();
                    let mut path_geometry_ms = 0.;
                    let mut path_search_ms = 0.;
                    // Lifetime is exactly one method/iterate: target, material,
                    // history and control are fixed throughout all path repairs.
                    let samples = residual_samples::Samples::new(args.reuse_newton_samples);
                    let evaluate_balance = |p: &[DVec3], l: &[f64]| {
                        samples.evaluate(p, l, || {
                            let mut trial = coupled_patch::assemble(&g, p, &target, &inertia)?;
                            if grabbing {
                                for k in 0..3 {
                                    trial.gradient[3 * control_vertex + k] +=
                                        10. * (p[control_vertex][k] - control[k]);
                                }
                            }
                            let reaction =
                                contact_problem.evaluate(&reference.world.positions(p), l, true)?;
                            Ok(trial
                                .gradient
                                .iter()
                                .zip(&reaction.force)
                                .map(|(g, f)| g - f)
                                .collect())
                        })
                    };
                    let attempt = discrete_newton::direction(
                        &state.x,
                        &lambda_history,
                        &lambda_target,
                        &indexed,
                        method,
                        &mut failed_newton,
                        evaluate_balance,
                    );
                    let initial_direction_ms = proposal_start.elapsed().as_secs_f64() * 1000.;
                    if let Some(capture) = failed_newton.as_mut()
                        && capture.get("step").is_none()
                    {
                        capture["step"] = json!(step);
                        capture["iteration"] = json!(iteration);
                    }
                    match attempt {
                        Ok((d, l)) => {
                            if args.newton_path_repair
                                && matches!(method, discrete_newton::Method::Constrained)
                            {
                                let mut candidate = d.clone();
                                let mut search_rows = vec![];
                                for repair in 0..8 {
                                    let mut proposed = x.clone();
                                    for i in 0..n {
                                        proposed[i] += candidate[i];
                                    }
                                    let before = search_rows.len();
                                    let geometry_start = Instant::now();
                                    for &pair in &pairs {
                                        let time = match certify(pair, &x, &proposed)? {
                                            Certificate::Violated { time, .. } if time > 1e-8 => {
                                                Some(time)
                                            }
                                            Certificate::Unresolved { begin, end, .. } => {
                                                Some(0.5 * (begin + end))
                                            }
                                            _ => None,
                                        };
                                        if let Some(time) = time {
                                            let at: Vec<_> = x
                                                .iter()
                                                .zip(&proposed)
                                                .map(|(a, b)| a.lerp(*b, time))
                                                .collect();
                                            let mut row =
                                                deforming_contact::linearize(pair, &top, &at, GAP)?;
                                            row.jacobian.truncate(dofs);
                                            let projection: f64 = row
                                                .jacobian
                                                .as_chunks::<3>()
                                                .0
                                                .iter()
                                                .zip(&candidate)
                                                .map(|(j, d)| DVec3::from_slice(j).dot(*d))
                                                .sum();
                                            row.offset = row.offset / time + projection;
                                            search_rows.push(row);
                                        }
                                    }
                                    path_geometry_ms +=
                                        geometry_start.elapsed().as_secs_f64() * 1000.;
                                    if search_rows.len() == before {
                                        break;
                                    }
                                    let search_start = Instant::now();
                                    let repaired = discrete_newton::direction_with_search_rows(
                                        &state.x,
                                        &lambda_history,
                                        &lambda_target,
                                        &indexed,
                                        method,
                                        &mut failed_newton,
                                        &search_rows,
                                        evaluate_balance,
                                    );
                                    path_search_ms += search_start.elapsed().as_secs_f64() * 1000.;
                                    if let Some(capture) = failed_newton.as_mut() {
                                        capture["step"] = json!(step);
                                        capture["iteration"] = json!(iteration);
                                        capture["path_repair"] = json!(repair);
                                    }
                                    match repaired {
                                        Ok((dx, reactions)) => {
                                            candidate = dx.clone();
                                            proposals.push((dx, reactions, "path"));
                                        }
                                        Err(reason) => {
                                            newton_rejections.push(json!({"iteration":iteration,
                                                "method":"path","repair":repair,"reason":reason}));
                                            break;
                                        }
                                    }
                                }
                            }
                            proposals.push((d, l, name));
                        }
                        Err(reason) => {
                            newton_rejections
                                .push(json!({"iteration":iteration,"method":name,"reason":reason}));
                        }
                    }
                    if let Some(entry) = iteration_trace.last_mut() {
                        let stats = entry
                            .as_object_mut()
                            .unwrap()
                            .entry("newton_samples")
                            .or_insert_with(|| json!({}))
                            .as_object_mut()
                            .unwrap();
                        stats.insert(
                            name.into(),
                            json!({"calls":samples.calls.get(),
                            "evaluations":samples.evaluations.get(),
                            "evaluation_ms":samples.evaluation_ns.get() as f64 / 1e6,
                            "initial_direction_ms":initial_direction_ms,
                            "path_geometry_ms":path_geometry_ms,
                            "path_search_ms":path_search_ms,
                            "proposal_ms":proposal_start.elapsed().as_secs_f64() * 1000.}),
                        );
                    }
                }
            }
            let guards = reference.world.detect(&x, 2. * cfg.radius, &cfg)?;
            let mut best: Option<AcceptedTrial> = None;
            let mut line_search_trace = vec![];
            for (directions, lambda_target, name) in &proposals {
                if args.fallback_bounded && *name == "bounded" && best.is_some() {
                    continue;
                }
                let mut alpha: f64 = 1.;
                let mut guard_vertex = None;
                // A generalized Newton direction can leave the reaction cone.
                // Restrict only its search length; never accept attraction.
                for (&a, &b) in lambda_history.iter().zip(lambda_target) {
                    if b < 0. {
                        alpha = alpha.min(0.99 * a / (a - b));
                    }
                }
                if alpha == 0. {
                    if args.audit_iterations {
                        line_search_trace
                            .push(json!({"method":name,"reason":"Reaction cone blocks direction"}));
                    }
                    continue;
                }
                for (i, d) in directions.iter().enumerate() {
                    if d.length() > 1e-15 {
                        let (end, _) = guards.truncate_step(i, state.x[i], state.x[i] + *d);
                        let bound = end.distance(state.x[i]) / d.length();
                        if bound < alpha {
                            alpha = bound;
                            guard_vertex = Some(i);
                        }
                    }
                }
                let start_alpha = alpha;
                let slope: f64 = directions
                    .iter()
                    .zip(s.gradient.as_chunks::<3>().0.iter())
                    .map(|(d, g)| d.dot(DVec3::from_slice(g)))
                    .sum();
                for backtrack in 0..32 {
                    let p: Vec<_> = state
                        .x
                        .iter()
                        .zip(directions)
                        .map(|(p, d)| *p + alpha * *d)
                        .collect();
                    let next = reference.world.positions(&p);
                    let mut safe = p.iter().all(|p| p.y >= GAP - TOL);
                    let mut blocked_by = if safe {
                        None
                    } else {
                        Some(
                            json!({"floor_min_m":p.iter().map(|p|p.y).fold(f64::INFINITY,f64::min)}),
                        )
                    };
                    if safe {
                        for &pair in &pairs {
                            match certify(pair, &x, &next)? {
                                Certificate::Safe { intervals } => {
                                    certificate_intervals += intervals
                                }
                                certificate => {
                                    safe = false;
                                    certificate_rejections += 1;
                                    if args.audit_iterations {
                                        blocked_by =
                                            Some(json!({"pair":pair,"certificate":certificate}));
                                    }
                                    break;
                                }
                            }
                        }
                    }
                    let trial_lambda: Vec<_> = lambda_history
                        .iter()
                        .zip(lambda_target)
                        .map(|(a, b)| (1. - alpha) * a + alpha * b)
                        .collect();
                    let mut trial_merit = 0.;
                    let decrease = if safe && args.discrete_forces {
                        let mut trial = coupled_patch::assemble(&g, &p, &target, &inertia)?;
                        if grabbing {
                            for axis in 0..3 {
                                trial.gradient[3 * control_vertex + axis] +=
                                    10. * (p[control_vertex][axis] - control[axis]);
                            }
                        }
                        let reaction = contact_problem.evaluate(&next, &trial_lambda, true)?;
                        let balance: Vec<_> = trial
                            .gradient
                            .iter()
                            .zip(&reaction.force)
                            .map(|(g, f)| g - f)
                            .collect();
                        let merit = contact_residual::merit(
                            &balance,
                            &reaction.gaps,
                            &trial_lambda,
                            1e-7,
                            TOL,
                        )?;
                        trial_merit = merit;
                        merit <= (1. - 1e-4 * alpha) * old_merit.unwrap()
                    } else {
                        safe && objective(&p)
                            .is_ok_and(|e| e <= s.energy + 1e-4 * alpha * slope.min(0.) + 1e-14)
                    };
                    if args.audit_iterations {
                        line_search_trace.push(json!({"method":name,"backtrack":backtrack,
                            "alpha":alpha,"safe":safe,"decrease":decrease,
                            "merit":if safe && args.discrete_forces {Some(trial_merit)} else {None},
                            "blocked_by":blocked_by}));
                    }
                    if decrease {
                        if best.as_ref().is_none_or(|b| trial_merit < b.merit) {
                            best = Some(AcceptedTrial {
                                positions: p,
                                multipliers: trial_lambda,
                                alpha,
                                start_alpha,
                                backtracks: backtrack,
                                guard_vertex,
                                merit: trial_merit,
                                method: name,
                            });
                        }
                        break;
                    }
                    alpha *= 0.5;
                }
            }
            if let Some(trial) = best {
                minimum_alpha = minimum_alpha.min(trial.alpha);
                if trial.method != "qp" {
                    newton_steps += 1;
                }
                if let Some(entry) = iteration_trace.last_mut() {
                    entry["accepted_alpha"] = json!(trial.alpha);
                    entry["start_alpha"] = json!(trial.start_alpha);
                    entry["backtracks"] = json!(trial.backtracks);
                    entry["guard_vertex"] = json!(trial.guard_vertex);
                    entry["accepted_method"] = json!(trial.method);
                    entry["accepted_merit"] = json!(trial.merit);
                    if trial.backtracks > 0 {
                        entry["line_search"] = json!(line_search_trace);
                    }
                }
                state.x = trial.positions;
                lambda_history = trial.multipliers;
            } else {
                failed_nonlinear = Some(json!({"step":step,"iteration":iteration,
                    "x":state.x,"lambda":lambda_history,"lambda_target":lambda_target,
                    "directions":directions,"old_merit":old_merit,"iterations":iteration_trace,"inexact_search_qps":inexact_search_qps,"newton_steps":newton_steps,"newton_rejections":newton_rejections,
                    "line_search":line_search_trace}));
                failure = Some(format!(
                    "Line search failed at step {step}, iteration {iteration}, residual {residual}"
                ));
                break 'steps;
            }
        }
        let x = reference.world.positions(&state.x);
        reference.world.validate_start(&x)?;
        for ((v, p), old) in state.v.iter_mut().zip(&state.x).zip(&old) {
            *v = (*p - *old) / DT;
        }
        state.time += DT;
        let material = energy_only(&state.x, &state.x)?;
        let kinetic: f64 = state
            .v
            .iter()
            .zip(&g.mesh.masses)
            .map(|(v, m)| 0.5 * m * v.length_squared())
            .sum();
        let gravity: f64 = state
            .x
            .iter()
            .zip(&g.mesh.masses)
            .map(|(p, m)| m * 9.81 * p.y)
            .sum();
        let energy = material + kinetic + gravity;
        let energy_balance = final_forces.map(|forces: ForceAudit| {
            let work = |force: &[f64]| -> f64 {
                force
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .zip(state.x.iter().zip(&old))
                    .map(|(f, (x, old))| DVec3::from_slice(f).dot(*x - *old))
                    .sum()
            };
            let material_remainder = material - material_previous - work(&forces.material);
            let contact_work = work(&forces.contact);
            let residual_work = work(&forces.residual);
            let dissipation: f64 = state
                .v
                .iter()
                .zip(&old_v)
                .zip(&g.mesh.masses)
                .map(|((v, old), m)| 0.5 * m * (*v - *old).length_squared())
                .sum();
            let grab_work = if grabbing {
                -10. * (state.x[control_vertex] - control)
                    .dot(state.x[control_vertex] - old[control_vertex])
            } else {
                0.
            };
            let predicted =
                material_remainder + contact_work + residual_work + grab_work - dissipation;
            json!({"change_j":energy-energy_previous,"material_remainder_j":material_remainder,
                "contact_work_j":contact_work,"residual_work_j":residual_work,
                "grab_work_j":grab_work,"implicit_dissipation_j":dissipation,
                "closure_error_j":energy-energy_previous-predicted})
        });
        if step >= release {
            max_rise = max_rise.max(energy - energy_previous);
        }
        energy_previous = energy;
        material_previous = material;
        let seam_energy: f64 = g.seams.iter().map(|seam| seam.energy(&state.x)).sum();
        trace.push(json!({"step":step,"converged":converged,"iterations":iterations,"iteration_trace":iteration_trace,"residual_n":residual,"seam_energy_j":seam_energy,
            "complementarity_j":complementarity,"qp_sweeps":qp_sweeps,"singular_block_fallbacks":fallbacks,"reduced_active_blocks":reduced_blocks,"boundary_pivots":boundary_pivots,
            "schur_entries_computed":schur_computed,"schur_cache_hits":schur_hits,"schur_max_bytes":schur_max_bytes,
            "added_rows":added_rows,"repair_solves":repair_solves,"repair_failures":repair_failures,"minimum_alpha":minimum_alpha,"max_reaction_counts_vf_ee":reaction_counts,"certificate_intervals":certificate_intervals,"certificate_rejections":certificate_rejections,
            "minimum_pair_distance_m":pairs.iter().map(|p|p.distance(&top,&x)).fold(f64::INFINITY,f64::min),
            "minimum_floor_height_m":state.x.iter().map(|p|p.y).fold(f64::INFINITY,f64::min),
            "inexact_search_qps":inexact_search_qps,"newton_steps":newton_steps,"newton_rejections":newton_rejections,"energy_j":energy,"energy_balance":energy_balance,"reactions":reaction_trace,"kinetic_j":kinetic,"material_j":material,"x":state.x,"v":state.v}));
        if args.progress && (step % 10 == 0 || !converged) {
            eprintln!(
                "step={step}, converged={converged}, iterations={iterations}, force={residual:e} N"
            );
        }
        if !converged && args.stop_on_unconverged {
            failure = Some(format!(
                "Nonlinear budget exhausted at step {step}: residual {residual}"
            ));
            break;
        }
    }
    let result = json!({"moving_plane_certificates":args.moving_plane_certificates,"reuse_newton_samples":args.reuse_newton_samples,"newton_path_repair":args.newton_path_repair,"constrained_newton":args.constrained_newton,"fallback_bounded":args.fallback_bounded,"bounded_newton":args.bounded_newton,"combined_newton":args.combined_newton,"natural_newton":args.natural_newton,"unilateral_newton":args.unilateral_newton,"rank_newton":args.rank_newton,"newton_forces":args.newton_forces,"force_law":if args.discrete_forces { "invariant-discrete" } else { "endpoint" },"held":args.held,"release_step":release,"full_energy_assembly":args.full_energy_assembly,"sewn_ease":args.sewn_ease,"fold_degrees":args.sewn_ease.map(|_|args.fold_degrees),"seams":g.seams.len(),"dual_method":method,"swept_repair":args.swept_repair,"accumulate_repair":args.accumulate_repair,"iterations_budget":args.iterations,"vertices":n,"pairs":pairs.len(),"requested_steps":args.steps,
        "first_step":first_step,"completed_steps":trace.len(),"failure":failure,"captured_newton":failed_newton,"failed_nonlinear":failed_nonlinear,"failed_qp":failed_qp,"failed_repair_qp":failed_repair_qp,"max_post_release_energy_rise_j":max_rise,
        "elapsed_ms":start.elapsed().as_secs_f64()*1000.,"frames":trace});
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(args.output, serde_json::to_vec_pretty(&result).unwrap())
        .map_err(|e| e.to_string())?;
    println!(
        "steps={}, failure={}",
        result["completed_steps"], result["failure"]
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sewn_fixture_keeps_material_rest_and_initial_contact_feasibility() {
        for ease in [0., 0.1] {
            let flat = template(Some(ease), 0.).unwrap().compile().unwrap();
            let folded = Arc::new(template(Some(ease), 20.).unwrap().compile().unwrap());
            assert_eq!(flat.mesh.uv, folded.mesh.uv);
            assert_eq!(flat.mesh.masses, folded.mesh.masses);
            assert_eq!(folded.mesh.uv.len(), 16);
            assert!(!folded.seams.is_empty());
            assert!(folded.mesh.pins.iter().all(Option::is_none));
            let mut world = VbdSolver::new(folded.clone(), None, &folded.initial_state)
                .unwrap()
                .world;
            let x = world.positions(&folded.initial_state.x);
            world.validate_start(&x).unwrap();
            let pairs = world.feature_hierarchy(&x, 4.).unwrap().pairs;
            let mut vf = 0;
            let mut ee = 0;
            for pair in pairs {
                match pair {
                    Pair::VertexFace(_) => vf += 1,
                    Pair::EdgeEdge(_) => ee += 1,
                    _ => continue,
                }
                assert!(pair.distance(&world.topology, &x) >= GAP - TOL);
            }
            assert!(vf > 0 && ee > 0);
            assert!(x.iter().all(|p| p.y >= GAP - TOL));
        }
    }
}

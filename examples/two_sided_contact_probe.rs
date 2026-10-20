//! Static flat-patch force gate for an independent two-sided contact trial.
use clap::Parser;
use glam::{DQuat, DVec3};
use marvelous_cloth::{
    math::{Jet, Scalar},
    ogc::{
        feature_hierarchy::{Hierarchy, Pair},
        potential::activation,
        topology::Topology,
        two_sided_contact::{MinimumContact, Settings},
    },
};
use serde_json::json;
use std::{fs, path::PathBuf};
#[path = "support/two_sided_fixture.rs"]
mod two_sided_fixture;

fn patch_sweeps() -> Result<Vec<serde_json::Value>, String> {
    let mut rows = Vec::new();
    for angle in [0., 0.37] {
        let q = DQuat::from_rotation_y(angle);
        let mut reference = vec![
            [-0.01, 0.001, -0.01],
            [0.01, 0.001, 0.01],
            [0.01, 0.001, -0.01],
            [-0.01, 0.001, 0.01],
            [-0.1, 0., -0.1],
            [0.1, 0., 0.1],
            [0.1, 0., -0.1],
            [-0.1, 0., 0.1],
        ]
        .into_iter()
        .map(DVec3::from_array)
        .collect::<Vec<_>>();
        for p in &mut reference[..4] {
            *p = q * *p;
        }
        let top = Topology::new(8, vec![[0, 1, 2], [1, 0, 3], [4, 5, 6], [5, 4, 7]])?;
        let model = MinimumContact::new(&top);
        // Deliberately overcomplete, fixed pair list for the entire sweep.
        // No detection churn or omitted force candidates can explain a bump.
        let hierarchy = Hierarchy::build(&top, &reference, 4, &Default::default(), 1.)?;
        let area_scale = 0.02;
        let vw = top
            .neighbors
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let mean = n
                    .iter()
                    .map(|j| reference[i].distance(reference[*j]))
                    .sum::<f64>()
                    / n.len() as f64;
                mean * mean / area_scale
            })
            .collect::<Vec<_>>();
        let ew = top
            .edges
            .iter()
            .map(|[a, b]| reference[*a].distance_squared(reference[*b]) / area_scale)
            .collect::<Vec<_>>();
        let fw = top
            .triangles
            .iter()
            .map(|[a, b, c]| {
                0.5 * (reference[*b] - reference[*a])
                    .cross(reference[*c] - reference[*a])
                    .length()
                    / area_scale
            })
            .collect::<Vec<_>>();
        for alpha in [0.2, 0.5] {
            for measures in [false, true] {
                let cfg = Settings {
                    radius: 0.003,
                    stiffness: 100_000.,
                    width: 0.01,
                    alpha,
                };
                let mut max_lateral: f64 = 0.;
                let mut min_normal = f64::INFINITY;
                let mut max_normal = f64::NEG_INFINITY;
                let mut min_energy = f64::INFINITY;
                let mut max_energy = f64::NEG_INFINITY;
                let mut active_max = [0usize; 4];
                for i in 0..1001 {
                    let slide = -0.025 + i as f64 * 0.00005;
                    let x = reference
                        .iter()
                        .enumerate()
                        .map(|(v, p)| {
                            std::array::from_fn(|a| {
                                let value = p[a] + if v < 4 && a == 0 { slide } else { 0. };
                                if v < 4 {
                                    Jet::<3>::variable(value, a)
                                } else {
                                    Jet::c(value)
                                }
                            })
                        })
                        .collect::<Vec<_>>();
                    let mut total = Jet::c(0.);
                    let mut active = [0usize; 4];
                    for &pair in &hierarchy.pairs {
                        let w = if !measures {
                            1.
                        } else {
                            match pair {
                                Pair::VertexFace([v, f]) => vw[v] * fw[f],
                                Pair::EdgeEdge([e, f]) => ew[e] * ew[f],
                                Pair::VertexEdge([v, e]) => vw[v] * ew[e],
                                Pair::VertexVertex([a, b]) => vw[a] * vw[b],
                            }
                        };
                        let e = model.energy(pair, &x, w, cfg)?;
                        if e.v > 0. {
                            active[pair.kind_index()] += 1;
                        }
                        total = total + e;
                    }
                    if !total.v.is_finite() || total.g.iter().any(|v| !v.is_finite()) {
                        return Err("Nonfinite full-patch energy/gradient".into());
                    }
                    for k in 0..4 {
                        active_max[k] = active_max[k].max(active[k]);
                    }
                    max_lateral = max_lateral.max(total.g[0].hypot(total.g[2]));
                    min_normal = min_normal.min(-total.g[1]);
                    max_normal = max_normal.max(-total.g[1]);
                    min_energy = min_energy.min(total.v);
                    max_energy = max_energy.max(total.v);
                }
                rows.push(json!({"source_rotation_rad":angle,"alpha":alpha,
                    "reference_measure_weights":measures,"fixed_candidate_count":hierarchy.pairs.len(),
                    "max_active_pairs_vf_ee_ve_vv":active_max,
                    "maximum_lateral_force_n":max_lateral,"minimum_normal_force_n":min_normal,
                    "maximum_normal_force_n":max_normal,"minimum_energy_j":min_energy,"maximum_energy_j":max_energy}));
            }
        }
    }
    Ok(rows)
}

#[derive(Parser)]
struct Args {
    #[arg(long)]
    output: PathBuf,
}
fn geometry(x: f64) -> [[Jet<3>; 3]; 5] {
    let p = [
        [x, 0.001, 0.],
        [-0.1, 0., -0.1],
        [0.1, 0., 0.1],
        [0.1, 0., -0.1],
        [-0.1, 0., 0.1],
    ];
    std::array::from_fn(|i| {
        std::array::from_fn(|a| {
            if i == 0 {
                Jet::variable(p[i][a], a)
            } else {
                Jet::c(p[i][a])
            }
        })
    })
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.output.exists() {
        return Err("Choose a fresh output".into());
    }
    let reference = activation(Jet::<1>::variable(0.001, 0), 0.003, 100_000.);
    let mut results = Vec::new();
    for alpha in [0.05, 0.2, 0.5, 0.8] {
        for measures in [false, true] {
            let cfg = Settings {
                radius: 0.003,
                stiffness: 100_000.,
                width: 0.01,
                alpha,
            };
            let mut max_lateral: f64 = 0.;
            let mut min_normal = f64::INFINITY;
            let mut max_normal = f64::NEG_INFINITY;
            let mut min_normal_x = 0.;
            let mut max_error: f64 = 0.;
            let mut peak_x = 0.;
            let mut min_energy = f64::INFINITY;
            let mut max_energy = f64::NEG_INFINITY;
            for i in 0..8001 {
                let x = -0.004 + i as f64 * 0.000001;
                let j = two_sided_fixture::energy(geometry(x), cfg, measures);
                if !j.v.is_finite()
                    || j.g.iter().any(|v| !v.is_finite())
                    || j.h.iter().flatten().any(|v| !v.is_finite())
                {
                    return Err(format!("Nonfinite kernel at x={x}, alpha={alpha}").into());
                }
                let lateral = j.g[0].hypot(j.g[2]);
                if lateral > max_lateral {
                    max_lateral = lateral;
                    peak_x = x;
                }
                if -j.g[1] < min_normal {
                    min_normal = -j.g[1];
                    min_normal_x = x;
                }
                max_normal = max_normal.max(-j.g[1]);
                max_error = max_error.max((j.v - reference.v).abs());
                min_energy = min_energy.min(j.v);
                max_energy = max_energy.max(j.v);
            }
            let limits=[1e-5,1e-6,1e-7,1e-8,1e-9].map(|eps| {
                let a=two_sided_fixture::energy(geometry(-eps),cfg,measures);
                let b=two_sided_fixture::energy(geometry(eps),cfg,measures);
                json!({"epsilon_m":eps,"energy_difference_j":(a.v-b.v).abs(),
                    "force_difference_n":a.g.iter().zip(b.g).map(|(a,b)|(a-b).powi(2)).sum::<f64>().sqrt()})
            });
            results.push(json!({"alpha":alpha,"reference_measure_weights":measures,
                "maximum_lateral_force_n":max_lateral,"peak_position_x_m":peak_x,
                "minimum_normal_force_n":min_normal,"maximum_normal_force_n":max_normal,
                "minimum_normal_force_position_x_m":min_normal_x,
                "maximum_energy_error_j":max_error,"minimum_energy_j":min_energy,
                "maximum_energy_j":max_energy,"shared_edge_limits":limits}));
        }
    }
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        args.output,
        serde_json::to_vec_pretty(&json!({
            "scope":"Independent two-sided minimum-factor trial, not full GCP or production",
            "reference_energy_j":reference.v,"reference_normal_force_n":-reference.g[0],
            "grid_spacing_m":0.000001,"width":0.01,"results":results,
            "full_patch_grid_spacing_m":0.00005,"full_patch_results":patch_sweeps()?
        }))?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn positive_continuous_trial_still_fails_flat_sliding_and_repulsion() {
        let cfg = Settings {
            radius: 0.003,
            stiffness: 100_000.,
            width: 0.01,
            alpha: 0.05,
        };
        for measures in [false, true] {
            let mut lateral: f64 = 0.;
            let mut normal = f64::INFINITY;
            for i in 0..401 {
                let j = two_sided_fixture::energy(
                    geometry(-0.001 + i as f64 * 0.000005),
                    cfg,
                    measures,
                );
                assert!(j.v >= 0. && j.v.is_finite());
                lateral = lateral.max(j.g[0].hypot(j.g[2]));
                normal = normal.min(-j.g[1]);
            }
            // Negative control: do not promote this model on positivity or
            // contact-refresh continuity alone. No force clamping is applied.
            assert!(lateral > 1000. && normal < -1.);
            let mut previous = f64::INFINITY;
            for eps in [1e-6, 1e-7, 1e-8, 1e-9] {
                let l = two_sided_fixture::energy(geometry(-eps), cfg, measures);
                let r = two_sided_fixture::energy(geometry(eps), cfg, measures);
                let diff =
                    l.g.iter()
                        .zip(r.g)
                        .map(|(a, b)| (a - b).powi(2))
                        .sum::<f64>()
                        .sqrt();
                assert!(diff < previous * 0.2);
                previous = diff;
            }
        }
    }
    #[test]
    fn full_surface_patch_keeps_spurious_sliding_force_with_fixed_candidates() {
        for row in patch_sweeps().unwrap() {
            assert!(row["maximum_lateral_force_n"].as_f64().unwrap() > 100.);
            assert!(
                row["maximum_energy_j"].as_f64().unwrap()
                    > 1.2 * row["minimum_energy_j"].as_f64().unwrap()
            );
            assert!(row["max_active_pairs_vf_ee_ve_vv"][1].as_u64().unwrap() > 0);
        }
    }
}

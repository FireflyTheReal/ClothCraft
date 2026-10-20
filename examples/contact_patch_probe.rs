//! Isolated quadrature gate, not a cloth backend or production VBD benchmark.
//! Same backward-Euler objective and OGC movement guards; fixed planar patch,
//! one moving point, no damping, friction, sleep, or velocity correction.
use clap::Parser;
use glam::{DMat3, DVec3};
use marvelous_cloth::{
    math::{Jet, Scalar},
    ogc::{
        OgcSettings, OgcWorld,
        patch_contact::{face_pair, wire_pair},
        potential::{Contact, Kind, activation},
        smooth_forest,
    },
};
use serde_json::json;
use std::{fs, path::PathBuf};
#[path = "support/two_sided_fixture.rs"]
mod two_sided_fixture;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    output: PathBuf,
    /// Additional opt-in smooth-forest energy scales (joules).
    #[arg(long, value_delimiter = ',')]
    smooth_tau: Vec<f64>,
    /// Also append an inactive but connected edge to each smooth wire.
    #[arg(long)]
    extend_wire: bool,
    /// Opt-in two-sided local-minimum trial, with unit and reference measures.
    #[arg(long, value_delimiter = ',')]
    two_sided_alpha: Vec<f64>,
    /// Opt-in local-plane field support radii in metres.
    #[arg(long, value_delimiter = ',')]
    local_support: Vec<f64>,
}
#[derive(Clone, Copy, Debug)]
enum Law {
    Plane,
    PrimitiveSum,
    FacePair,
    WireSum,
    WirePair,
    WireSmooth(f64),
    WireSmoothExtended(f64),
    TwoSided { alpha: f64, measures: bool },
    LocalSurface(f64),
}
fn energy<S: Scalar>(law: Law, x: [[S; 3]; 5], cfg: &OgcSettings) -> S {
    match law {
        Law::LocalSurface(support) => {
            use marvelous_cloth::ogc::local_surface::{self, Sample};
            let samples = [
                Sample::triangle([x[1], x[2], x[3]], 1.).unwrap(),
                Sample::triangle([x[2], x[1], x[4]], 1.).unwrap(),
            ];
            local_surface::energy(x[0], &samples, support, 1., cfg.radius, cfg.stiffness)
                .expect("Local surface trial left its oriented support")
        }
        Law::TwoSided { alpha, measures } => two_sided_fixture::energy(
            x,
            marvelous_cloth::ogc::two_sided_contact::Settings {
                radius: cfg.radius,
                stiffness: cfg.stiffness,
                width: 0.01,
                alpha,
            },
            measures,
        ),
        Law::Plane => activation(x[0][1], cfg.radius, cfg.stiffness),
        Law::FacePair => face_pair(x, cfg.radius, cfg.stiffness),
        Law::WireSum | Law::WirePair | Law::WireSmooth(_) | Law::WireSmoothExtended(_) => {
            // A transverse segment translating rigidly with the probe point.
            // Both endpoints have its height. The same planar OGC movement
            // guards therefore keep the whole source segment above the plane.
            let p = x[0];
            let wire = [
                [p[0], p[1], p[2] - S::c(0.01)],
                [p[0], p[1], p[2] + S::c(0.01)],
                [S::c(-0.1), S::c(0.), S::c(0.)],
                [S::c(0.), S::c(0.), S::c(0.)],
                [S::c(0.1), S::c(0.), S::c(0.)],
            ];
            if let Law::WireSmooth(tau) | Law::WireSmoothExtended(tau) = law {
                let mut positions = wire.to_vec();
                let mut targets = vec![[2, 3], [3, 4]];
                if matches!(law, Law::WireSmoothExtended(_)) {
                    positions.push([S::c(0.2), S::c(0.), S::c(0.)]);
                    targets.push([4, 5]);
                }
                smooth_forest::wire(&positions, [0, 1], &targets, cfg.radius, cfg.stiffness, tau)
                    .expect("Smooth forest evaluation failed in the research probe")
                    .energy
            } else if matches!(law, Law::WirePair) {
                wire_pair(wire, cfg.radius, cfg.stiffness)
            } else {
                [[0, 1, 2, 3], [0, 1, 3, 4]]
                    .into_iter()
                    .map(|vertices| {
                        Contact {
                            kind: Kind::SegmentSegment,
                            vertices,
                            count: 4,
                            // Transverse nonparallel edges: unit mollifier.
                            mollifier_epsilon: 1e-30,
                        }
                        .energy(
                            vertices.map(|i| wire[i]),
                            cfg.radius,
                            cfg.stiffness,
                        )
                    })
                    .fold(S::c(0.), |a, b| a + b)
            }
        }
        Law::PrimitiveSum => [[0, 1, 2, 3], [0, 2, 1, 4]]
            .into_iter()
            .map(|vertices| {
                Contact {
                    kind: Kind::PointTriangle,
                    vertices,
                    count: 4,
                    mollifier_epsilon: 1.,
                }
                .energy(vertices.map(|i| x[i]), cfg.radius, cfg.stiffness)
            })
            .fold(S::c(0.), |a, b| a + b),
    }
}
fn positions(p: DVec3) -> [DVec3; 5] {
    [
        p,
        DVec3::new(-0.1, 0., -0.1),
        DVec3::new(0.1, 0., 0.1),
        DVec3::new(0.1, 0., -0.1),
        DVec3::new(-0.1, 0., 0.1),
    ]
}
fn contact_jet(law: Law, x: [DVec3; 5], cfg: &OgcSettings) -> Jet<3> {
    energy(
        law,
        std::array::from_fn(|i| {
            std::array::from_fn(|a| {
                if i == 0 {
                    Jet::<3>::variable(x[i][a], a)
                } else {
                    Jet::c(x[i][a])
                }
            })
        }),
        cfg,
    )
}
fn run(law: Law, hold: bool, cfg: &OgcSettings) -> Result<serde_json::Value, String> {
    let dt: f64 = 1. / 60.;
    let mass = 0.02;
    let inertia = mass / dt.powi(2);
    // A constant external preload, not an invented garment mass. This fixes
    // an analytical plane equilibrium at 1 mm in the logarithmic barrier.
    let load = cfg.stiffness * (cfg.radius * 0.5).powi(2) / 0.001;
    let mut x = positions(DVec3::new(-0.004, 0.001, 0.));
    let mut v = DVec3::ZERO;
    let mut world = OgcWorld::new(&x, vec![[1, 2, 3], [2, 1, 4]], 1, [])?;
    world.directional_bounds = true;
    let mut trace = Vec::new();
    let release = if hold { 240 } else { 180 };
    let mut max_residual: f64 = 0.;
    let mut max_energy_rise: f64 = 0.;
    let mut previous_energy = None;
    let mut min_gap = f64::INFINITY;
    let mut truncations = 0;
    let mut unconverged_steps = 0;
    let mut line_search_failures = 0;
    let mut stopped_at_patch_boundary = false;
    for step in 0..480 {
        let old = x[0];
        let target = old + v * dt;
        let grab = (60..release).contains(&step).then(|| {
            let t = ((step - 60 + 1) as f64 / 120.).min(1.);
            DVec3::new(-0.004 + 0.008 * t, 0.001, 0.)
        });
        let objective = |p: DVec3| {
            energy(law, positions(p).map(|p| p.to_array()), cfg)
                + load * p.y
                + 0.5 * inertia * (p - target).length_squared()
                + grab.map_or(0., |g| 2500. * (p - g).length_squared())
        };
        for _ in 0..64 {
            let j = contact_jet(law, x, cfg);
            let mut g = DVec3::from_array(j.g) + (x[0] - target) * inertia + DVec3::Y * load;
            let mut h =
                DMat3::from_cols_array(&std::array::from_fn::<_, 9, _>(|i| j.h[i % 3][i / 3]))
                    + DMat3::IDENTITY * inertia;
            if let Some(grab) = grab {
                g += (x[0] - grab) * 5000.;
                h += DMat3::IDENTITY * 5000.;
            }
            if g.length() < 1e-6 {
                break;
            }
            // Same shifted positive local direction as production VBD.
            let lower = (0..3)
                .map(|i| {
                    h.col(i)[i]
                        - (0..3)
                            .filter(|&a| a != i)
                            .map(|a| h.col(i)[a].abs())
                            .sum::<f64>()
                })
                .fold(f64::INFINITY, f64::min);
            if lower < 1e-8 {
                h += DMat3::IDENTITY * (1e-8 - lower + 0.01 * inertia);
            }
            let delta = -(h.inverse() * g);
            let guards = world.detect(&x, 2. * cfg.radius, cfg)?;
            let before = objective(x[0]);
            let mut accepted = false;
            for backtrack in 0..32 {
                let (p, clipped) =
                    guards.truncate_step(0, x[0], x[0] + delta * 0.5_f64.powi(backtrack));
                truncations += usize::from(clipped);
                let e = objective(p);
                if p.is_finite() && e.is_finite() && e <= before + 1e-14 {
                    x[0] = p;
                    accepted = true;
                    break;
                }
            }
            if !accepted {
                line_search_failures += 1;
                break;
            }
        }
        // Measure at the accepted endpoint, including an unfinished solve.
        let final_residual = (DVec3::from_array(contact_jet(law, x, cfg).g)
            + (x[0] - target) * inertia
            + DVec3::Y * load
            + grab.map_or(DVec3::ZERO, |g| (x[0] - g) * 5000.))
        .length();
        unconverged_steps += usize::from(final_residual >= 1e-6);
        max_residual = max_residual.max(final_residual);
        v = (x[0] - old) / dt;
        min_gap = min_gap.min(x[0].y);
        if x[0].y <= 0. {
            return Err(format!("{law:?} step {step}: crossed the guarded plane"));
        }
        stopped_at_patch_boundary = x[0].x.abs() >= 0.09 || x[0].z.abs() >= 0.09;
        let mechanical = 0.5 * mass * v.length_squared()
            + load * x[0].y
            + energy(law, x.map(|p| p.to_array()), cfg);
        if step >= release {
            if let Some(previous) = previous_energy {
                max_energy_rise = max_energy_rise.max(mechanical - previous);
            }
            previous_energy = Some(mechanical);
        }
        trace.push(json!({"step":step,"position_m":x[0],"velocity_m_s":v,
            "residual_n":final_residual,"mechanical_energy_j":mechanical}));
        if stopped_at_patch_boundary {
            break;
        }
    }
    Ok(
        json!({"law":format!("{law:?}"),"hold":hold,"release_step":release,
        "tau_j":if let Law::WireSmooth(tau) | Law::WireSmoothExtended(tau) = law { Some(tau) } else { None },
        "dt_s":dt,"mass_kg":mass,"normal_load_n":load,"grab_stiffness_n_m":5000.,
        "minimum_gap_m":min_gap,"max_residual_n":max_residual,
        "unconverged_steps":unconverged_steps,"line_search_failures":line_search_failures,
        "stopped_at_patch_boundary":stopped_at_patch_boundary,
        "max_post_release_energy_rise_j":max_energy_rise,"truncations":truncations,
        "trace":trace}),
    )
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.smooth_tau.iter().any(|t| !t.is_finite() || *t <= 0.) {
        return Err("Smooth tau must be finite and positive".into());
    }
    if args
        .two_sided_alpha
        .iter()
        .any(|a| !a.is_finite() || *a <= 0. || *a > 1.)
    {
        return Err("Two-sided alpha must be in (0, 1]".into());
    }
    if args
        .local_support
        .iter()
        .any(|r| !r.is_finite() || *r <= 0.)
    {
        return Err("Local surface support must be finite and positive".into());
    }
    if args.output.exists() {
        return Err("Choose a new output path".into());
    }
    let cfg = OgcSettings::default();
    let mut runs = Vec::new();
    for hold in [false, true] {
        for law in [
            Law::Plane,
            Law::PrimitiveSum,
            Law::FacePair,
            Law::WireSum,
            Law::WirePair,
        ] {
            runs.push(run(law, hold, &cfg)?);
        }
        for &tau in &args.smooth_tau {
            runs.push(run(Law::WireSmooth(tau), hold, &cfg)?);
            if args.extend_wire {
                runs.push(run(Law::WireSmoothExtended(tau), hold, &cfg)?);
            }
        }
        for &alpha in &args.two_sided_alpha {
            for measures in [false, true] {
                runs.push(run(Law::TwoSided { alpha, measures }, hold, &cfg)?);
            }
        }
        for &support in &args.local_support {
            runs.push(run(Law::LocalSurface(support), hold, &cfg)?);
        }
    }
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        args.output,
        serde_json::to_vec_pretty(&json!({
            "scope":"Isolated fixed two-face patch. Not a cloth or performance benchmark.",
            "contact":cfg,"runs":runs
        }))?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compare(hold: bool) {
        let cfg = OgcSettings::default();
        let reference = run(Law::Plane, hold, &cfg).unwrap();
        // These finite tau values have a measured nonzero sliding bias.
        // This trajectory tolerance is not a claim of exact frictionlessness.
        for law in [
            Law::FacePair,
            Law::WirePair,
            Law::WireSmooth(1e-7),
            Law::WireSmooth(1e-9),
            Law::WireSmoothExtended(1e-7),
            Law::WireSmoothExtended(1e-9),
            Law::LocalSurface(0.05),
            Law::LocalSurface(0.08),
            Law::LocalSurface(0.2),
        ] {
            let candidate = run(law, hold, &cfg).unwrap();
            assert_eq!(candidate["unconverged_steps"], 0);
            assert_eq!(candidate["line_search_failures"], 0);
            assert_eq!(candidate["stopped_at_patch_boundary"], false);
            assert!(candidate["minimum_gap_m"].as_f64().unwrap() > 0.);
            assert!(
                candidate["max_post_release_energy_rise_j"]
                    .as_f64()
                    .unwrap()
                    < 1e-10
            );
            let a = reference["trace"].as_array().unwrap();
            let b = candidate["trace"].as_array().unwrap();
            assert_eq!(b.len(), 480);
            for (a, b) in a.iter().zip(b) {
                for (key, tolerance) in [("position_m", 1e-8), ("velocity_m_s", 1e-6)] {
                    for axis in 0..3 {
                        assert!(
                            (a[key][axis].as_f64().unwrap() - b[key][axis].as_f64().unwrap()).abs()
                                < tolerance
                        );
                    }
                }
            }
            // Releasing a moving handle retains the horizontal velocity:
            // the candidate cannot pass by freezing or damping the point.
            if !hold {
                assert!(candidate["trace"][180]["velocity_m_s"][0].as_f64().unwrap() > 0.0039);
                assert!(candidate["trace"][479]["velocity_m_s"][0].as_f64().unwrap() > 0.0039);
            }
        }
    }
    #[test]
    fn release_in_motion_matches_frictionless_reference_without_damping() {
        compare(false);
    }
    #[test]
    fn held_release_matches_frictionless_reference_without_damping() {
        compare(true);
    }
}

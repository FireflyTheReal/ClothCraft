//! CPU research only: hard point/surface inequalities, NOT a cloth backend.
//! Original OGC movement guards plus finite-triangle swept clearance checks.
//! Compares analytical, all-linearized and constraint-generation contact.
use clap::Parser;
use glam::{DMat3, DVec3};
use marvelous_cloth::{
    ogc::{
        OgcSettings, OgcWorld,
        geometry::{closest_segments, closest_triangle, segment_triangle},
        topology::Topology,
    },
    solver_lab::contact_qp::{self, Row, Settings},
};
use serde::Serialize;
use std::{fs, path::PathBuf, time::Instant};

const GAP: f64 = 0.001;
const DT: f64 = 1. / 60.;
const MASS: f64 = 0.02;
const LOAD: f64 = 225.;
const FEAS: f64 = 1e-11;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    output: PathBuf,
}
#[derive(Clone, Copy, Debug, Serialize)]
enum Mode {
    Plane,
    AllLinearized,
    WorkingSet,
}
#[derive(Clone, Copy, Debug, Serialize)]
enum Fixture {
    PointTriangles,
    TransverseWire,
}
impl Fixture {
    fn count(self, cells: usize, top: &Topology) -> usize {
        match self {
            Self::PointTriangles => top.triangles.len(),
            Self::TransverseWire => cells * 2,
        }
    }
    fn segment(index: usize, cells: usize) -> [DVec3; 2] {
        let a = -0.1 + 0.2 * index as f64 / (cells * 2) as f64;
        let b = -0.1 + 0.2 * (index + 1) as f64 / (cells * 2) as f64;
        [DVec3::new(a, 0., 0.), DVec3::new(b, 0., 0.)]
    }
    fn separation(self, p: DVec3, i: usize, cells: usize, x: &[DVec3], top: &Topology) -> DVec3 {
        match self {
            Self::PointTriangles => p - closest_triangle(p, i, x, top).0,
            Self::TransverseWire => {
                let [a, b] = [p - 0.01 * DVec3::Z, p + 0.01 * DVec3::Z];
                let [c, d] = Self::segment(i, cells);
                let (s, t) = closest_segments(a, b, c, d);
                a.lerp(b, s) - c.lerp(d, t)
            }
        }
    }
    fn swept(self, path: [DVec3; 2], i: usize, cells: usize, x: &[DVec3], top: &Topology) -> f64 {
        let [a, b] = path;
        match self {
            Self::PointTriangles => swept_distance(a, b, i, x, top),
            Self::TransverseWire => {
                // For this translating transverse wire, the swept surface
                // includes z=0 throughout and the target lies in z=0. Its
                // minimum distance is exactly the center path/target distance.
                assert!(a.z.abs() < 1e-14 && b.z.abs() < 1e-14);
                if a.distance_squared(b) <= 1e-24 {
                    return self
                        .separation(a, i, cells, x, top)
                        .length()
                        .min(self.separation(b, i, cells, x, top).length());
                }
                let [c, d] = Self::segment(i, cells);
                let (s, t) = closest_segments(a, b, c, d);
                a.lerp(b, s).distance(c.lerp(d, t))
            }
        }
    }
}
#[derive(Serialize)]
struct Frame {
    step: usize,
    position: DVec3,
    velocity: DVec3,
    energy_j: f64,
    swept_gap_m: f64,
    iterations: usize,
    qp_sweeps: usize,
    qp_displacement_m: f64,
    converged: bool,
}
#[derive(Serialize)]
struct Run {
    mode: Mode,
    fixture: Fixture,
    cells_per_axis: usize,
    row_copies: usize,
    held_release: bool,
    elapsed_ms: f64,
    max_reference_position_error_m: f64,
    max_reference_velocity_error_m_s: f64,
    max_post_release_energy_rise_j: f64,
    minimum_swept_gap_m: f64,
    unconverged_steps: usize,
    failure: Option<String>,
    frames: Vec<Frame>,
}
fn mesh(cells: usize) -> (Vec<DVec3>, Topology) {
    let mut x = vec![DVec3::new(-0.004, GAP, 0.)];
    for z in 0..=cells {
        for i in 0..=cells {
            x.push(DVec3::new(
                -0.1 + 0.2 * i as f64 / cells as f64,
                0.,
                -0.1 + 0.2 * z as f64 / cells as f64,
            ));
        }
    }
    let mut triangles = Vec::new();
    for z in 0..cells {
        for i in 0..cells {
            let a = 1 + z * (cells + 1) + i;
            let b = a + 1;
            let d = a + cells + 1;
            let c = d + 1;
            if (z + i) % 2 == 0 {
                triangles.extend([[a, c, b], [a, d, c]]);
            } else {
                triangles.extend([[a, d, b], [b, d, c]]);
            }
        }
    }
    let top = Topology::new(x.len(), triangles).unwrap();
    (x, top)
}
fn distance(p: DVec3, face: usize, x: &[DVec3], top: &Topology) -> f64 {
    p.distance(closest_triangle(p, face, x, top).0)
}
/// Finite segment/triangle distance: endpoints, triangle edges, intersection.
/// The segment is the point's actual accepted straight path. This is not a
/// general moving-triangle CCD implementation.
fn swept_distance(a: DVec3, b: DVec3, face: usize, x: &[DVec3], top: &Topology) -> f64 {
    let tri = top.triangles[face].map(|v| x[v]);
    if segment_triangle(a, b, tri) {
        return 0.;
    }
    let mut gap = distance(a, face, x, top).min(distance(b, face, x, top));
    if a.distance_squared(b) > 1e-24 {
        for k in 0..3 {
            let c = tri[k];
            let d = tri[(k + 1) % 3];
            let (s, t) = closest_segments(a, b, c, d);
            gap = gap.min(a.lerp(b, s).distance(c.lerp(d, t)));
        }
    }
    gap
}
fn run(
    mode: Mode,
    fixture: Fixture,
    cells: usize,
    copies: usize,
    held: bool,
) -> Result<Run, String> {
    let (mut x, top) = mesh(cells);
    let mut world = OgcWorld::new(&x, top.triangles.clone(), 1, [])?;
    world.directional_bounds = true;
    let cfg = OgcSettings::default();
    let count = fixture.count(cells, &top);
    let mut v = DVec3::ZERO;
    let release = if held { 240 } else { 180 };
    let mut frames = Vec::new();
    let start = Instant::now();
    let mut min_gap = GAP;
    let mut max_rise: f64 = 0.;
    let mut previous_energy = LOAD * GAP;
    let mut failure = None;
    'frames: for step in 0..480 {
        let before = x[0];
        let target = before + DT * v;
        let grab = if (60..release).contains(&step) {
            5000.
        } else {
            0.
        };
        let control = DVec3::new(
            -0.004 + 0.008 * ((step as f64 - 59.) / 120.).clamp(0., 1.),
            GAP,
            0.,
        );
        let inertia = MASS / (DT * DT);
        let h = DMat3::from_diagonal(DVec3::splat(inertia + grab));
        let mut converged = false;
        let mut iterations = 0;
        let mut sweeps = 0;
        let mut correction = f64::INFINITY;
        let mut step_gap = GAP;
        for iteration in 0..64 {
            iterations = iteration + 1;
            let p = x[0];
            let g = inertia * (p - target) + grab * (p - control) + LOAD * DVec3::Y;
            let mut active = match mode {
                Mode::Plane => Vec::new(),
                Mode::AllLinearized => (0..count).collect::<Vec<_>>(),
                Mode::WorkingSet => (0..count)
                    .filter(|&f| fixture.separation(p, f, cells, &x, &top).length() <= GAP + FEAS)
                    .collect(),
            };
            // Add violated finite constraints, never treat an absent row as
            // permission to ignore a collision. No index or seam-specific rule.
            let qp = loop {
                let rows = if matches!(mode, Mode::Plane) {
                    vec![Row {
                        normal: DVec3::Y,
                        offset: GAP - p.y,
                    }]
                } else {
                    active
                        .iter()
                        .flat_map(|&f| {
                            let d = fixture.separation(p, f, cells, &x, &top);
                            std::iter::repeat_n(
                                Row {
                                    normal: d.normalize(),
                                    offset: GAP - d.length(),
                                },
                                copies,
                            )
                        })
                        .collect()
                };
                let q = contact_qp::solve(
                    h,
                    g,
                    &rows,
                    None,
                    Settings {
                        max_sweeps: 4096,
                        feasibility_m: FEAS,
                        ..Settings::default()
                    },
                )?;
                sweeps += q.sweeps;
                if !q.converged {
                    failure = Some(format!(
                        "Unconverged inner QP: {mode:?}, frame {step}, iter {iteration}; feasibility={} m, stationarity={} N, complementarity={} J",
                        q.feasibility_m, q.stationarity_n, q.complementarity_j
                    ));
                    break 'frames;
                }
                let end = p + q.displacement;
                let missing = (0..count)
                    .filter(|f| !active.contains(f))
                    .filter(|&f| fixture.swept([p, end], f, cells, &x, &top) < GAP - FEAS)
                    .collect::<Vec<_>>();
                if !matches!(mode, Mode::WorkingSet) || missing.is_empty() {
                    break q;
                }
                active.extend(missing);
            };
            correction = qp.displacement.length();
            if correction < 1e-10 {
                converged = true;
                break;
            }
            let guards = world.detect(&x, 2. * cfg.radius, &cfg)?;
            let (next, _) = guards.truncate_step(0, p, p + qp.displacement);
            let path_gap = (0..count)
                .map(|f| fixture.swept([p, next], f, cells, &x, &top))
                .fold(f64::INFINITY, f64::min);
            if !path_gap.is_finite() || path_gap < GAP - FEAS {
                return Err(format!(
                    "Swept clearance violation: {path_gap}, {mode:?}, frame {step}"
                ));
            }
            step_gap = step_gap.min(path_gap);
            // The quadratic objective must decrease on the accepted, guarded
            // path. There is no contact potential to change on row refresh.
            let d = next - p;
            let objective_change = g.dot(d) + 0.5 * d.dot(h * d);
            if objective_change > 1e-11 {
                return Err(format!("Objective increased: {objective_change}"));
            }
            x[0] = next;
            // OGC's point/plane guards also protect the complete transverse
            // source wire here: it has constant y and stays above the plane.
            // This argument is specific to the fixture, not generic EE CCD.
            if matches!(fixture, Fixture::TransverseWire) && next.y < GAP - FEAS {
                return Err("Wire left the protected half space".into());
            }
        }
        v = (x[0] - before) / DT;
        let energy = 0.5 * MASS * v.length_squared() + LOAD * x[0].y;
        if step > release {
            max_rise = max_rise.max(energy - previous_energy);
        }
        previous_energy = energy;
        min_gap = min_gap.min(step_gap);
        frames.push(Frame {
            step,
            position: x[0],
            velocity: v,
            energy_j: energy,
            swept_gap_m: step_gap,
            iterations,
            qp_sweeps: sweeps,
            qp_displacement_m: correction,
            converged,
        });
    }
    Ok(Run {
        mode,
        fixture,
        cells_per_axis: cells,
        row_copies: copies,
        held_release: held,
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.,
        max_reference_position_error_m: 0.,
        max_reference_velocity_error_m_s: 0.,
        max_post_release_energy_rise_j: max_rise,
        minimum_swept_gap_m: min_gap,
        unconverged_steps: frames.iter().filter(|f| !f.converged).count(),
        failure,
        frames,
    })
}
fn main() -> Result<(), String> {
    let args = Args::parse();
    let mut runs = Vec::new();
    for (fixture, held) in [Fixture::PointTriangles, Fixture::TransverseWire]
        .into_iter()
        .flat_map(|f| [false, true].map(|h| (f, h)))
    {
        let reference = run(Mode::Plane, fixture, 1, 1, held)?;
        for cells in [1, 8] {
            for mode in [Mode::AllLinearized, Mode::WorkingSet] {
                for copies in [1, 8] {
                    let mut r = run(mode, fixture, cells, copies, held)?;
                    for (f, exact) in r.frames.iter().zip(&reference.frames) {
                        r.max_reference_position_error_m = r
                            .max_reference_position_error_m
                            .max(f.position.distance(exact.position));
                        r.max_reference_velocity_error_m_s = r
                            .max_reference_velocity_error_m_s
                            .max(f.velocity.distance(exact.velocity));
                    }
                    println!(
                        "{fixture:?} {mode:?} {cells}x{cells} copies={copies} held={held}: frames={}, unconverged={}, max error={:.3e} m, gap={:.9} m, failure={:?}",
                        r.frames.len(),
                        r.unconverged_steps,
                        r.max_reference_position_error_m,
                        r.minimum_swept_gap_m,
                        r.failure
                    );
                    runs.push(r);
                }
            }
        }
        runs.push(reference);
    }
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(
        args.output,
        serde_json::to_vec_pretty(&runs).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn swept_gap_checks_interior_edge_crossing_and_stationary_paths() {
        let (x, top) = mesh(1);
        let a = DVec3::new(-0.01, GAP, 0.);
        let b = DVec3::new(0.01, GAP, 0.);
        for f in 0..2 {
            assert!((swept_distance(a, b, f, &x, &top) - GAP).abs() < 1e-12);
            assert!((swept_distance(a, a, f, &x, &top) - distance(a, f, &x, &top)).abs() < 1e-12);
        }
        let a = DVec3::new(0.01, 0.01, 0.);
        let b = DVec3::new(0.01, -0.01, 0.);
        assert_eq!(swept_distance(a, b, 0, &x, &top), 0.);
    }
    #[test]
    fn transverse_wire_checks_the_whole_sweep_not_only_endpoints() {
        let (x, top) = mesh(1);
        let fixture = Fixture::TransverseWire;
        let a = DVec3::new(-0.01, 0.002, 0.);
        let b = DVec3::new(0.01, -0.002, 0.);
        for i in 0..2 {
            assert!(fixture.separation(a, i, 1, &x, &top).length() >= 0.002);
            assert!(fixture.separation(b, i, 1, &x, &top).length() >= 0.002);
            assert!(fixture.swept([a, b], i, 1, &x, &top) < 1e-12);
        }
    }
    #[test]
    fn finite_ee_working_set_preserves_moving_and_held_release() {
        for held in [false, true] {
            let exact = run(Mode::Plane, Fixture::TransverseWire, 1, 1, held).unwrap();
            let trial = run(Mode::WorkingSet, Fixture::TransverseWire, 8, 8, held).unwrap();
            assert!(trial.failure.is_none(), "{:?}", trial.failure);
            assert_eq!(trial.frames.len(), 480);
            assert_eq!(trial.unconverged_steps, 0);
            assert!(trial.minimum_swept_gap_m >= GAP - FEAS);
            assert!(trial.max_post_release_energy_rise_j < 1e-10);
            for (a, b) in trial.frames.iter().zip(&exact.frames) {
                assert!(a.position.distance(b.position) < 1e-8);
                assert!(a.velocity.distance(b.velocity) < 1e-8);
            }
            let speed = trial.frames.last().unwrap().velocity.length();
            if held {
                assert!(speed < 1e-8);
            } else {
                // No friction/damping in this fixture: tangential motion MUST
                // survive release. Settling it artificially would be a bug.
                assert!((speed - 0.004).abs() < 1e-8, "{speed}");
            }
        }
    }
}

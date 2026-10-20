//! CPU-only, fresh-geometry reproduction of EE/PF feature transitions.
use clap::Parser;
use glam::DVec3;
use marvelous_cloth::{
    fitting::FittingProject,
    ogc::{OgcSettings, OgcWorld, geometry, potential::Kind},
    vbd::VbdSolver,
};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

#[derive(Parser)]
struct Args {
    #[arg(long, requires = "capture")]
    project: Option<PathBuf>,
    #[arg(long, requires = "project")]
    capture: Option<PathBuf>,
    #[arg(long)]
    output: PathBuf,
    /// Inspect a point-face boundary instead of an edge-edge endpoint.
    #[arg(long)]
    point_face: bool,
    /// Compare the finite-primitive experiment instead of the EE taper.
    #[arg(long)]
    primitive_distances: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a = Args::parse();
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build_global()?;
    if a.output.exists() {
        return Err("Choose a new output path".into());
    }
    let (mut world, cfg, from, to, ids) = if let (Some(project), Some(capture)) =
        (&a.project, &a.capture)
    {
        let p = FittingProject::load(project)?;
        let s = p.compile()?;
        let world = VbdSolver::new(s.garment, Some(&s.collider.surface), &s.state)?.world;
        let v: Value = serde_json::from_slice(&fs::read(capture)?)?;
        let change = v["largest_feature_changes"]
            .as_array()
            .ok_or("Missing changes")?
            .iter()
            .find(|c| {
                c["kind"]
                    == if a.point_face {
                        "PointFace"
                    } else {
                        "EdgeEdge"
                    }
            })
            .ok_or("No requested feature transition captured")?;
        let from: Vec<DVec3> = serde_json::from_value(v["previous_detection_positions_m"].clone())?;
        let to: Vec<DVec3> = serde_json::from_value(v["dynamic_positions_m"].clone())?;
        let ids: [usize; 4] = serde_json::from_value(change["vertices"].clone())?;
        if from.len() != world.moving || to.len() != world.moving {
            return Err("Capture dimensions differ".into());
        }
        let from = world.positions(&from);
        let to = world.positions(&to);
        (
            world,
            p.simulation.unwrap_or_default().contact,
            from,
            to,
            ids,
        )
    } else if a.point_face {
        let from = vec![
            DVec3::new(0., -1e-9, 0.0002),
            DVec3::new(-0.01, 0., 0.),
            DVec3::new(0.01, 0., 0.),
            DVec3::new(0., 0.02, 0.),
            DVec3::new(0., -0.02, 0.01),
            DVec3::new(-0.001, -1e-9, 0.02),
            DVec3::new(0.001, 0.001 - 1e-9, 0.02),
        ];
        let mut to = from.clone();
        for i in [0, 5, 6] {
            to[i].y += 2e-9;
        }
        let mut world = OgcWorld::new(&from, vec![[1, 2, 3], [2, 1, 4], [0, 5, 6]], 7, [])?;
        world.directional_bounds = true;
        (world, OgcSettings::default(), from, to, [0, 1, 2, 3])
    } else {
        let from = vec![
            DVec3::ZERO,
            DVec3::new(0., 0.02, 0.),
            DVec3::new(-0.01, -1e-6, 0.0002),
            DVec3::new(0.01, -1e-6, 0.0002),
            DVec3::new(0.02, 0., -0.01),
            DVec3::new(-0.02, -0.02, 0.02),
            DVec3::new(0.02, -0.02, 0.02),
            DVec3::new(-0.01, -1e-6, 0.02),
        ];
        let mut to = from.clone();
        for i in [2, 3, 7] {
            to[i].y += 2e-6;
        }
        let mut world = OgcWorld::new(
            &from,
            vec![[0, 1, 4], [0, 4, 5], [0, 5, 6], [2, 3, 7]],
            from.len(),
            [],
        )?;
        world.directional_bounds = true;
        (world, OgcSettings::default(), from, to, [0, 1, 2, 3])
    };
    let mut candidate = world.clone();
    candidate.experimental_interior_edges = !a.primitive_distances;
    candidate.experimental_primitive_distances = a.primitive_distances;
    let old = world.detect(&from, cfg.radius * 2., &cfg)?;
    let old_candidate = candidate.detect(&from, cfg.radius * 2., &cfg)?;
    let mut samples = Vec::new();
    for i in 0..=64 {
        let alpha = i as f64 / 64.;
        let x: Vec<_> = from
            .iter()
            .zip(&to)
            .map(|(a, b)| *a + (*b - *a) * alpha)
            .collect();
        world.validate_start(&x)?;
        let baseline = world.detect(&x, cfg.radius * 2., &cfg)?;
        let smooth = candidate.detect(&x, cfg.radius * 2., &cfg)?;
        assert_eq!(baseline.radii, smooth.radii);
        assert_eq!(baseline.minimum_distance, smooth.minimum_distance);
        assert_eq!(baseline.directional, smooth.directional);
        let feature_geometry = if a.point_face {
            let face = world
                .topology
                .triangles
                .iter()
                .position(|tri| {
                    let mut sorted = *tri;
                    sorted.sort_unstable();
                    sorted == ids[1..]
                })
                .ok_or("Captured triangle not found in the project")?;
            let [p, a, b, c] = ids.map(|j| x[j]);
            let u = b - a;
            let v = c - a;
            let q = p - a;
            let det = u.length_squared() * v.length_squared() - u.dot(v).powi(2);
            let beta = (v.length_squared() * u.dot(q) - u.dot(v) * v.dot(q)) / det;
            let gamma = (u.length_squared() * v.dot(q) - u.dot(v) * u.dot(q)) / det;
            let (closest, feature) = geometry::closest_triangle(p, face, &x, &world.topology);
            let feasible = match feature {
                geometry::Feature::Vertex(i) => {
                    geometry::vertex_feasible(p, i, &x, &world.topology)
                }
                geometry::Feature::Edge(e) => geometry::edge_feasible(p, e, &x, &world.topology),
                geometry::Feature::Face(_) => true,
            };
            json!({"barycentric":[1.-beta-gamma,beta,gamma],
                "closest_feature":format!("{feature:?}"),"feature_feasible":feasible,
                "distance_m":p.distance(closest)})
        } else {
            let [a, b, c, d] = ids.map(|j| x[j]);
            let (s, t) = geometry::closest_segments(a, b, c, d);
            let pa = a.lerp(b, s);
            let pb = c.lerp(d, t);
            let endpoint_feasible = [(s <= 0., ids[0], pb), (s >= 1., ids[1], pb),
            (t <= 0., ids[2], pa), (t >= 1., ids[3], pa)].into_iter()
            .filter(|(endpoint,_,_)| *endpoint).map(|(_,v,p)| json!({"vertex":v,
                "feasible":geometry::vertex_feasible(p,v,&x,&world.topology),
                "violating_neighbors":world.topology.neighbors[v].iter().filter(|j| (p-x[v]).dot(x[v]-x[**j]) < -1e-14).collect::<Vec<_>>() })).collect::<Vec<_>>();
            json!({"s":s,"t":t,"distance_m":pa.distance(pb),"endpoint_tests":endpoint_feasible})
        };
        let pair = |set: &marvelous_cloth::ogc::ContactSet| {
            let contact = set.contacts.iter().find(|c| {
                (if a.point_face {
                    matches!(c.kind, Kind::PointFace | Kind::PointTriangle)
                } else {
                    matches!(
                        c.kind,
                        Kind::EdgeEdge | Kind::EdgeEdgeInterior | Kind::SegmentSegment
                    )
                }) && c.vertices == ids
            });
            json!({"present":contact.is_some(), "energy_j":contact.map_or(0.,|c|c.value(&x,cfg.radius,cfg.stiffness)),
                "gradient_norm_n":contact.map_or(0.,|c|ids.iter().map(|j|c.block(&x,*j,cfg.radius,cfg.stiffness).1.length_squared()).sum::<f64>().sqrt())})
        };
        // Keep the established EE JSON keys for existing analysis scripts.
        let mut sample = feature_geometry;
        sample.as_object_mut().unwrap().extend(json!({"alpha":alpha,
            "baseline":pair(&baseline),"candidate":pair(&smooth),
            "baseline_total_contact_j":baseline.value(&x,&cfg),"candidate_total_contact_j":smooth.value(&x,&cfg),
            "baseline_refresh_delta_j":baseline.value(&x,&cfg)-old.value(&x,&cfg),
            "candidate_refresh_delta_j":smooth.value(&x,&cfg)-old_candidate.value(&x,&cfg)}).as_object().unwrap().clone());
        samples.push(sample);
    }
    if let Some(parent) = a.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        a.output,
        serde_json::to_vec_pretty(
            &json!({"vertices":ids,"kind":if a.point_face {"PointFace"} else {"EdgeEdge"},
                "experimental_primitive_distances":a.primitive_distances,
                "config":cfg,"bounds_identical":true,"sampled_intersections":false,"samples":samples}),
        )?,
    )?;
    Ok(())
}

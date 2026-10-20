//! Local-field accuracy and micro-cost audit. No cloth stepping or GPU use.
use clap::Parser;
use glam::{DQuat, DVec3};
use marvelous_cloth::{
    fitting::FittingProject,
    math::{Jet, Scalar},
    ogc::{
        geometry::closest_triangle,
        local_surface::{self, Sample},
        potential::Kind,
    },
    vbd::VbdSolver,
};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    hint::black_box,
    path::PathBuf,
    time::Instant,
};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    project: Option<PathBuf>,
}

fn dress_audit(path: &PathBuf) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let project = FittingProject::load(path)?;
    let scene = project.compile()?;
    let time = scene.state.time;
    let mut solver = VbdSolver::new(scene.garment, Some(&scene.collider.surface), &scene.state)?;
    let x = solver.world.positions(&scene.state.x);
    let cfg = project.simulation.unwrap_or_default().contact;
    let contacts = solver.world.detect(&x, 2. * cfg.radius, &cfg)?;
    let top = &solver.world.topology;
    let mut face_lookup = BTreeMap::new();
    let mut edge_faces = vec![Vec::new(); top.edges.len()];
    for (f, t) in top.triangles.iter().enumerate() {
        let mut key = *t;
        key.sort_unstable();
        face_lookup.insert(key, f);
        for &e in &top.triangle_edges[f] {
            edge_faces[e].push(f);
        }
    }
    let mut rows = Vec::new();
    for term in &contacts.contacts {
        if term.kind != Kind::PointFace || term.value(&x, cfg.radius, cfg.stiffness) <= 0. {
            continue;
        }
        let vertex = term.vertices[0];
        let mut key = [term.vertices[1], term.vertices[2], term.vertices[3]];
        key.sort_unstable();
        let face = *face_lookup.get(&key).ok_or("Contact triangle is missing")?;
        if !solver.world.vertex_face_eligible(vertex, face)? {
            return Err("Active seed violates world exclusions".into());
        }
        let mut patch = BTreeSet::from([face]);
        let mut excluded = 0;
        for &e in &top.triangle_edges[face] {
            for &f in &edge_faces[e] {
                if solver.world.vertex_face_eligible(vertex, f)? {
                    patch.insert(f);
                } else {
                    excluded += 1;
                }
            }
        }
        let samples = patch
            .iter()
            .map(|f| Sample::triangle(top.triangles[*f].map(|i| x[i].to_array().map(Jet::c)), 1.))
            .collect::<Result<Vec<_>, _>>()?;
        // Diagnostic only: a separate, frozen radius for each snapshot query.
        // This is NOT a radius update rule or a force halo for simulation.
        let max_center_distance = patch
            .iter()
            .map(|f| {
                let [a, b, c] = top.triangles[*f].map(|i| x[i]);
                x[vertex].distance((a + b + c) / 3.)
            })
            .fold(0., f64::max);
        let support = 2. * (max_center_distance + cfg.radius);
        let query = std::array::from_fn(|a| Jet::<3>::variable(x[vertex][a], a));
        let field = local_surface::evaluate(query, &samples, support)?;
        let closest = closest_triangle(x[vertex], face, &x, top).0;
        let on_mesh = local_surface::evaluate(closest.to_array().map(Jet::c), &samples, support)?;
        let true_distance = patch
            .iter()
            .map(|f| x[vertex].distance(closest_triangle(x[vertex], *f, &x, top).0))
            .fold(f64::INFINITY, f64::min);
        rows.push(
            json!({"vertex":vertex,"seed_face":face,"patch_face_count":patch.len(),
            "excluded_neighbor_visits":excluded,"support_m":support,
            "finite_patch_distance_m":true_distance,"absolute_field_m":field.value.v.abs(),
            "gap_error_m":field.value.v.abs()-true_distance,
            "field_on_seed_mesh_m":on_mesh.value.v,"normal_coherence":field.normal_coherence,
            "query_gradient_norm":DVec3::from_array(field.value.g).length()}),
        );
    }
    let after = solver.world.detect(&x, 2. * cfg.radius, &cfg)?;
    let unchanged = contacts.radii == after.radii
        && contacts.centers == after.centers
        && format!("{:?}", contacts.contacts) == format!("{:?}", after.contacts)
        && format!("{:?}", contacts.directional) == format!("{:?}", after.directional);
    if !unchanged {
        return Err("Read-only audit changed production contacts/bounds".into());
    }
    Ok(
        json!({"scope":"Snapshot VF patch field mismatch; no force assembly or stepping",
        "project":path,"state_time_s":time,"contact_radius_m":cfg.radius,
        "production_contacts_and_bounds_unchanged":unchanged,"queries":rows}),
    )
}
fn mesh(n: usize, uneven: bool, angle: f64) -> Vec<Sample<Jet<3>>> {
    let q = DQuat::from_rotation_z(angle) * DQuat::from_rotation_x(-0.37);
    let mut vertices = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            let t = i as f64 / n as f64;
            let x = if uneven { t * t } else { t };
            vertices.push(q * DVec3::new(-0.1 + 0.2 * x, 0., -0.1 + 0.2 * j as f64 / n as f64));
        }
    }
    let mut samples = Vec::new();
    for j in 0..n {
        for i in 0..n {
            let a = j * (n + 1) + i;
            let b = a + 1;
            let c = a + n + 1;
            let d = c + 1;
            let triangles = if (i + j) % 2 == 0 {
                [[a, c, b], [b, c, d]]
            } else {
                [[a, d, b], [a, c, d]]
            };
            for t in triangles {
                let [a, b, c] = t.map(|v| vertices[v]);
                let area = 0.5 * (b - a).cross(c - a).length();
                samples.push(
                    Sample::triangle(t.map(|v| vertices[v].to_array().map(Jet::c)), area).unwrap(),
                );
            }
        }
    }
    samples
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.output.exists() {
        return Err("Choose a fresh output path".into());
    }
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build_global()?;
    let mut scans = Vec::new();
    for n in [2, 8] {
        for uneven in [false, true] {
            for support in [0.08, 0.2] {
                let angle = 0.43;
                let q = DQuat::from_rotation_z(angle) * DQuat::from_rotation_x(-0.37);
                let normal = q * DVec3::Y;
                let samples = mesh(n, uneven, angle);
                let mut max_gap_error: f64 = 0.;
                let mut max_lateral: f64 = 0.;
                let mut max_normal_error: f64 = 0.;
                let mut min_samples = usize::MAX;
                let mut max_samples = 0;
                for i in 0..1201 {
                    let p = q * DVec3::new(-0.06 + i as f64 * 0.0001, 0.001, 0.007);
                    let point = std::array::from_fn(|a| Jet::<3>::variable(p[a], a));
                    let field = local_surface::evaluate(point, &samples, support)?;
                    let energy =
                        local_surface::energy(point, &samples, support, 1., 0.003, 100_000.)?;
                    let g = DVec3::from_array(energy.g);
                    max_gap_error = max_gap_error.max((field.value.v - 0.001).abs());
                    max_lateral = max_lateral.max((g - normal * g.dot(normal)).length());
                    max_normal_error = max_normal_error.max((-g.dot(normal) - 225.).abs());
                    min_samples = min_samples.min(field.active_samples);
                    max_samples = max_samples.max(field.active_samples);
                }
                let p = q * DVec3::new(0.002, 0.001, 0.007);
                let point = std::array::from_fn(|a| Jet::<3>::variable(p[a], a));
                let start = Instant::now();
                for _ in 0..1000 {
                    black_box(local_surface::energy(
                        black_box(point),
                        black_box(&samples),
                        support,
                        1.,
                        0.003,
                        100_000.,
                    )?);
                }
                scans.push(json!({"grid_cells_per_axis":n,"uneven":uneven,"support_m":support,
                    "sample_count":samples.len(),"active_samples_min":min_samples,"active_samples_max":max_samples,
                    "maximum_gap_error_m":max_gap_error,"maximum_lateral_force_n":max_lateral,
                    "maximum_normal_force_error_n":max_normal_error,
                    "microseconds_per_jet3_query":start.elapsed().as_secs_f64()*1000.}));
            }
        }
    }
    let mut layers = Vec::new();
    for opposite in [false, true] {
        let samples = [
            Sample::oriented([0., 0., 0.].map(Jet::c), [0., 1., 0.].map(Jet::c), 1.)?,
            Sample::oriented(
                [0., 0.002, 0.].map(Jet::c),
                [0., if opposite { -1. } else { 1. }, 0.].map(Jet::c),
                1.,
            )?,
        ];
        for height in [0., 0.0005, 0.001, 0.0015, 0.002] {
            let p = [
                Jet::<3>::variable(0., 0),
                Jet::variable(height, 1),
                Jet::variable(0., 2),
            ];
            let f = local_surface::evaluate(p, &samples, 0.1)?;
            layers.push(json!({"opposite_normals":opposite,"height_m":height,
                "field_m":f.value.v,"query_gradient":f.value.g,
                "normal_coherence":f.normal_coherence,"true_plane_distance_m":height.min(0.002-height)}));
        }
    }
    let crease = [
        Sample::oriented([-0.05, 0.01, 0.], [0.2, 1., 0.], 1.)?,
        Sample::oriented([0.05, 0.01, 0.], [-0.2, 1., 0.], 1.)?,
    ];
    let on_crease_face = [0.01, 0.002, 0.];
    let f = local_surface::evaluate(on_crease_face, &crease, 0.2)?;
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        args.output,
        serde_json::to_vec_pretty(&json!({
            "scope":"Single-patch local field, not a self-contact solver",
            "planar_scans":scans,"layer_counterexamples":layers,
            "dress_audit":args.project.as_ref().map(dress_audit).transpose()?,
            "concave_crease_counterexample":{"point_on_actual_face":on_crease_face,
                "field_m":f.value,"normal_coherence":f.normal_coherence,
                "barrier_on_actual_face_j":local_surface::energy(on_crease_face,&crease,0.2,1.,0.003,100_000.)?}
        }))?,
    )?;
    Ok(())
}

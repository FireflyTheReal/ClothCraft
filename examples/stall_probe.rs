//! CPU investigation of the geometric pairs restricting a saved cloth state.
use marvelous_cloth::{
    fitting::FittingProject,
    ogc::geometry::{self, Feature},
    vbd::VbdSolver,
};
use serde_json::json;
use std::{collections::BTreeSet, path::Path};

fn main() -> Result<(), String> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build_global()
        .map_err(|e| e.to_string())?;
    let path = std::env::args()
        .nth(1)
        .ok_or("Usage: stall_probe PROJECT")?;
    let p = FittingProject::load(Path::new(&path))?;
    let s = p.compile()?;
    let settings = p.simulation.clone().unwrap_or_default();
    let mut solver = VbdSolver::new(s.garment.clone(), Some(&s.collider.surface), &s.state)?;
    // Report the geometric witnesses behind the original isotropic restriction,
    // even when the production solver can now move inside separating planes.
    solver.world.directional_bounds = false;
    let x = solver.world.positions(&s.state.x);
    let contact = solver
        .world
        .detect(&x, settings.contact.radius * 2., &settings.contact)?;
    let top = &solver.world.topology;
    let targets: BTreeSet<_> = contact
        .radii
        .iter()
        .enumerate()
        .filter_map(|(i, r)| (*r < 1e-5).then_some(i))
        .collect();
    let mut excluded = BTreeSet::new();
    for seam in &s.garment.seams {
        for sample in &seam.samples {
            for &(a, wa) in &sample.point.terms {
                for &(b, wb) in &sample.point.terms {
                    if wa * wb < 0. {
                        excluded.insert([a.min(b), a.max(b)]);
                    }
                }
            }
        }
    }
    let adjacent = |a: usize, b: usize| a == b || excluded.contains(&[a.min(b), a.max(b)]);
    let vertex = |i: usize| {
        if i >= s.state.x.len() {
            return json!({"avatar_vertex":i-s.state.x.len(),"position":x[i]});
        }
        let panel = s
            .garment
            .panels
            .iter()
            .find(|p| p.vertices.contains(&i))
            .unwrap();
        json!({"panel":panel.id,"uv_m":s.garment.mesh.uv[i],"position":x[i],"speed_m_s":s.state.v[i].length(),"bound_m":contact.radii[i],"incident_contacts":contact.incident[i].len(),"temporary_vertex":i})
    };
    let mut vf = BTreeSet::new();
    for &v in &targets {
        for t in 0..top.triangles.len() {
            vf.insert((v, t));
        }
    }
    for (t, ids) in top
        .triangles
        .iter()
        .enumerate()
        .filter(|(_, ids)| ids.iter().any(|v| targets.contains(v)))
    {
        for v in 0..x.len() {
            if !ids.contains(&v) {
                vf.insert((v, t));
            }
        }
    }
    let mut witnesses = Vec::new();
    for (v, t) in vf {
        let ids = top.triangles[t];
        if ids.iter().any(|&i| adjacent(v, i)) {
            continue;
        }
        let (q, feature) = geometry::closest_triangle(x[v], t, &x, top);
        let distance = x[v].distance(q);
        if distance > 3e-5 {
            continue;
        }
        let feasible = match feature {
            Feature::Vertex(i) => geometry::vertex_feasible(x[v], i, &x, top),
            Feature::Edge(e) => geometry::edge_feasible(x[v], e, &x, top),
            Feature::Face(_) => true,
        };
        witnesses.push(json!({"kind":"VF","distance_m":distance,"feasible":feasible,"feature":format!("{feature:?}"),"vertices":[vertex(v),vertex(ids[0]),vertex(ids[1]),vertex(ids[2])]}));
    }
    let mut ee = BTreeSet::new();
    for (e, _) in top
        .edges
        .iter()
        .enumerate()
        .filter(|(_, ids)| ids.iter().any(|v| targets.contains(v)))
    {
        for f in 0..top.edges.len() {
            if e != f {
                ee.insert((e.min(f), e.max(f)));
            }
        }
    }
    for (e, f) in ee {
        let a = top.edges[e];
        let b = top.edges[f];
        if a.iter().any(|&i| b.iter().any(|&j| adjacent(i, j))) {
            continue;
        }
        let (u, v) = geometry::closest_segments(x[a[0]], x[a[1]], x[b[0]], x[b[1]]);
        let pa = x[a[0]].lerp(x[a[1]], u);
        let pb = x[b[0]].lerp(x[b[1]], v);
        let distance = pa.distance(pb);
        if distance > 3e-5 {
            continue;
        }
        witnesses.push(json!({"kind":"EE","distance_m":distance,"parameters":[u,v],"vertices":[vertex(a[0]),vertex(a[1]),vertex(b[0]),vertex(b[1])]}));
    }
    witnesses.sort_by(|a, b| {
        a["distance_m"]
            .as_f64()
            .unwrap()
            .total_cmp(&b["distance_m"].as_f64().unwrap())
    });
    println!("{}",serde_json::to_string_pretty(&json!({"restricted_vertices":targets.iter().copied().map(vertex).collect::<Vec<_>>(),"witnesses":witnesses})).map_err(|e| e.to_string())?);
    Ok(())
}

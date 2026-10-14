use marvelous_cloth::garment::{
    meshing::{self, MeshingSettings},
    presets::{self, GarmentKind},
};
use std::collections::BTreeMap;

#[test]
fn cdt_refines_area_angles_and_preserves_all_named_contours() {
    for kind in [GarmentKind::Tshirt, GarmentKind::Dress] {
        for (ratio, angle) in [(1.0, 0.0), (2.0, 37.0)] {
            let mut t = presets::basic(kind, 2);
            for p in &mut t.panels {
                let m = p.polygon.as_mut().unwrap().meshing.as_mut().unwrap();
                m.anisotropy = ratio;
                m.axis_degrees = angle;
            }
            let g = t.compile().unwrap();
            for (p, r) in t.panels.iter().zip(&g.panels) {
                let poly = p.polygon.as_ref().unwrap();
                let m = poly.meshing_settings();
                let q = meshing::quality(&g.mesh.triangles[r.triangles.clone()], &m);
                assert!(
                    q.max_metric_triangle_area <= 3.0_f64.sqrt() / 4.0 + 1e-9,
                    "{q:?}"
                );
                assert!(q.min_metric_angle_degrees >= 24.999, "{q:?}");
                let mut edges = BTreeMap::<_, Vec<_>>::new();
                for triangle in &g.mesh.triangles[r.triangles.clone()] {
                    let [a, b, c] = triangle.vertices;
                    for (a, b, c) in [(a, b, c), (b, c, a), (c, a, b)] {
                        edges.entry((a.min(b), a.max(b))).or_default().push(c);
                    }
                }
                for (i, name) in poly.edges.iter().enumerate() {
                    let a = poly.points[i];
                    let b = poly.points[(i + 1) % poly.points.len()];
                    let ids = &r.boundaries[name];
                    assert!(g.mesh.uv[ids[0]].distance(a) < 1e-10);
                    assert!(g.mesh.uv[*ids.last().unwrap()].distance(b) < 1e-10);
                    for pair in ids.windows(2) {
                        assert_eq!(
                            edges[&(pair[0].min(pair[1]), pair[0].max(pair[1]))].len(),
                            1
                        );
                    }
                }
                for ((a, b), opposite) in edges {
                    if let [c, d] = opposite[..] {
                        let [a, b, c, d] = [a, b, c, d].map(|i| m.to_metric(g.mesh.uv[i]));
                        let cot = (a - c).dot(b - c) / (a - c).perp_dot(b - c).abs()
                            + (a - d).dot(b - d) / (a - d).perp_dot(b - d).abs();
                        assert!(cot >= -1e-7, "unconstrained edge is not Delaunay: {cot}");
                    }
                }
            }
        }
    }
}

#[test]
fn density_is_controlled_by_material_space_size_and_is_reproducible() {
    let coarse = presets::basic(GarmentKind::Tshirt, 1).compile().unwrap();
    let template = presets::basic(GarmentKind::Tshirt, 2);
    let fine = template.compile().unwrap();
    let again = template.compile().unwrap();
    assert!(fine.mesh.triangles.len() > coarse.mesh.triangles.len() * 3);
    assert_eq!(fine.mesh.uv, again.mesh.uv);
    assert_eq!(
        fine.mesh
            .triangles
            .iter()
            .map(|t| t.vertices)
            .collect::<Vec<_>>(),
        again
            .mesh
            .triangles
            .iter()
            .map(|t| t.vertices)
            .collect::<Vec<_>>()
    );
    assert!(
        (fine.mesh.masses.iter().sum::<f64>() - coarse.mesh.masses.iter().sum::<f64>()).abs()
            < 1e-10
    );
    assert!(
        (fine.seams.iter().map(|s| s.measure()).sum::<f64>()
            - coarse.seams.iter().map(|s| s.measure()).sum::<f64>())
        .abs()
            < 1e-10
    );
}

#[test]
fn invalid_sizes_and_exhausted_budgets_fail_explicitly() {
    for size in [0.0, f64::NAN, 0.00001] {
        assert!(
            MeshingSettings {
                target_edge_m: size,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
    let mut t = presets::basic(GarmentKind::Dress, 2);
    t.panels[0]
        .polygon
        .as_mut()
        .unwrap()
        .meshing
        .as_mut()
        .unwrap()
        .max_vertices = 64;
    assert!(t.compile().unwrap_err().contains("budget"));
}

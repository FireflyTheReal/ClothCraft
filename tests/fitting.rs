use glam::DVec3;
use marvelous_cloth::{
    avatar::{MeshCollider, SurfaceMesh},
    contact::Collider,
    garment::presets::{self, GarmentKind},
};
use std::sync::Arc;

fn cube() -> Arc<SurfaceMesh> {
    let vertices = vec![
        [-1., -1., -1.],
        [1., -1., -1.],
        [1., 1., -1.],
        [-1., 1., -1.],
        [-1., -1., 1.],
        [1., -1., 1.],
        [1., 1., 1.],
        [-1., 1., 1.],
    ]
    .into_iter()
    .map(DVec3::from_array)
    .collect();
    let faces = [
        [0, 3, 2, 1],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [3, 7, 6, 2],
        [0, 4, 7, 3],
        [1, 2, 6, 5],
    ];
    Arc::new(SurfaceMesh {
        vertices,
        triangles: faces
            .into_iter()
            .flat_map(|[a, b, c, d]| [[a, b, c], [a, c, d]])
            .collect(),
    })
}
#[test]
fn avatar_bvh_distance_matches_analytic_box_inside_and_outside() {
    let c = MeshCollider::new(cube(), 1000.0).unwrap();
    for i in 1..400 {
        let p = DVec3::new(
            (i as f64 * 1.23).sin(),
            (i as f64 * 2.47).sin(),
            (i as f64 * 3.31).sin(),
        ) * 1.6;
        let q = p.abs() - DVec3::ONE;
        let exact = q.max(DVec3::ZERO).length() + q.max_element().min(0.0);
        assert!((c.signed_distance(p) - exact).abs() < 1e-12, "{p:?}");
    }
    assert_eq!(c.z_bounds(0.2, 0.3), Some([-1.0, 1.0]));
    assert!(c.z_bounds(2.0, 0.0).is_none());
}
#[test]
fn avatar_ray_bvh_matches_exhaustive_visibility_including_parallel_and_boundary_rays() {
    let surface = cube();
    let collider = MeshCollider::new(surface.clone(), 1000.).unwrap();
    for i in 0..600 {
        let origin = DVec3::new(
            (i as f64 * 1.23).sin(),
            (i as f64 * 2.47).cos(),
            (i as f64 * 3.31).sin(),
        ) * 2.;
        for direction in [
            DVec3::X,
            -DVec3::Y,
            DVec3::Z,
            -origin,
            DVec3::new(1e-16, 0., -1.),
        ] {
            for max_t in [0.1, 1., 3., f64::INFINITY] {
                let expected = surface.triangles.iter().any(|tri| {
                    marvelous_cloth::picking::ray_triangle(
                        origin,
                        direction,
                        tri.map(|v| surface.vertices[v]),
                    )
                    .is_some_and(|(t, _)| t < max_t)
                });
                assert_eq!(
                    collider.occludes_ray(origin, direction, max_t),
                    expected,
                    "{origin:?} {direction:?} {max_t}"
                );
            }
        }
    }
    assert!(!collider.occludes_ray(DVec3::new(1., 1., 2.), -DVec3::Z, 1.));
    assert!(collider.occludes_ray(DVec3::new(1., 1., 2.), -DVec3::Z, 1.001));
    assert!(!collider.occludes_ray(DVec3::ZERO, DVec3::ZERO, 3.));
    assert!(!collider.occludes_ray(DVec3::NAN, DVec3::X, 3.));
}
#[test]
fn avatar_contact_derivatives_cover_face_edge_and_vertex_regions() {
    let c = MeshCollider::new(cube(), 1000.0).unwrap();
    let eps = 1e-7;
    for x in [
        DVec3::new(0.97, 0.2, 0.3),
        DVec3::new(1.01, 0.2, 0.3),
        DVec3::new(1.02, 1.03, 0.2),
        DVec3::new(1.02, 1.03, 1.04),
    ] {
        let e = c.element(0, x, 0.2, 0.03).unwrap();
        for k in 0..3 {
            let mut a = x;
            let mut b = x;
            a[k] += eps;
            b[k] -= eps;
            let value = (c.value(a, 0.2, 0.03) - c.value(b, 0.2, 0.03)) / (2.0 * eps);
            assert!((value - e.gradient[k]).abs() < 1e-6);
            let ga = c.element(0, a, 0.2, 0.03).unwrap();
            let gb = c.element(0, b, 0.2, 0.03).unwrap();
            for j in 0..3 {
                let h = (ga.gradient[j] - gb.gradient[j]) / (2.0 * eps);
                assert!((h - e.hessian[j * 3 + k]).abs() < 1e-5);
            }
        }
    }
}
#[test]
fn obj_negative_indices_and_invalid_geometry_are_checked() {
    let m = SurfaceMesh::parse_obj("v 0 0 0\nv 1 0 0\nv 0 1 0\nf -3 -2 -1\n").unwrap();
    assert_eq!(m.triangles, vec![[0, 1, 2]]);
    for source in [
        "v 0 0 0\nf 0 1 2",
        "v nan 0 0",
        "v 0 0 0\nv 1 0 0\nv 2 0 0\nf 1 2 3",
    ] {
        assert!(SurfaceMesh::parse_obj(source).is_err());
    }
}
#[test]
fn garment_contours_keep_area_mass_and_named_seams_under_refinement() {
    for kind in [GarmentKind::Tshirt, GarmentKind::Dress] {
        let mut areas = vec![];
        let mut lengths = vec![];
        for level in 0..=2 {
            let t = presets::basic(kind, level);
            let g = t.compile().unwrap();
            areas.push(g.mesh.triangles.iter().map(|t| t.area).sum::<f64>());
            assert!(
                (g.mesh.masses.iter().sum::<f64>()
                    - areas.last().unwrap() * t.fabrics[0].properties.density)
                    .abs()
                    < 1e-12
            );
            lengths.push(g.seams.iter().map(|s| s.measure()).sum::<f64>());
            assert_eq!(
                g.seams.len(),
                if kind == GarmentKind::Tshirt { 8 } else { 4 }
            );
            assert!(g.panels.iter().all(|p| p.boundaries.contains_key("hem")));
            assert!(
                t.seams
                    .iter()
                    .all(|s| !s.id.contains("cuff") && !s.id.contains("neck") && s.id != "hem")
            );
        }
        assert!((areas[0] - areas[2]).abs() < 1e-12);
        assert!((lengths[0] - lengths[2]).abs() < 1e-12);
    }
}
#[test]
fn polygons_reject_crossings_reversed_winding_and_v1_schema() {
    let mut t = presets::basic(GarmentKind::Tshirt, 1);
    t.version = 1;
    assert!(t.compile().is_err());
    t.version = 2;
    t.panels[0].polygon.as_mut().unwrap().points.swap(4, 8);
    assert!(t.compile().is_err());
    let mut t = presets::basic(GarmentKind::Dress, 1);
    t.panels[0].polygon.as_mut().unwrap().points.reverse();
    assert!(t.compile().is_err());
}

#[test]
fn fitting_cache_rejects_remeshed_state_and_failed_step_is_transactional() {
    use marvelous_cloth::fitting::FittingProject;
    let avatar =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/makehuman/mannequin.obj");
    let mut project = FittingProject::basic(GarmentKind::Tshirt, 0, avatar);
    let mut scene = project.compile().unwrap();
    let before = scene.state.clone();
    let mut bad = project.solver.clone();
    bad.gravity.x = f64::NAN;
    assert!(scene.step(&bad).is_err());
    assert_eq!(scene.state.x, before.x);
    assert_eq!(scene.state.v, before.v);
    assert_eq!(scene.state.time, before.time);
    project.state = Some(scene.state);
    let path =
        std::env::temp_dir().join(format!("cloth-fitting-cache-{}.json", std::process::id()));
    project.save(&path).unwrap();
    let mut restored = FittingProject::load(&path).unwrap();
    restored.compile().unwrap();
    restored.template.panels[0]
        .polygon
        .as_mut()
        .unwrap()
        .meshing
        .as_mut()
        .unwrap()
        .target_edge_m *= 0.8;
    assert!(
        restored
            .compile()
            .err()
            .unwrap()
            .contains("Stale fitting cache")
    );
    std::fs::remove_file(path).unwrap();
}

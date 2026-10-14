use glam::{DVec2, DVec3};
use marvelous_cloth::garment::{GarmentTemplate, embedding::MaterialPath, export};

#[test]
fn attachments_reproduce_material_coordinates_and_affine_deformations() {
    let g = GarmentTemplate::two_panel_demo().compile().unwrap();
    let mut x = g.initial_state.x.clone();
    for i in g.panels[1].vertices.clone() {
        let u = g.mesh.uv[i];
        x[i] = DVec3::new(1.0 + 2.0 * u.x, -2.0 + 3.0 * u.y, u.x - u.y);
    }
    for uv in [DVec2::ZERO, DVec2::new(0.4, 0.6), DVec2::new(0.123, 0.371)] {
        let p = g.embed_point("right", uv).unwrap();
        let reconstructed = p
            .vertices
            .iter()
            .zip(p.weights)
            .map(|(i, w)| g.mesh.uv[*i] * w)
            .sum::<DVec2>();
        assert!(reconstructed.distance(uv) < 1e-13);
        assert!(
            p.evaluate(&x).unwrap().distance(DVec3::new(
                1.0 + 2.0 * uv.x,
                -2.0 + 3.0 * uv.y,
                uv.x - uv.y
            )) < 1e-13
        );
        assert!(p.evaluate(&[]).is_err());
    }
    assert!(g.embed_point("missing", DVec2::ZERO).is_err());
    assert!(g.embed_point("right", DVec2::new(-0.01, 0.2)).is_err());
    assert!(g.embed_point("right", DVec2::NAN).is_err());
}

#[test]
fn material_path_splits_at_fold_instead_of_cutting_through_surface() {
    let mut t = GarmentTemplate::two_panel_demo();
    t.panels.truncate(1);
    t.seams.clear();
    t.panels[0].pattern.nx = 2;
    t.panels[0].pattern.ny = 1;
    let g = t.compile().unwrap();
    let mut x = g.initial_state.x.clone();
    // Fold right half by 90 degrees around u=.2.
    for (i, p) in x.iter_mut().enumerate() {
        let uv = g.mesh.uv[i];
        *p = if uv.x <= 0.2 {
            DVec3::new(uv.x, uv.y, 0.0)
        } else {
            DVec3::new(0.2, uv.y, uv.x - 0.2)
        };
    }
    let path = MaterialPath {
        panel: "left".into(),
        points: vec![DVec2::new(0.0, 0.3), DVec2::new(0.4, 0.3)],
    };
    let embedded = g.embed_path(&path).unwrap();
    assert!(embedded.iter().any(|p| (p.uv.x - 0.2).abs() < 1e-13));
    let points: Vec<_> = embedded.iter().map(|p| p.evaluate(&x).unwrap()).collect();
    let length: f64 = points.windows(2).map(|p| p[0].distance(p[1])).sum();
    assert!((length - 0.4).abs() < 1e-12);
    assert!(points[0].distance(*points.last().unwrap()) < length - 0.1);
}

#[test]
fn material_paths_recompile_after_remeshing_and_export_guides() {
    let mut t = GarmentTemplate::two_panel_demo();
    let path = MaterialPath {
        panel: "right".into(),
        points: vec![DVec2::new(0.03, 0.08), DVec2::new(0.38, 0.57)],
    };
    let original = t.compile().unwrap();
    let a = original.embed_path(&path).unwrap();
    t.panels[1].pattern.nx = 7;
    t.panels[1].pattern.ny = 9;
    let fine = t.compile().unwrap();
    let b = fine.embed_path(&path).unwrap();
    for (a, b) in [
        (a.first().unwrap(), b.first().unwrap()),
        (a.last().unwrap(), b.last().unwrap()),
    ] {
        assert!(
            a.evaluate(&original.initial_state.x)
                .unwrap()
                .distance(b.evaluate(&fine.initial_state.x).unwrap())
                < 1e-13
        );
    }
    let obj = export::grain_guides_obj(&t, &fine, &fine.initial_state).unwrap();
    assert_eq!(obj.lines().filter(|l| l.starts_with("l ")).count(), 4);
    assert!(obj.contains("g right_weft"));
}

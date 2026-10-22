use glam::{DVec2, DVec3};
use marvelous_cloth::garment::{
    GarmentTemplate,
    domain::Hole,
    meshing::MeshingSettings,
    production_metrics::report,
    shapes::{self, Primitive},
};

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-11, "{actual} != {expected}");
}

fn polygon(kind: Primitive) -> GarmentTemplate {
    let mut template = GarmentTemplate::two_panel_demo();
    template.version = 2;
    template.panels.truncate(1);
    template.seams.clear();
    template.panels[0].pins.clear();
    template.panels[0].polygon = Some(
        shapes::primitive(
            kind,
            DVec2::new(0.4, 0.6),
            90.,
            MeshingSettings {
                target_edge_m: 0.07,
                ..Default::default()
            },
        )
        .unwrap(),
    );
    template
}

#[test]
fn exact_rectangles_aggregate_shared_fabric_independently_of_mesh_density() {
    let template = GarmentTemplate::two_panel_demo();
    let output = report(&template).unwrap();
    assert_eq!(output.panel_count, 2);
    assert_eq!(output.fabrics.len(), 1);
    assert_eq!(output.fabrics[0].panel_count, 2);
    close(output.net_area_m2, 0.48);
    close(output.mass_kg, 0.48 * 0.2);
    for p in &output.panels {
        close(p.net_area_m2, 0.24);
        close(p.mass_kg, 0.24 * 0.2);
        assert_eq!(p.material_bounds.min_m, [0., 0.]);
        close(p.material_bounds.max_m[0], 0.4);
        close(p.material_bounds.max_m[1], 0.6);
    }
    let json = serde_json::to_value(&output).unwrap();
    assert_eq!(json["format"], "clothcraft-net-material-metrics");
    assert_eq!(json["geometry_basis"], "compiled_material_rest_mesh");
}

#[test]
fn polygon_holes_are_subtracted_and_curves_are_not_bounding_rectangles() {
    let mut template = polygon(Primitive::Rectangle);
    template.panels[0]
        .polygon
        .as_mut()
        .unwrap()
        .holes
        .push(Hole {
            points: vec![
                DVec2::new(0.1, 0.1),
                DVec2::new(0.1, 0.3),
                DVec2::new(0.2, 0.3),
                DVec2::new(0.2, 0.1),
            ],
            edges: (0..4).map(|i| format!("hole_{i}")).collect(),
        });
    let output = report(&template).unwrap();
    close(output.net_area_m2, 0.24 - 0.02);
    close(output.mass_kg, 0.22 * 0.2);
    let ellipse = report(&polygon(Primitive::Ellipse)).unwrap();
    let ideal = std::f64::consts::PI * 0.2 * 0.3;
    assert!((ellipse.net_area_m2 - ideal).abs() < 0.001 * ideal);
    assert!(ellipse.net_area_m2 < 0.2);
    close(ellipse.mass_kg, ellipse.net_area_m2 * 0.2);
}

#[test]
fn grain_board_and_rigid_placement_do_not_change_quantities() {
    let mut template = GarmentTemplate::two_panel_demo();
    let before = report(&template).unwrap();
    for panel in &mut template.panels {
        panel.layout_2d = Some(DVec2::new(4., -2.));
        panel.layout_rotation_degrees = 47.;
        panel.placement.translation += DVec3::new(10., -20., 30.);
        panel.placement.rotation_degrees = DVec3::new(23., -57., 12.);
        panel.grain_degrees = 72.;
    }
    assert_eq!(report(&template).unwrap(), before);
}

#[test]
fn material_identity_and_density_edits_are_not_stale_or_grouped_by_name() {
    let mut template = GarmentTemplate::two_panel_demo();
    let mut fabric = template.fabrics[0].clone();
    fabric.id = "another".into();
    fabric.properties.density = 0.4;
    template.panels[1].fabric.clone_from(&fabric.id);
    template.fabrics.push(fabric);
    let mut unused = template.fabrics[0].clone();
    unused.id = "unused".into();
    template.fabrics.push(unused);
    let before = report(&template).unwrap();
    assert_eq!(before.fabrics.len(), 2);
    assert_eq!(before.fabrics[0].fabric_id, "another");
    assert_eq!(before.fabrics[0].fabric_name, before.fabrics[1].fabric_name);
    close(before.mass_kg, 0.24 * 0.6);
    template.fabrics[0].properties.density *= 3.;
    let after = report(&template).unwrap();
    close(after.panels[0].mass_kg, before.panels[0].mass_kg * 3.);
    close(after.panels[1].mass_kg, before.panels[1].mass_kg);
    close(after.net_area_m2, before.net_area_m2);
    let compiled = template.compile().unwrap();
    close(after.mass_kg, compiled.mesh.masses.iter().sum());
}

#[test]
fn invalid_numbers_and_aggregate_overflow_return_errors_without_mutation() {
    for density in [0., -1., f64::NAN, f64::INFINITY] {
        let mut template = GarmentTemplate::two_panel_demo();
        template.fabrics[0].properties.density = density;
        assert!(report(&template).is_err());
    }
    let mut template = GarmentTemplate::two_panel_demo();
    template.seams.clear();
    template.fabrics[0].properties.density = f64::MAX;
    for panel in &mut template.panels {
        panel.pattern.width = 1.;
        panel.pattern.height = 1.;
    }
    let before = serde_json::to_vec(&template).unwrap();
    assert!(report(&template).is_err());
    assert_eq!(before, serde_json::to_vec(&template).unwrap());
}

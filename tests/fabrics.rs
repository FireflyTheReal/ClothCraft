use glam::DVec3;
use marvelous_cloth::{
    authoring::{Command, apply},
    energy,
    fabrics::{self, PRESETS, REFERENCE_TRACTION},
    garment::GarmentTemplate,
    model::{Pattern, Scene},
};

#[test]
fn preset_stretch_matches_actual_membrane_force_and_surface_mass() {
    for preset in PRESETS {
        let p = preset.properties();
        let material = p.bind(0.);
        material.validate().unwrap();
        let (mesh, _) = Pattern {
            width: 0.2,
            height: 0.3,
            nx: 2,
            ny: 2,
        }
        .build(&material, Scene::Sphere)
        .unwrap();
        assert!((mesh.masses.iter().sum::<f64>() - 0.06 * preset.gsm / 1000.).abs() < 1e-12);
        let triangle = &mesh.triangles[0];
        for axis in 0..2 {
            let lambda = 1. + preset.extension[axis] / 100.;
            let at = |l: f64| {
                let x = triangle.vertices.map(|i| {
                    let mut q = mesh.uv[i];
                    q[axis] *= l;
                    [q.x, q.y, 0.]
                });
                energy::membrane(x, triangle, &material)
            };
            let traction = (at(lambda + 1e-6) - at(lambda - 1e-6)) / (2e-6 * triangle.area);
            assert!(
                (traction - REFERENCE_TRACTION).abs() < 1e-5,
                "{} {axis} {traction}",
                preset.id
            );
            let k = [p.warp, p.weft][axis];
            assert!((fabrics::extension_percent(k, &p) - preset.extension[axis]).abs() < 1e-10);
        }
    }
    for invalid in [0., -1., f64::NAN, f64::INFINITY, 101.] {
        assert!(fabrics::stiffness_for_extension(invalid, &PRESETS[0].properties()).is_err());
    }
}

#[test]
fn material_edit_updates_mass_and_only_selected_binding_while_preserving_drape() {
    let t = GarmentTemplate::two_panel_demo();
    let g = t.compile().unwrap();
    let mut state = g.initial_state.clone();
    for i in 0..state.x.len() {
        if g.mesh.pins[i].is_none() {
            state.x[i] += DVec3::new(0.01, -0.03, 0.02);
            state.v[i] = DVec3::Y * -0.1;
        }
    }
    let p = PRESETS[3].properties();
    let command = |all_panels| Command::Fabric {
        panel: t.panels[0].id.clone(),
        name: "Denim test".into(),
        properties: p.clone(),
        all_panels,
    };
    let edit = apply(&t, command(false)).unwrap();
    assert!(edit.reset_simulation);
    assert_ne!(
        edit.template.panels[0].fabric,
        edit.template.panels[1].fabric
    );
    assert_eq!(edit.template.panels[1].fabric, t.panels[1].fabric);
    assert_eq!(edit.garment.materials[1].warp, g.materials[1].warp);
    assert_eq!(edit.garment.mesh.uv, g.mesh.uv);
    assert_eq!(edit.state_after(&g, &state).x, state.x);
    assert_eq!(edit.state_after(&g, &state).v, state.v);
    let range = edit.garment.panels[0].vertices.clone();
    let ratio = p.density / g.materials[0].density;
    for i in range {
        assert!((edit.garment.mesh.masses[i] - g.mesh.masses[i] * ratio).abs() < 1e-12);
    }
    let all = apply(&t, command(true)).unwrap();
    assert!(
        all.garment
            .materials
            .iter()
            .all(|m| m.warp == p.warp && m.density == p.density)
    );
    let roundtrip: GarmentTemplate =
        serde_json::from_str(&serde_json::to_string(&all.template).unwrap()).unwrap();
    assert_eq!(
        roundtrip.compile().unwrap().mesh.masses,
        all.garment.mesh.masses
    );
    let mut bad = p;
    bad.density = -0.1;
    let before = serde_json::to_vec(&t).unwrap();
    assert!(
        apply(
            &t,
            Command::Fabric {
                panel: t.panels[0].id.clone(),
                name: "bad".into(),
                properties: bad,
                all_panels: true
            }
        )
        .is_err()
    );
    assert_eq!(serde_json::to_vec(&t).unwrap(), before);
}

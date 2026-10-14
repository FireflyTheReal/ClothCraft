use glam::{DVec2, DVec3};
use marvelous_cloth::{
    authoring::{Command, apply},
    garment::{GarmentTemplate, meshing::MeshingSettings},
};

#[test]
fn adding_and_sewing_a_panel_keep_the_existing_drape() {
    let t = GarmentTemplate::two_panel_demo();
    let g = t.compile().unwrap();
    let mut state = g.initial_state.clone();
    state.time = 3.;
    for i in 0..state.x.len() {
        if g.mesh.pins[i].is_none() {
            state.x[i] += DVec3::new(0.01, -0.02, 0.03);
            state.v[i] = DVec3::new(0., -0.01, 0.);
        }
    }
    let edit = apply(
        &t,
        Command::Polygon {
            points: vec![
                DVec2::ZERO,
                DVec2::new(0.2, 0.),
                DVec2::new(0.2, 0.3),
                DVec2::new(0., 0.3),
            ],
            tangents: vec![],
            meshing: MeshingSettings::default(),
            fabric: t.fabrics[0].id.clone(),
            replace: false,
        },
    )
    .unwrap();
    let next = edit.state_after(&g, &state);
    assert!(edit.reset_simulation); // Rebuild solver topology, but preserve compatible state.
    assert_eq!(next.time, state.time);
    assert_eq!(next.x[..state.x.len()], state.x);
    assert_eq!(next.v[..state.v.len()], state.v);
    let new = &edit.garment.panels[2];
    assert_eq!(
        next.x[new.vertices.clone()],
        edit.garment.initial_state.x[new.vertices.clone()]
    );

    let remove_seam = apply(
        &edit.template,
        Command::RemoveSeam {
            seam: edit.template.seams[0].id.clone(),
        },
    )
    .unwrap();
    let after = remove_seam.state_after(&edit.garment, &next);
    assert_eq!(after.x, next.x);
    assert_eq!(after.v, next.v);
    let seam = &edit.template.seams[0];
    let sew = apply(
        &remove_seam.template,
        Command::Sew {
            a: seam.a.clone(),
            b: seam.b.clone(),
            reverse: seam.reverse_b,
            ease: seam.allow_ease,
        },
    )
    .unwrap();
    assert_eq!(sew.state_after(&remove_seam.garment, &after).x, next.x);
}

#[test]
fn deleting_and_moving_panels_maps_state_by_identity_and_local_topology() {
    let mut t = GarmentTemplate::two_panel_demo();
    for p in &mut t.panels {
        p.pins.clear();
    }
    let g = t.compile().unwrap();
    let mut state = g.initial_state.clone();
    for p in &mut state.x {
        *p += DVec3::Z * 0.02;
    }
    let old = &g.panels[1];
    let remove = apply(
        &t,
        Command::RemovePanel {
            panel: t.panels[0].id.clone(),
        },
    )
    .unwrap();
    assert_eq!(
        remove.state_after(&g, &state).x,
        state.x[old.vertices.clone()]
    );
    let mut placement = t.panels[0].placement.clone();
    placement.translation += DVec3::X * 0.1;
    let moved = apply(
        &t,
        Command::Place {
            panel: t.panels[0].id.clone(),
            placement,
        },
    )
    .unwrap();
    let next = moved.state_after(&g, &state);
    let first = &g.panels[0];
    assert_eq!(
        next.x[first.vertices.clone()],
        moved.garment.initial_state.x[first.vertices.clone()]
    );
    assert_eq!(next.x[old.vertices.clone()], state.x[old.vertices.clone()]);
}

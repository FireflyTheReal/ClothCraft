use glam::{DVec2, DVec3};
use marvelous_cloth::{
    authoring::{self, Command},
    fitting::FittingProject,
    garment::{
        curves,
        meshing::MeshingSettings,
        presets::{self, GarmentKind},
        seam::{BoundaryRef, Side},
    },
};

fn drawing() -> marvelous_cloth::garment::GarmentTemplate {
    authoring::apply(
        &presets::basic(GarmentKind::Tshirt, 0),
        Command::Polygon {
            points: vec![
                DVec2::ZERO,
                DVec2::new(0.4, 0.),
                DVec2::new(0.4, 0.5),
                DVec2::new(0., 0.5),
            ],
            tangents: vec![
                DVec2::ZERO,
                DVec2::new(0.08, 0.08),
                DVec2::ZERO,
                DVec2::ZERO,
            ],
            meshing: MeshingSettings {
                target_edge_m: 0.04,
                ..Default::default()
            },
            fabric: "woven-reference".into(),
            replace: true,
        },
    )
    .unwrap()
    .template
}
#[test]
fn bezier_polyline_bounds_error_and_keeps_collinear_overshoot() {
    let p = [
        DVec2::ZERO,
        DVec2::new(0., 0.8),
        DVec2::new(1., -0.5),
        DVec2::X,
    ];
    let line = curves::flatten(p, curves::FLATNESS_M).unwrap();
    for i in 0..=2000 {
        let v = curves::cubic(p, i as f64 / 2000.);
        let distance = line
            .windows(2)
            .map(|pair| {
                let d = pair[1] - pair[0];
                v.distance(pair[0] + d * ((v - pair[0]).dot(d) / d.length_squared()).clamp(0., 1.))
            })
            .fold(f64::INFINITY, f64::min);
        assert!(distance <= curves::FLATNESS_M + 1e-12);
    }
    let line =
        curves::flatten([DVec2::ZERO, DVec2::X * 3., DVec2::X * 3., DVec2::X], 0.001).unwrap();
    assert!(line.iter().any(|p| p.x > 2.));
    assert!(curves::flatten(p, 0.).is_err());
}
#[test]
fn curved_boundaries_keep_ids_lengths_and_mass_under_remeshing() {
    let t = drawing();
    let a = t.compile().unwrap();
    let mut fine = t.clone();
    fine.panels[0]
        .polygon
        .as_mut()
        .unwrap()
        .meshing
        .as_mut()
        .unwrap()
        .target_edge_m = 0.02;
    let b = fine.compile().unwrap();
    let mass = |g: &marvelous_cloth::garment::CompiledGarment| g.mesh.masses.iter().sum::<f64>();
    assert!((mass(&a) - mass(&b)).abs() < 1e-12);
    assert!(a.panels[0].boundaries["edge_1"].len() > 2);
    for name in &t.panels[0].polygon.as_ref().unwrap().edges {
        let length = |g: &marvelous_cloth::garment::CompiledGarment| {
            g.panels[0].boundaries[name]
                .windows(2)
                .map(|p| g.mesh.uv[p[0]].distance(g.mesh.uv[p[1]]))
                .sum::<f64>()
        };
        assert!((length(&a) - length(&b)).abs() < 1e-10);
    }
    let serialized = serde_json::to_string(&t).unwrap();
    let read: marvelous_cloth::garment::GarmentTemplate =
        serde_json::from_str(&serialized).unwrap();
    assert_eq!(a.mesh.uv, read.compile().unwrap().mesh.uv);
}
#[test]
fn board_motion_is_not_a_physical_transform_and_does_not_stale_state_cache() {
    let t = drawing();
    let a = t.compile().unwrap();
    let edit = authoring::apply(
        &t,
        Command::BoardTransform {
            panel: t.panels[0].id.clone(),
            origin: DVec2::new(-2., 3.),
            rotation_degrees: 73.,
        },
    )
    .unwrap();
    assert!(!edit.reset_simulation);
    assert_eq!(a.mesh.uv, edit.garment.mesh.uv);
    assert_eq!(a.initial_state.x, edit.garment.initial_state.x);
    assert_eq!(a.mesh.masses, edit.garment.mesh.masses);
    assert!(authoring::same_physics(&t, &edit.template));
    let mut p = FittingProject::basic(
        GarmentKind::Tshirt,
        0,
        "assets/makehuman/mannequin.obj".into(),
    );
    p.template = t;
    p.state = Some(a.initial_state);
    let file = std::env::temp_dir().join(format!(
        "clothcraft-layout-test-{}.json",
        std::process::id()
    ));
    p.save(&file).unwrap();
    let mut read = FittingProject::load(&file).unwrap();
    std::fs::remove_file(file).unwrap();
    read.template = serde_json::from_str(&serde_json::to_string(&edit.template).unwrap()).unwrap();
    assert_eq!(read.template.panels[0].layout_rotation_degrees, 73.);
    assert!(read.compile().is_ok());
}
#[test]
fn curve_sewing_and_rest_edits_are_transactional() {
    let t = drawing();
    let id = t.panels[0].id.clone();
    let t = authoring::apply(&t, Command::Duplicate { panel: id.clone() })
        .unwrap()
        .template;
    let edge = |panel: String| BoundaryRef {
        panel,
        side: Side::Top,
        edge: Some("edge_1".into()),
        range: [0., 1.],
    };
    let t = authoring::apply(
        &t,
        Command::Sew {
            a: edge(id.clone()),
            b: edge(t.panels[1].id.clone()),
            reverse: false,
            ease: false,
        },
    )
    .unwrap()
    .template;
    let mut poly = t.panels[0].polygon.clone().unwrap();
    poly.curves.get_mut("edge_1").unwrap()[0] += DVec2::new(0., 0.04);
    assert!(
        authoring::apply(
            &t,
            Command::Shape {
                panel: id.clone(),
                polygon: poly
            }
        )
        .is_err()
    );
    let mut poly = t.panels[0].polygon.clone().unwrap();
    poly.curves.insert("missing_edge".into(), [DVec2::ZERO; 2]);
    assert!(
        authoring::apply(
            &t,
            Command::Shape {
                panel: id,
                polygon: poly
            }
        )
        .is_err()
    );
    assert!(t.compile().is_ok());
}
#[test]
fn moving_anchor_can_rebase_local_uv_without_moving_untouched_world_points() {
    let mut t = drawing();
    t.panels[0].layout_rotation_degrees = 37.;
    t.panels[0].layout_2d = Some(DVec2::new(-2., 3.));
    let panel = &t.panels[0];
    let mut poly = panel.polygon.clone().unwrap();
    let untouched = poly.points[2];
    let board_before =
        panel.layout_2d.unwrap() + authoring::rotate_2d(untouched, panel.layout_rotation_degrees);
    let before = panel
        .placement
        .transform(DVec3::new(untouched.x, -untouched.y, 0.));
    poly.points[0].x -= 0.2;
    let edit = authoring::apply(
        &t,
        Command::Shape {
            panel: panel.id.clone(),
            polygon: poly,
        },
    )
    .unwrap();
    let panel = &edit.template.panels[0];
    let untouched = panel.polygon.as_ref().unwrap().points[2];
    let board_after =
        panel.layout_2d.unwrap() + authoring::rotate_2d(untouched, panel.layout_rotation_degrees);
    assert!(board_after.distance(board_before) < 1e-12);
    assert!(
        before.distance(
            panel
                .placement
                .transform(DVec3::new(untouched.x, -untouched.y, 0.))
        ) < 1e-12
    );
    assert!(edit.reset_simulation);
}

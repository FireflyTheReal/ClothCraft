use glam::{DVec2, DVec3};
use marvelous_cloth::{
    authoring::{self, Command, History},
    garment::{
        GarmentTemplate, export,
        meshing::MeshingSettings,
        presets::{self, GarmentKind},
        seam::{BoundaryRef, Side},
        shapes::{self, BooleanOp, MirrorAxis, Primitive},
    },
};

fn settings() -> MeshingSettings {
    MeshingSettings {
        target_edge_m: 0.08,
        ..Default::default()
    }
}
fn add(
    t: &GarmentTemplate,
    kind: Primitive,
    size: DVec2,
    origin: DVec2,
    replace: bool,
) -> GarmentTemplate {
    authoring::apply(
        t,
        Command::Primitive {
            kind,
            size,
            origin,
            replace,
            sweep_degrees: 180.,
            meshing: settings(),
            fabric: "woven-reference".into(),
        },
    )
    .unwrap()
    .template
}
fn square() -> GarmentTemplate {
    add(
        &presets::basic(GarmentKind::Tshirt, 0),
        Primitive::Rectangle,
        DVec2::splat(0.6),
        DVec2::ZERO,
        true,
    )
}
fn area(t: &GarmentTemplate) -> f64 {
    t.compile()
        .unwrap()
        .mesh
        .triangles
        .iter()
        .map(|t| t.area)
        .sum()
}
fn edge(panel: &str, name: &str) -> BoundaryRef {
    BoundaryRef {
        panel: panel.into(),
        edge: Some(name.into()),
        side: Side::Top,
        range: [0., 1.],
    }
}
fn with_line() -> GarmentTemplate {
    let t = square();
    authoring::apply(
        &t,
        Command::InternalLine {
            panel: t.panels[0].id.clone(),
            points: vec![DVec2::new(0.1, 0.2), DVec2::new(0.5, 0.2)],
            tangents: vec![DVec2::new(0.1, 0.12), DVec2::new(0.1, -0.12)],
        },
    )
    .unwrap()
    .template
}

#[test]
fn circle_and_arc_have_expected_area_and_curved_edges() {
    for (kind, expected) in [
        (Primitive::Ellipse, std::f64::consts::PI * 0.09),
        (Primitive::Sector, std::f64::consts::PI * 0.045),
    ] {
        let t = add(&square(), kind, DVec2::splat(0.6), DVec2::ZERO, true);
        assert!((area(&t) - expected).abs() < 2e-4);
        assert!(!t.panels[0].polygon.as_ref().unwrap().curves.is_empty());
        let g = t.compile().unwrap();
        export::grain_guides_obj(&t, &g, &g.initial_state).unwrap();
    }
    assert!(shapes::primitive(Primitive::Sector, DVec2::ONE, 0., settings()).is_err());
    assert!(
        shapes::primitive(Primitive::Ellipse, DVec2::splat(f64::NAN), 180., settings()).is_err()
    );
}

#[test]
fn boolean_rectangles_conserve_area_for_every_operation() {
    let t = square();
    let t = add(
        &t,
        Primitive::Rectangle,
        DVec2::splat(0.6),
        DVec2::new(0.3, 0.),
        false,
    );
    for (op, expected) in [
        (BooleanOp::Union, 0.54),
        (BooleanOp::Intersection, 0.18),
        (BooleanOp::Difference, 0.18),
    ] {
        let out = authoring::apply(
            &t,
            Command::Boolean {
                a: t.panels[0].id.clone(),
                b: t.panels[1].id.clone(),
                op,
                keep_tool: false,
            },
        )
        .unwrap();
        assert!((area(&out.template) - expected).abs() < 1e-8);
        assert_eq!(out.template.panels.len(), 1);
        let masses: f64 = out.garment.mesh.masses.iter().sum();
        assert!((masses - expected * out.template.fabrics[0].properties.density).abs() < 1e-10);
    }
}

#[test]
fn subtraction_has_a_real_hole_and_exports_split_grain_guides() {
    let t = add(
        &square(),
        Primitive::Rectangle,
        DVec2::splat(0.2),
        DVec2::splat(0.2),
        false,
    );
    let edit = authoring::apply(
        &t,
        Command::Boolean {
            a: t.panels[0].id.clone(),
            b: t.panels[1].id.clone(),
            op: BooleanOp::Difference,
            keep_tool: false,
        },
    )
    .unwrap();
    let t = edit.template;
    let g = edit.garment;
    let hole = &t.panels[0].polygon.as_ref().unwrap().holes[0];
    assert!((area(&t) - 0.32).abs() < 1e-9);
    assert!(g.embed_point(&t.panels[0].id, DVec2::splat(0.3)).is_err());
    for tri in &g.mesh.triangles {
        assert!(!marvelous_cloth::garment::domain::inside(
            tri.rest.iter().sum::<DVec2>() / 3.,
            &hole.points
        ));
    }
    assert!(export::pattern_svg(&t).contains("fill-rule=\"evenodd\""));
    let guides = export::grain_guides_obj(&t, &g, &g.initial_state).unwrap();
    assert_eq!(guides.lines().filter(|l| l.starts_with("l ")).count(), 4);
    for name in &hole.edges {
        assert!(g.panels[0].boundaries[name].len() >= 2);
    }
    let roundtrip: GarmentTemplate =
        serde_json::from_slice(&serde_json::to_vec(&t).unwrap()).unwrap();
    assert_eq!(roundtrip.compile().unwrap().mesh.uv, g.mesh.uv);
}

#[test]
fn boolean_respects_board_rotation_and_disconnected_islands() {
    let t = add(
        &square(),
        Primitive::Rectangle,
        DVec2::splat(0.6),
        DVec2::new(0.6, 0.),
        false,
    );
    let mut rotated = t.clone();
    rotated.panels[1].layout_rotation_degrees = 90.;
    let out = authoring::apply(
        &rotated,
        Command::Boolean {
            a: t.panels[0].id.clone(),
            b: t.panels[1].id.clone(),
            op: BooleanOp::Intersection,
            keep_tool: false,
        },
    )
    .unwrap();
    assert!((area(&out.template) - 0.36).abs() < 1e-9);
    let mut apart = t;
    apart.panels[1].layout_2d = Some(DVec2::new(1., 0.));
    let out = authoring::apply(
        &apart,
        Command::Boolean {
            a: apart.panels[0].id.clone(),
            b: apart.panels[1].id.clone(),
            op: BooleanOp::Union,
            keep_tool: false,
        },
    )
    .unwrap();
    assert_eq!(out.template.panels.len(), 2);
    // The overlay float adapter quantizes its coordinate domain (~nanometres here).
    assert!((area(&out.template) - 0.72).abs() < 1e-8);
}

#[test]
fn double_mirror_restores_curves_lines_pins_and_seam_mapping() {
    let mut t = with_line();
    t.panels[0].pin_points = vec![DVec2::ZERO];
    t.panels[0].grain_degrees = 32.;
    t = add(
        &t,
        Primitive::Ellipse,
        DVec2::splat(0.3),
        DVec2::new(1., 0.),
        false,
    );
    let a = t.panels[0].id.clone();
    let b = t.panels[1].id.clone();
    t = authoring::apply(
        &t,
        Command::Sew {
            a: edge(&a, "edge_1"),
            b: edge(&b, "edge_1"),
            reverse: false,
            ease: true,
        },
    )
    .unwrap()
    .template;
    t.seams[0].a.range = [0.2, 0.9];
    t.seams[0].notches = vec![[0.3, 0.4], [0.7, 0.8]];
    for axis in [MirrorAxis::X, MirrorAxis::Y] {
        for id in [&a, &b] {
            let once = authoring::apply(
                &t,
                Command::Mirror {
                    panel: id.clone(),
                    axis,
                    copy_side: None,
                },
            )
            .unwrap();
            assert!((area(&once.template) - area(&t)).abs() < 1e-9);
            let twice = authoring::apply(
                &once.template,
                Command::Mirror {
                    panel: id.clone(),
                    axis,
                    copy_side: None,
                },
            )
            .unwrap()
            .template;
            for (p, q) in t.panels.iter().zip(&twice.panels) {
                let (p, q) = (p.polygon.as_ref().unwrap(), q.polygon.as_ref().unwrap());
                assert_eq!(p.edges, q.edges);
                for (p, q) in p
                    .outer_points()
                    .unwrap()
                    .iter()
                    .zip(q.outer_points().unwrap())
                {
                    assert!(p.distance(q) < 1e-12);
                }
                for (p, q) in p.lines.iter().zip(&q.lines) {
                    for (p, q) in p.samples().unwrap().iter().zip(q.samples().unwrap()) {
                        assert!(p.distance(q) < 1e-12);
                    }
                }
            }
            assert_eq!(t.seams[0].reverse_b, twice.seams[0].reverse_b);
            for (p, q) in t.seams[0]
                .notches
                .iter()
                .flatten()
                .zip(twice.seams[0].notches.iter().flatten())
            {
                assert!((p - q).abs() < 1e-12);
            }
            assert!((t.panels[0].grain_degrees - twice.panels[0].grain_degrees).abs() < 1e-12);
            assert!(twice.panels[0].pin_points[0].length() < 1e-12);
        }
    }
}

#[test]
fn mirrored_copies_are_independent_and_undo_restores_original() {
    let t = with_line();
    for (axis, side) in [
        (MirrorAxis::X, -1),
        (MirrorAxis::X, 1),
        (MirrorAxis::Y, -1),
        (MirrorAxis::Y, 1),
    ] {
        let out = authoring::apply(
            &t,
            Command::Mirror {
                panel: t.panels[0].id.clone(),
                axis,
                copy_side: Some(side),
            },
        )
        .unwrap();
        assert_ne!(out.template.panels[0].id, out.template.panels[1].id);
        let delta = authoring::layout(&out.template, 1) - authoring::layout(&out.template, 0);
        assert!(
            (if axis == MirrorAxis::X {
                delta.x
            } else {
                delta.y
            }) * side as f64
                > 0.
        );
        let mut history = History::default();
        history.record(t.clone());
        let undo = history.restore(&out.template, false).unwrap();
        assert_eq!(
            serde_json::to_vec(&undo.template).unwrap(),
            serde_json::to_vec(&t).unwrap()
        );
    }
}

#[test]
fn internal_curve_is_constrained_inside_material_and_survives_remeshing() {
    let t = with_line();
    let line = &t.panels[0].polygon.as_ref().unwrap().lines[0];
    for (target, anisotropy, axis) in [
        (0.08, 1., 0.),
        (0.045, 1., 0.),
        (0.025, 1., 0.),
        (0.035, 2., 37.),
    ] {
        let mut t = t.clone();
        t.panels[0]
            .polygon
            .as_mut()
            .unwrap()
            .meshing
            .as_mut()
            .unwrap()
            .target_edge_m = target;
        let settings = t.panels[0]
            .polygon
            .as_mut()
            .unwrap()
            .meshing
            .as_mut()
            .unwrap();
        settings.anisotropy = anisotropy;
        settings.axis_degrees = axis;
        let g = t.compile().unwrap();
        assert!((g.mesh.triangles.iter().map(|t| t.area).sum::<f64>() - 0.36).abs() < 1e-9);
        let chain = &g.panels[0].boundaries[&line.id];
        let samples = line.samples().unwrap();
        assert!(g.mesh.uv[chain[0]].distance(samples[0]) < 1e-9);
        assert!(g.mesh.uv[*chain.last().unwrap()].distance(*samples.last().unwrap()) < 1e-9);
        for pair in chain.windows(2) {
            let adjacent = g
                .mesh
                .triangles
                .iter()
                .filter(|tri| pair.iter().all(|v| tri.vertices.contains(v)))
                .count();
            assert_eq!(
                adjacent, 2,
                "An interior constraint must have fabric on both sides"
            );
        }
    }
}

#[test]
fn ruffle_is_longer_material_with_a_real_conservative_seam() {
    let t = with_line();
    let p = &t.panels[0];
    let id = &p.polygon.as_ref().unwrap().lines[0].id;
    let out = authoring::apply(
        &t,
        Command::Ruffle {
            panel: p.id.clone(),
            line: id.clone(),
            width: 0.08,
            ratio: 1.75,
        },
    )
    .unwrap();
    let g = &out.garment;
    let seam = &g.seams[0];
    assert!((seam.lengths[1] / seam.lengths[0] - 1.75).abs() < 1e-10);
    assert!((area(&out.template) - 0.36 - seam.lengths[1] * 0.08).abs() < 1e-9);
    let x = &g.initial_state.x;
    assert!(seam.energy(x) > 0.);
    let mut gradient = vec![0.; x.len() * 3];
    for e in seam.elements(x) {
        for (d, v) in e.dofs.iter().zip(e.gradient) {
            gradient[*d] += v;
        }
    }
    let force: DVec3 = gradient
        .chunks_exact(3)
        .map(|p| DVec3::new(p[0], p[1], p[2]))
        .sum();
    assert!(force.length() < 1e-7);
    for (i, &analytic) in gradient
        .iter()
        .enumerate()
        .filter(|(_, v)| v.abs() > 1e-6)
        .take(12)
    {
        let mut plus = x.clone();
        let mut minus = x.clone();
        plus[i / 3][i % 3] += 1e-6;
        minus[i / 3][i % 3] -= 1e-6;
        let fd = (seam.energy(&plus) - seam.energy(&minus)) / 2e-6;
        assert!((fd - analytic).abs() < 1e-5 * (1. + analytic.abs()));
    }
    assert!(seam.samples.iter().all(|s| s.point.terms.len() <= 4));
    assert!(export::pattern_svg(&out.template).contains("stroke-dasharray"));
    assert!(
        authoring::apply(
            &out.template,
            Command::RemoveLine {
                panel: p.id.clone(),
                line: id.clone()
            }
        )
        .is_err()
    );
}

#[test]
fn crossing_outside_and_duplicate_lines_fail_without_mutation() {
    let t = with_line();
    let before = serde_json::to_vec(&t).unwrap();
    for points in [
        vec![DVec2::new(-0.1, 0.2), DVec2::splat(0.4)],
        vec![DVec2::new(0.3, 0.1), DVec2::new(0.3, 0.5)],
        vec![DVec2::splat(0.2); 2],
    ] {
        assert!(
            authoring::apply(
                &t,
                Command::InternalLine {
                    panel: t.panels[0].id.clone(),
                    tangents: vec![DVec2::ZERO; points.len()],
                    points
                }
            )
            .is_err()
        );
    }
    assert_eq!(before, serde_json::to_vec(&t).unwrap());
}

#[test]
fn booleans_refuse_dropping_lines_or_empty_results_atomically() {
    let t = add(
        &with_line(),
        Primitive::Rectangle,
        DVec2::ONE,
        DVec2::new(2., 0.),
        false,
    );
    let before = serde_json::to_vec(&t).unwrap();
    assert!(
        authoring::apply(
            &t,
            Command::Boolean {
                a: t.panels[0].id.clone(),
                b: t.panels[1].id.clone(),
                op: BooleanOp::Union,
                keep_tool: false
            }
        )
        .is_err()
    );
    assert_eq!(before, serde_json::to_vec(&t).unwrap());
    let t = add(
        &square(),
        Primitive::Rectangle,
        DVec2::ONE,
        DVec2::new(2., 0.),
        false,
    );
    assert!(
        authoring::apply(
            &t,
            Command::Boolean {
                a: t.panels[0].id.clone(),
                b: t.panels[1].id.clone(),
                op: BooleanOp::Intersection,
                keep_tool: false
            }
        )
        .is_err()
    );
}

#[test]
fn absent_extensions_keep_legacy_serialization_unchanged() {
    let t = square();
    let value = serde_json::to_value(t).unwrap();
    assert!(value["panels"][0]["polygon"].get("holes").is_none());
    assert!(value["panels"][0]["polygon"].get("lines").is_none());
}

#[test]
fn circular_cutout_and_separate_line_share_one_material_domain() {
    let t = add(
        &square(),
        Primitive::Ellipse,
        DVec2::splat(0.2),
        DVec2::splat(0.2),
        false,
    );
    let t = authoring::apply(
        &t,
        Command::Boolean {
            a: t.panels[0].id.clone(),
            b: t.panels[1].id.clone(),
            op: BooleanOp::Difference,
            keep_tool: false,
        },
    )
    .unwrap()
    .template;
    let t = authoring::apply(
        &t,
        Command::InternalLine {
            panel: t.panels[0].id.clone(),
            points: vec![DVec2::new(0.08, 0.1), DVec2::new(0.52, 0.1)],
            tangents: vec![DVec2::ZERO; 2],
        },
    )
    .unwrap()
    .template;
    let g = t.compile().unwrap();
    assert!((area(&t) - (0.36 - std::f64::consts::PI * 0.01)).abs() < 2e-4);
    assert!(g.embed_point(&t.panels[0].id, DVec2::splat(0.3)).is_err());
    export::grain_guides_obj(&t, &g, &g.initial_state).unwrap();
    let axis = MirrorAxis::Y;
    let mirrored = authoring::apply(
        &t,
        Command::Mirror {
            panel: t.panels[0].id.clone(),
            axis,
            copy_side: None,
        },
    )
    .unwrap();
    assert!((area(&mirrored.template) - area(&t)).abs() < 1e-9);
}

#[test]
fn internal_path_rejects_adjacent_backtracking() {
    let t = square();
    assert!(
        authoring::apply(
            &t,
            Command::InternalLine {
                panel: t.panels[0].id.clone(),
                points: vec![
                    DVec2::new(0.1, 0.1),
                    DVec2::new(0.5, 0.1),
                    DVec2::new(0.3, 0.1)
                ],
                tangents: vec![DVec2::ZERO; 3]
            }
        )
        .is_err()
    );
}

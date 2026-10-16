use glam::{DVec2, DVec3};
use marvelous_cloth::{
    authoring::{self, Command, History, clipboard::PatternClipboard},
    garment::{
        GarmentTemplate,
        seam::{BoundaryRef, Side},
    },
};

fn edge(panel: &str, range: [f64; 2], reversed: bool) -> BoundaryRef {
    BoundaryRef {
        panel: panel.into(),
        edge: None,
        side: if panel == "left" {
            Side::Right
        } else {
            Side::Left
        },
        range,
        reversed,
    }
}
fn empty() -> GarmentTemplate {
    let mut t = GarmentTemplate::two_panel_demo();
    t.seams.clear();
    t
}
#[test]
fn partial_seams_leave_the_remainder_free_and_roundtrip_with_undo() {
    let t = empty();
    let a = edge("left", [0., 1. / 6.], false);
    let b = edge("right", [0., 1. / 6.], false);
    let edit = authoring::apply(
        &t,
        Command::Sew {
            a,
            b,
            reverse: true,
            ease: false,
        },
    )
    .unwrap();
    let g = &edit.garment;
    let s = &g.seams[0];
    assert!((s.lengths[0] - 0.1).abs() < 1e-12);
    let mut x = g.initial_state.x.clone();
    let baseline = s.energy(&x);
    // Move a vertex strictly outside the sewn interval and its supporting elements.
    let far = g.panels[0]
        .vertices
        .clone()
        .max_by(|a, b| g.mesh.uv[*a].y.total_cmp(&g.mesh.uv[*b].y))
        .unwrap();
    x[far] += DVec3::splat(10.);
    assert_eq!(s.energy(&x), baseline);
    let mut history = History::default();
    history.record(t.clone());
    let undone = history.restore(&edit.template, false).unwrap();
    assert!(undone.template.seams.is_empty());
    let redone = history.restore(&undone.template, true).unwrap();
    assert_eq!(redone.template.seams[0].a.range, [0., 1. / 6.]);
    let json = serde_json::to_string(&edit.template).unwrap();
    let loaded: GarmentTemplate = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.compile().unwrap().seams[0].lengths, s.lengths);
    let second = authoring::apply(
        &loaded,
        Command::Sew {
            a: edge("left", [0.5, 1.], false),
            b: edge("right", [0.5, 1.], false),
            reverse: true,
            ease: false,
        },
    )
    .unwrap();
    assert_eq!(second.garment.seams.len(), 2);
}

fn chain(multi_a: bool) -> GarmentTemplate {
    let mut t = empty();
    let mut third = t.panels[1].clone();
    third.id = "third".into();
    third.placement.translation.x += 0.6;
    t.panels.push(third);
    let a = if multi_a {
        vec![
            edge("left", [0., 0.5], false),
            edge("left", [0.5, 1.], false),
        ]
    } else {
        vec![edge("left", [0., 1.], false)]
    };
    authoring::apply(
        &t,
        Command::SewChain {
            seam: None,
            a,
            b: vec![
                edge("right", [0., 0.5], true),
                edge("third", [0., 0.5], false),
            ],
            reverse: false,
            ease: false,
        },
    )
    .unwrap()
    .template
}
#[test]
fn one_to_many_and_many_to_many_integrate_total_length_without_bridging_gaps() {
    for multi_a in [false, true] {
        let mut t = chain(multi_a);
        for ny in [3, 7, 13] {
            for p in &mut t.panels {
                p.pattern.ny = ny;
            }
            let g = t.compile().unwrap();
            let s = &g.seams[0];
            assert!((s.measure() - 0.6).abs() < 1e-12);
            assert!((s.samples.iter().map(|s| s.weight_m).sum::<f64>() - 0.6).abs() < 1e-12);
            let mut x = vec![DVec3::ZERO; g.mesh.uv.len()];
            for i in g.panels[0].vertices.clone() {
                x[i] = DVec3::Z * 0.2;
            }
            assert!((s.energy(&x) - 0.5 * s.stiffness * 0.6 * 0.04).abs() < 1e-10);
            // Each sample references A and exactly one of the separate B panels.
            for sample in &s.samples {
                let owns = |panel: usize| {
                    sample
                        .point
                        .terms
                        .iter()
                        .any(|(v, _)| g.panels[panel].vertices.contains(v))
                };
                assert!(owns(0));
                assert_ne!(owns(1), owns(2));
            }
            // Analytic force/Hessian stay consistent across chain boundaries.
            let elements = s.elements(&x);
            let v = s.samples[0].point.terms[0].0;
            let force: f64 = elements
                .iter()
                .map(|e| {
                    e.dofs
                        .iter()
                        .position(|d| *d == 3 * v + 2)
                        .map_or(0., |i| e.gradient[i])
                })
                .sum();
            let h = 1e-6;
            let mut plus = x.clone();
            let mut minus = x.clone();
            plus[v].z += h;
            minus[v].z -= h;
            assert!(((s.energy(&plus) - s.energy(&minus)) / (2. * h) - force).abs() < 1e-6);
            let shift = DVec3::new(5., -2., 9.);
            let moved: Vec<_> = x.iter().map(|p| *p + shift).collect();
            assert!((s.energy(&moved) - s.energy(&x)).abs() < 1e-8);
        }
    }
}
#[test]
fn chains_preserve_references_through_copy_delete_edit_and_reload() {
    let t = chain(true);
    let json = serde_json::to_string(&t).unwrap();
    let loaded: GarmentTemplate = serde_json::from_str(&json).unwrap();
    loaded.compile().unwrap();
    let ids = t.panels.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
    let copied = PatternClipboard::capture(&t, &ids).unwrap();
    let pasted = authoring::apply(
        &t,
        Command::Paste {
            clipboard: copied,
            center: DVec2::new(2., 2.),
        },
    )
    .unwrap();
    assert_eq!(pasted.template.seams.len(), 2);
    let s = &pasted.template.seams[1];
    assert!(s.references().all(|r| !ids.contains(&r.panel)));
    let removed = authoring::apply(
        &t,
        Command::RemovePanel {
            panel: "third".into(),
        },
    )
    .unwrap();
    assert!(removed.template.seams.is_empty());
    let s = &t.seams[0];
    let mut a: Vec<_> = s.side(false).cloned().collect();
    a[1].range = [0.4, 1.];
    let before = serde_json::to_string(&t).unwrap();
    assert!(
        authoring::apply(
            &t,
            Command::SewChain {
                seam: Some(s.id.clone()),
                a,
                b: s.side(true).cloned().collect(),
                reverse: false,
                ease: true
            }
        )
        .is_err()
    );
    assert_eq!(serde_json::to_string(&t).unwrap(), before);
}

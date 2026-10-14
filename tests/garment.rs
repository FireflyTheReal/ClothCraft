use glam::{DQuat, DVec3, EulerRot};
use marvelous_cloth::{
    energy,
    garment::{
        GarmentTemplate, export,
        seam::Side,
        template::{Fabric, FabricProperties},
    },
    solver::{ReferenceSolver, SolverSettings},
};

fn parallel(a: usize, b: usize) -> GarmentTemplate {
    let mut t = GarmentTemplate::two_panel_demo();
    t.panels[0].pattern.ny = a;
    t.panels[1].pattern.ny = b;
    t.panels[1].placement.rotation_degrees = DVec3::ZERO;
    t.panels[1].placement.translation = DVec3::new(0.1, 1.0, 0.0);
    t
}
fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() < tol, "{a} != {b}");
}

#[test]
fn line_energy_is_exact_and_independent_of_nonmatching_discretization() {
    for (a, b) in [(1, 1), (2, 7), (13, 3), (11, 17)] {
        let g = parallel(a, b).compile().unwrap();
        let seam = &g.seams[0];
        close(seam.measure(), 0.6, 1e-14);
        close(
            seam.energy(&g.initial_state.x),
            0.5 * 5000.0 * 0.6 * 0.1_f64.powi(2),
            1e-10,
        );
        let mut x = g.initial_state.x.clone();
        // A linear varying gap d(t)=.1+.2t integrates analytically over the seam.
        for i in g.panels[1].vertices.clone() {
            x[i].x += 0.2 * g.mesh.uv[i].y / 0.6;
        }
        let expected = 0.5 * 5000.0 * 0.6 * (0.01 + 0.02 + 0.04 / 3.0);
        close(seam.energy(&x), expected, 1e-10);
        close(seam.max_gap(&x), 0.3, 1e-13);
        for sample in &seam.samples {
            close(sample.point.terms.iter().map(|(_, c)| c).sum(), 0.0, 1e-14);
        }
    }
}

#[test]
fn seam_gradient_hessian_conservation_and_rigid_motion() {
    let g = parallel(2, 3).compile().unwrap();
    let seam = &g.seams[0];
    let x = &g.initial_state.x;
    let n = x.len() * 3;
    let mut grad = vec![0.0; n];
    let mut hess = vec![0.0; n * n];
    for e in seam.elements(x) {
        for (i, &d) in e.dofs.iter().enumerate() {
            grad[d] += e.gradient[i];
            for (j, &q) in e.dofs.iter().enumerate() {
                hess[d * n + q] += e.hessian[i * e.dofs.len() + j];
            }
        }
    }
    let eps = 1e-6;
    for d in 0..n {
        let mut a = x.clone();
        let mut b = x.clone();
        a[d / 3][d % 3] += eps;
        b[d / 3][d % 3] -= eps;
        close(
            (seam.energy(&a) - seam.energy(&b)) / (2.0 * eps),
            grad[d],
            2e-8,
        );
        let gradient = |x: &[DVec3]| {
            let mut v = vec![0.0; n];
            for e in seam.elements(x) {
                for (i, &d) in e.dofs.iter().enumerate() {
                    v[d] += e.gradient[i];
                }
            }
            v
        };
        let ga = gradient(&a);
        let gb = gradient(&b);
        for j in 0..n {
            close((ga[j] - gb[j]) / (2.0 * eps), hess[j * n + d], 2e-7);
            close(hess[j * n + d], hess[d * n + j], 1e-12);
        }
    }
    let forces: Vec<_> = grad
        .as_chunks::<3>()
        .0
        .iter()
        .map(|v| -DVec3::new(v[0], v[1], v[2]))
        .collect();
    assert!(forces.iter().sum::<DVec3>().length() < 1e-11);
    assert!(
        x.iter()
            .zip(&forces)
            .map(|(p, f)| p.cross(*f))
            .sum::<DVec3>()
            .length()
            < 1e-11
    );
    let direction: Vec<_> = (0..n).map(|i| (i as f64 * 1.31).sin()).collect();
    let quadratic: f64 = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| direction[i] * hess[i * n + j] * direction[j])
                .sum::<f64>()
        })
        .sum();
    assert!(quadratic >= -1e-12);
    let rotation = DQuat::from_rotation_y(0.71) * DQuat::from_rotation_z(1.23);
    let moved: Vec<_> = x
        .iter()
        .map(|p| rotation * (*p) + DVec3::new(2.0, -1.0, 4.0))
        .collect();
    close(seam.energy(&moved), seam.energy(x), 1e-10);
}

#[test]
fn reversal_trimming_notches_and_explicit_ease() {
    let mut t = parallel(3, 7);
    let g = t.compile().unwrap();
    close(g.max_seam_gap(&g.initial_state.x), 0.1, 1e-13);
    t.seams[0].reverse_b = false;
    let g = t.compile().unwrap();
    close(g.max_seam_gap(&g.initial_state.x), 0.37_f64.sqrt(), 1e-13);
}

#[test]
fn seam_display_guides_follow_trimming_reversal_and_notch_correspondence() {
    for reverse in [false, true] {
        let mut t = parallel(3, 7);
        t.seams[0].a.range = [0.2, 0.8];
        t.seams[0].b.range = [0.1, 0.7];
        t.seams[0].notches = vec![[0.4, 0.6]];
        t.seams[0].reverse_b = reverse;
        t.seams[0].allow_ease = true;
        let g = t.compile().unwrap();
        let guides = &g.seams[0].guides;
        assert_eq!(guides.len(), 9);
        for (i, pair) in guides.iter().enumerate() {
            let s = i as f64 / 8.;
            let u = if s <= 0.4 {
                s * 1.5
            } else {
                0.6 + (s - 0.4) * 2. / 3.
            };
            let point = |side: usize| {
                pair[side]
                    .iter()
                    .map(|(v, w)| g.mesh.uv[*v] * *w)
                    .sum::<glam::DVec2>()
            };
            close(point(0).y, 0.6 * (0.2 + 0.6 * s), 1e-13);
            close(
                point(1).y,
                0.6 * (1. - (0.1 + 0.6 * if reverse { 1. - u } else { u })),
                1e-13,
            );
            for side in pair {
                close(side.iter().map(|(_, w)| w).sum(), 1., 1e-14);
            }
        }
    }
}

#[test]
fn partial_edges_and_ease_keep_original_rest_patterns() {
    let mut t = parallel(3, 7);
    t.seams[0].a.range = [0.2, 0.8];
    t.seams[0].b.range = [0.2, 0.8];
    let g = t.compile().unwrap();
    close(g.seams[0].measure(), 0.36, 1e-14);
    close(g.seams[0].energy(&g.initial_state.x), 9.0, 1e-11);
    t.seams[0].notches = vec![[0.4, 0.6]];
    assert!(t.compile().unwrap_err().contains("local easing"));
    t.seams[0].allow_ease = true;
    let g = t.compile().unwrap();
    assert_eq!(g.warnings.len(), 1);
    close(g.mesh.triangles.iter().map(|t| t.area).sum(), 0.48, 1e-13);
    t.panels[1].pattern.height = 0.7;
    t.seams[0].allow_ease = false;
    assert!(t.compile().unwrap_err().contains("unequal lengths"));
    t.seams[0].allow_ease = true;
    let g = t.compile().unwrap();
    close(g.seams[0].measure(), 0.39, 1e-13);
}

#[test]
fn invalid_templates_fail_before_simulation() {
    let original = GarmentTemplate::two_panel_demo();
    let check = |t: GarmentTemplate| assert!(t.compile().is_err());
    let mut t = original.clone();
    t.version = 3;
    check(t);
    let mut t = original.clone();
    t.panels[1].id = t.panels[0].id.clone();
    check(t);
    let mut t = original.clone();
    t.panels[0].fabric = "missing".into();
    check(t);
    let mut t = original.clone();
    t.seams[0].b.panel = "missing".into();
    check(t);
    let mut t = original.clone();
    t.seams[0].stiffness = f64::NAN;
    check(t);
    for range in [
        [0.5, 0.5],
        [-0.1, 1.0],
        [0.0, 1.01],
        [0.8, 0.2],
        [f64::NAN, 1.0],
    ] {
        let mut t = original.clone();
        t.seams[0].a.range = range;
        check(t);
    }
    for notches in [
        vec![[0.0, 0.0]],
        vec![[0.8, 0.8], [0.7, 0.7]],
        vec![[0.5, 1.0]],
    ] {
        let mut t = original.clone();
        t.seams[0].notches = notches;
        check(t);
    }
    let mut t = original.clone();
    let mut seam = t.seams[0].clone();
    seam.id = "overlap".into();
    t.seams.push(seam);
    check(t);
    let mut t = original.clone();
    t.seams[0].b = t.seams[0].a.clone();
    check(t);
    let mut t = original.clone();
    t.panels[0].pattern.nx = 0;
    check(t);
    let mut t = original.clone();
    t.panels[0].placement.translation.x = f64::INFINITY;
    check(t);
    let mut json = serde_json::to_value(original).unwrap();
    json["future_field"] = true.into();
    assert!(serde_json::from_value::<GarmentTemplate>(json).is_err());
}

#[test]
fn adjacent_seam_ranges_and_shared_corner_merge_safely() {
    let mut t = parallel(2, 3);
    t.panels.truncate(1);
    t.panels[0].pattern.width = t.panels[0].pattern.height;
    t.seams[0].a.side = Side::Top;
    t.seams[0].b.panel = "left".into();
    t.seams[0].b.side = Side::Left;
    let g = t.compile().unwrap();
    let seam = &g.seams[0];
    assert!(seam.probes[0].terms.is_empty()); // shared top-left endpoint cancels
    for e in seam.elements(&g.initial_state.x) {
        let mut dofs = e.dofs.clone();
        dofs.sort();
        dofs.dedup();
        assert_eq!(dofs.len(), e.dofs.len());
    }
    let mut t = parallel(2, 3);
    t.seams[0].a.range = [0.0, 0.5];
    t.seams[0].b.range = [0.5, 1.0];
    let mut next = t.seams[0].clone();
    next.id = "second".into();
    next.a.range = [0.5, 1.0];
    next.b.range = [0.0, 0.5];
    t.seams.push(next);
    assert!(t.compile().is_ok());
}

#[test]
fn material_binding_mass_grain_and_placement_are_independent() {
    let mut t = parallel(2, 3);
    let props = FabricProperties {
        density: 0.73,
        warp: 234.0,
        ..Default::default()
    };
    t.fabrics.push(Fabric {
        id: "second".into(),
        name: "Second fabric".into(),
        properties: props,
    });
    t.panels[1].fabric = "second".into();
    t.panels[1].placement.rotation_degrees = DVec3::new(13.0, 24.0, 35.0);
    let g = t.compile().unwrap();
    for (i, panel) in g.panels.iter().enumerate() {
        close(
            g.mesh.masses[panel.vertices.clone()].iter().sum(),
            0.24 * g.materials[i].density,
            1e-13,
        );
        assert!(
            g.mesh.triangles[panel.triangles.clone()]
                .iter()
                .all(|tr| tr.material == i)
        );
        for tr in &g.mesh.triangles[panel.triangles.clone()] {
            close(
                energy::membrane(
                    tr.vertices.map(|v| g.initial_state.x[v].to_array()),
                    tr,
                    &g.materials[i],
                ),
                0.0,
                1e-20,
            );
        }
    }
    let frames = g.fiber_frames(&g.initial_state.x);
    let r = t.panels[1].placement.rotation_degrees * std::f64::consts::PI / 180.0;
    let rotated = DQuat::from_euler(EulerRot::XYZ, r.x, r.y, r.z)
        * DVec3::new(0.5_f64.sqrt(), -0.5_f64.sqrt(), 0.0);
    for f in &frames[g.panels[1].triangles.clone()] {
        assert!(f.valid);
        assert!(f.warp.distance(rotated) < 1e-13);
        close(f.stretch[0], 1.0, 1e-13);
        close(f.shear_cosine, 0.0, 1e-13);
    }
}

#[test]
fn template_roundtrip_and_exports_preserve_panel_identity() {
    let t = GarmentTemplate::two_panel_demo();
    let json = serde_json::to_string(&t).unwrap();
    let loaded: GarmentTemplate = serde_json::from_str(&json).unwrap();
    let a = t.compile().unwrap();
    let b = loaded.compile().unwrap();
    assert_eq!(a.initial_state.x, b.initial_state.x);
    assert_eq!(a.mesh.uv, b.mesh.uv);
    let obj = export::obj(&b, &b.initial_state);
    assert!(obj.contains("g left\n"));
    assert!(obj.contains("g right\n"));
    assert_eq!(
        obj.lines().filter(|l| l.starts_with("f ")).count(),
        b.mesh.triangles.len()
    );
    let mut t = t;
    t.name = "<untrusted & title>".into();
    let svg = export::pattern_svg(&t);
    assert!(svg.contains("&lt;untrusted &amp; title&gt;"));
}

#[test]
fn cpu_assembly_converges_and_preserves_pins() {
    let g = GarmentTemplate::two_panel_demo().compile().unwrap();
    let mut state = g.initial_state.clone();
    let initial = g.max_seam_gap(&state.x);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    pool.install(|| {
        for _ in 0..20 {
            let m = ReferenceSolver::step_garment(&g, &mut state, &SolverSettings::default(), None)
                .unwrap();
            assert!(
                m.converged,
                "residual={} tolerance={}",
                m.residual_rms_n, m.tolerance_n
            );
            assert!(m.objective_final_j <= m.objective_initial_j + 1e-10);
            assert!(m.seam_j.is_finite() && m.seam_samples > 0);
            close(m.seam_j, g.seams[0].energy(&state.x), 1e-12);
            for (i, pin) in g.mesh.pins.iter().enumerate() {
                if let Some(p) = pin {
                    assert_eq!(state.x[i], *p);
                }
            }
        }
    });
    assert!(g.max_seam_gap(&state.x) < initial * 0.05);
}

#[test]
fn corrupt_compiled_seam_returns_error_without_advancing_state() {
    let mut g = GarmentTemplate::two_panel_demo().compile().unwrap();
    let mut s = g.initial_state.clone();
    g.seams[0].samples[0].point.terms[0].0 = usize::MAX;
    assert!(ReferenceSolver::step_garment(&g, &mut s, &SolverSettings::default(), None).is_err());
    assert_eq!(s.x, g.initial_state.x);
    assert_eq!(s.time, 0.0);
}

#[test]
fn disconnected_panels_match_independent_solves_with_distinct_materials() {
    let mut t = parallel(1, 1);
    t.seams.clear();
    for p in &mut t.panels {
        p.pattern.nx = 1;
        p.pins.clear();
    }
    t.fabrics.push(Fabric {
        id: "heavy".into(),
        name: "Heavy".into(),
        properties: FabricProperties {
            warp: 800.0,
            density: 0.83,
            thickness: 0.003,
            ..Default::default()
        },
    });
    t.panels[1].fabric = "heavy".into();
    let g = t.compile().unwrap();
    let mut state = g.initial_state.clone();
    for (i, p) in state.x.iter_mut().enumerate() {
        p.x += 0.03 * g.mesh.uv[i].x;
    }
    let settings = SolverSettings {
        absolute_tolerance: 1e-9,
        relative_tolerance: 1e-8,
        ..Default::default()
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    pool.install(|| {
        let m = ReferenceSolver::step_garment(&g, &mut state, &settings, None).unwrap();
        assert!(m.converged);
        for i in 0..2 {
            let mut single = t.clone();
            single.panels = vec![t.panels[i].clone()];
            let one = single.compile().unwrap();
            let mut s = one.initial_state.clone();
            for (j, p) in s.x.iter_mut().enumerate() {
                p.x += 0.03 * one.mesh.uv[j].x;
            }
            let m = ReferenceSolver::step(&one.mesh, &mut s, &one.materials[0], &settings, None)
                .unwrap();
            assert!(m.converged);
            for (p, q) in state.x[g.panels[i].vertices.clone()].iter().zip(&s.x) {
                assert!(p.distance(*q) < 1e-8);
            }
        }
    });
}

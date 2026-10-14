use glam::{DQuat, DVec3};
use marvelous_cloth::{
    contact::Sphere,
    energy::{self, Element},
    model::{Material, Pattern, Scene},
    project::{self, Project},
    solver::{ReferenceSolver, SolverSettings},
};

fn fixture() -> (
    marvelous_cloth::model::Mesh,
    marvelous_cloth::model::State,
    Material,
) {
    let m = Material::default();
    let (mesh, state) = Pattern {
        nx: 2,
        ny: 2,
        ..Default::default()
    }
    .build(&m, Scene::Sphere)
    .unwrap();
    (mesh, state, m)
}
fn compare_derivatives(
    x: &[DVec3],
    element: impl Fn(&[DVec3]) -> Element,
    value: impl Fn(&[DVec3]) -> f64,
) {
    let e = element(x);
    let eps = 2e-6;
    for (local, &global) in e.dofs.iter().enumerate() {
        let mut a = x.to_vec();
        let mut b = x.to_vec();
        a[global / 3][global % 3] += eps;
        b[global / 3][global % 3] -= eps;
        let numerical = (value(&a) - value(&b)) / (2.0 * eps);
        assert!(
            (numerical - e.gradient[local]).abs() < 2e-6 * (1.0 + numerical.abs()),
            "gradient {local}: {numerical} vs {}",
            e.gradient[local]
        );
        let ga = element(&a);
        let gb = element(&b);
        for i in 0..e.dofs.len() {
            let numerical = (ga.gradient[i] - gb.gradient[i]) / (2.0 * eps);
            let exact = e.hessian[i * e.dofs.len() + local];
            assert!(
                (numerical - exact).abs() < 3e-5 * (1.0 + numerical.abs()),
                "Hessian ({i},{local}): {numerical} vs {exact}"
            );
            assert!((exact - e.hessian[local * e.dofs.len() + i]).abs() < 1e-9);
        }
    }
}

#[test]
fn membrane_gradient_hessian_tension_compression_and_rest() {
    let (mesh, s, m) = fixture();
    let t = &mesh.triangles[0];
    for scale in [0.87, 1.0, 1.14] {
        let mut x = s.x.clone();
        for p in &mut x {
            p.x *= scale;
        }
        if scale != 1.0 {
            x[t.vertices[2]].y += 0.013;
        }
        compare_derivatives(
            &x,
            |x| energy::membrane_element(x, t, &m),
            |x| energy::membrane(t.vertices.map(|i| x[i].to_array()), t, &m),
        );
    }
}

#[test]
fn tension_is_stiffer_than_compression_at_equal_green_strain() {
    let (mesh, s, m) = fixture();
    let t = &mesh.triangles[0];
    let energy_at = |strain: f64| {
        let p = t.vertices.map(|i| {
            let mut p = s.x[i];
            p.x *= (1.0 + 2.0 * strain).sqrt();
            p.to_array()
        });
        energy::membrane(p, t, &m)
    };
    assert!(energy_at(0.1) > 3.0 * energy_at(-0.1));
}

#[test]
fn initial_shallow_penetration_is_reported_and_reduced() {
    let (mesh, mut state, m) = fixture();
    let sp = Sphere::default();
    for p in &mut state.x {
        p.y = 0.34;
    }
    let before = state
        .x
        .iter()
        .map(|p| sp.penetration(*p, m.thickness))
        .fold(0.0, f64::max);
    let settings = SolverSettings {
        gravity: DVec3::ZERO,
        ..Default::default()
    };
    let metrics = ReferenceSolver::step(&mesh, &mut state, &m, &settings, Some(&sp)).unwrap();
    assert!(metrics.initial_penetrating_vertices > 0);
    assert!(metrics.max_vertex_penetration_m < before * 0.5);
}

#[test]
fn degenerate_geometry_is_rejected_without_changing_state() {
    let (mesh, mut state, m) = fixture();
    let [a, b, _] = mesh.triangles[0].vertices;
    state.x[b] = state.x[a];
    let before = state.x.clone();
    assert!(
        ReferenceSolver::step(&mesh, &mut state, &m, &SolverSettings::default(), None).is_err()
    );
    assert_eq!(state.x, before);
    assert_eq!(state.time, 0.0);
}

#[test]
fn bending_gradient_hessian() {
    let (mesh, mut s, m) = fixture();
    let h = &mesh.hinges[0];
    s.x[h.vertices[2]].y += 0.17;
    compare_derivatives(
        &s.x,
        |x| energy::bending_element(x, h, &m),
        |x| energy::bending(h.vertices.map(|i| x[i].to_array()), h, &m),
    );
}

#[test]
fn nearly_flat_bending_is_nonnegative_and_resolves_small_angles() {
    let (mesh, mut s, m) = fixture();
    let h = &mesh.hinges[0];
    for amplitude in [1e-4, 1e-7, 1e-10] {
        let rest = s.x[h.vertices[2]].y;
        s.x[h.vertices[2]].y = rest + amplitude;
        let e = energy::bending(h.vertices.map(|i| s.x[i].to_array()), h, &m);
        assert!(e > 0.0, "small bend lost to roundoff: {amplitude}");
        assert!(e.is_finite());
        s.x[h.vertices[2]].y = rest;
    }
}

#[test]
fn contact_gradient_hessian() {
    let sphere = Sphere::default();
    let x = [DVec3::new(0.12, 0.24, 0.08)];
    compare_derivatives(
        &x,
        |x| sphere.element(0, x[0], 0.001, 0.03).unwrap(),
        |x| sphere.energy(x[0].to_array(), 0.001, 0.03),
    );
}

#[test]
fn flat_rest_zero_energy_force_and_mass_conservation() {
    let (mesh, s, m) = fixture();
    assert!((mesh.masses.iter().sum::<f64>() - m.density).abs() < 1e-12);
    for e in mesh
        .triangles
        .iter()
        .map(|t| energy::membrane_element(&s.x, t, &m))
        .chain(
            mesh.hinges
                .iter()
                .map(|h| energy::bending_element(&s.x, h, &m)),
        )
    {
        assert!(e.energy.abs() < 1e-12);
        assert!(e.gradient.iter().all(|g| g.abs() < 1e-10));
    }
}

#[test]
fn rigid_motion_energy_invariance_and_internal_balance() {
    let (mesh, mut s, m) = fixture();
    s.x[4].y += 0.09;
    s.x[1].x += 0.04;
    let q = DQuat::from_rotation_y(0.71) * DQuat::from_rotation_z(-0.36);
    let moved: Vec<_> =
        s.x.iter()
            .map(|p| q * (*p) + DVec3::new(2.0, -1.3, 0.4))
            .collect();
    let original = mesh
        .triangles
        .iter()
        .map(|t| energy::membrane_element(&s.x, t, &m))
        .chain(
            mesh.hinges
                .iter()
                .map(|h| energy::bending_element(&s.x, h, &m)),
        );
    let transformed = mesh
        .triangles
        .iter()
        .map(|t| energy::membrane_element(&moved, t, &m))
        .chain(
            mesh.hinges
                .iter()
                .map(|h| energy::bending_element(&moved, h, &m)),
        );
    for (a, b) in original.zip(transformed) {
        assert!((a.energy - b.energy).abs() < 1e-10);
        let mut net = DVec3::ZERO;
        let mut torque = DVec3::ZERO;
        for i in (0..a.dofs.len()).step_by(3) {
            let f = DVec3::from_slice(&a.gradient[i..i + 3]);
            net += f;
            torque += s.x[a.dofs[i] / 3].cross(f);
            let rotated = q * f;
            let actual = DVec3::from_slice(&b.gradient[i..i + 3]);
            assert!(rotated.distance(actual) < 1e-9);
        }
        assert!(net.length() < 1e-9);
        assert!(torque.length() < 1e-9);
    }
}

#[test]
fn rotating_grain_90_swaps_warp_weft() {
    let (mesh, mut s, mut m) = fixture();
    s.x[1].x += 0.05;
    let t = &mesh.triangles[0];
    let p = t.vertices.map(|i| s.x[i].to_array());
    let e0 = energy::membrane(p, t, &m);
    m.grain_degrees = 90.0;
    std::mem::swap(&mut m.warp, &mut m.weft);
    assert!((energy::membrane(p, t, &m) - e0).abs() < 1e-10);
}

#[test]
fn free_fall_matches_implicit_euler_with_damping() {
    let (mut mesh, mut state, m) = fixture();
    mesh.pins.fill(None);
    let old = state.x.clone();
    let settings = SolverSettings::default();
    let metrics = ReferenceSolver::step(&mesh, &mut state, &m, &settings, None).unwrap();
    let expected = settings.gravity * settings.dt.powi(2) / (1.0 + settings.damping * settings.dt);
    assert!(metrics.converged);
    for (p, old) in state.x.iter().zip(old) {
        assert!((*p - old - expected).length() < 1e-8);
    }
}

#[test]
fn hanging_pins_fixed_and_objective_decreases() {
    let p = Project {
        scene: Scene::Hanging,
        sphere: None,
        pattern: Pattern {
            nx: 4,
            ny: 4,
            ..Default::default()
        },
        ..Default::default()
    };
    let (mesh, mut state) = p.build().unwrap();
    let center = state.x[12];
    for _ in 0..20 {
        let m = ReferenceSolver::step(&mesh, &mut state, &p.material, &p.solver, None).unwrap();
        assert!(m.objective_final_j <= m.objective_initial_j + 1e-12);
        assert!(
            m.converged,
            "residual {} > {}",
            m.residual_rms_n, m.tolerance_n
        );
        for (i, pin) in mesh.pins.iter().enumerate() {
            if let Some(pin) = pin {
                assert_eq!(state.x[i], *pin);
                assert_eq!(state.v[i], DVec3::ZERO);
            }
        }
    }
    assert!(state.x[12].y < center.y);
}

#[test]
fn sphere_drape_makes_contact_and_stays_finite() {
    let p = Project {
        pattern: Pattern {
            nx: 4,
            ny: 4,
            ..Default::default()
        },
        ..Default::default()
    };
    let (mesh, mut state) = p.build().unwrap();
    let mut contact = false;
    for _ in 0..35 {
        let m = ReferenceSolver::step(&mesh, &mut state, &p.material, &p.solver, p.sphere.as_ref())
            .unwrap();
        contact |= m.contacting_vertices > 0;
        assert!(state.x.iter().all(|p| p.is_finite()));
        assert!(
            m.max_vertex_penetration_m < 0.015,
            "vertex penetration {}",
            m.max_vertex_penetration_m
        );
        assert!(m.objective_final_j <= m.objective_initial_j + 1e-12);
    }
    assert!(contact);
}

#[test]
fn failed_step_is_transactional() {
    let (mesh, mut state, m) = fixture();
    let before = state.x.clone();
    let settings = SolverSettings {
        dt: f64::NAN,
        ..Default::default()
    };
    assert!(ReferenceSolver::step(&mesh, &mut state, &m, &settings, None).is_err());
    assert_eq!(state.x, before);
    assert_eq!(state.time, 0.0);
}

#[test]
fn iteration_limit_is_not_reported_as_convergence() {
    let p = Project {
        scene: Scene::Hanging,
        sphere: None,
        solver: SolverSettings {
            newton_iterations: 1,
            absolute_tolerance: 1e-12,
            relative_tolerance: 1e-12,
            ..Default::default()
        },
        ..Default::default()
    };
    let (mesh, mut state) = p.build().unwrap();
    let m = ReferenceSolver::step(&mesh, &mut state, &p.material, &p.solver, None).unwrap();
    assert!(!m.converged);
    assert!(m.residual_rms_n > m.tolerance_n);
}

#[test]
fn project_roundtrip_and_invalid_inputs() {
    let p = Project::default();
    let data = serde_json::to_string(&p).unwrap();
    let read: Project = serde_json::from_str(&data).unwrap();
    let (mesh, state) = read.build().unwrap();
    let obj = project::obj(&mesh, &state);
    assert_eq!(
        obj.lines().filter(|l| l.starts_with("vt ")).count(),
        mesh.uv.len()
    );
    assert_eq!(
        obj.lines().filter(|l| l.starts_with("f ")).count(),
        mesh.triangles.len()
    );
    let mut bad = p.clone();
    bad.pattern.nx = 0;
    assert!(bad.build().is_err());
    bad = p.clone();
    bad.material.density = -1.0;
    assert!(bad.build().is_err());
    bad = p;
    bad.version = 99;
    assert!(bad.build().is_err());
}

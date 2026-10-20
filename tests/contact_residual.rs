use glam::DVec3;
use marvelous_cloth::{
    ogc::{feature_hierarchy::Pair, topology::Topology},
    solver_lab::contact_residual::{Problem, merit},
};

#[test]
fn finite_motion_changes_force_basis_without_changing_endpoint_feasibility() {
    let old = vec![
        DVec3::new(-0.5, 0.1, 0.02),
        DVec3::new(0.5, 0.1, 0.02),
        DVec3::new(0., -0.4, 0.),
        DVec3::new(0., 0.6, 0.),
        DVec3::new(0.7, 0.7, 0.),
    ];
    let top = Topology::new(5, vec![[0, 1, 4], [2, 3, 4]]).unwrap();
    let edge = |a, b| top.edges.iter().position(|e| *e == [a, b]).unwrap();
    let pairs = [Pair::EdgeEdge([edge(0, 1), edge(2, 3)])];
    let p = Problem {
        topology: &top,
        pairs: &pairs,
        old: &old,
        cloth_vertices: old.len(),
        clearance: 0.001,
    };
    let mut new = old.clone();
    new[0] += DVec3::new(0.08, 0.03, 0.01);
    new[3].z -= 0.02;
    let mut multipliers = vec![0.; old.len() + 1];
    multipliers[old.len()] = 2.;
    let endpoint = p.evaluate(&new, &multipliers, false).unwrap();
    let discrete = p.evaluate(&new, &multipliers, true).unwrap();
    assert_eq!(endpoint.gaps, discrete.gaps);
    assert_eq!(endpoint.complementarity_j, discrete.complementarity_j);
    let difference: f64 = endpoint
        .force
        .iter()
        .zip(&discrete.force)
        .map(|(a, b)| (a - b).powi(2))
        .sum();
    assert!(difference > 1e-5);
    let work: f64 = new
        .iter()
        .zip(&old)
        .zip(discrete.force.chunks_exact(3))
        .map(|((a, b), f)| (*a - *b).dot(DVec3::from_slice(f)))
        .sum();
    assert!(
        (work - 2. * (pairs[0].distance(&top, &new) - pairs[0].distance(&top, &old))).abs() < 1e-12
    );
    let zero = p
        .evaluate(&new, &vec![0.; multipliers.len()], true)
        .unwrap();
    assert_eq!(zero.gaps, discrete.gaps);
    assert!(zero.force.iter().all(|f| *f == 0.));
    multipliers[0] = -1.;
    assert!(p.evaluate(&new, &multipliers, true).is_err());
}

#[test]
fn natural_merit_detects_each_complementarity_violation_and_force_imbalance() {
    let value = |force, gap, lambda| merit(&[force], &[gap], &[lambda], 2., 0.01).unwrap();
    assert_eq!(value(0., 0.1, 0.), 0.);
    assert_eq!(value(0., 0., 2.), 0.);
    assert!(value(0., -0.01, 0.) > 0.);
    assert!(value(0., 0.1, -2.) > 0.);
    assert!(value(0., 0.1, 2.) > 0.);
    assert_eq!(value(2., 0., 2.), 1.);
    assert!(merit(&[], &[0.], &[], 1., 1.).is_err());
    assert!(merit(&[f64::NAN], &[], &[], 1., 1.).is_err());
    assert!(merit(&[1.], &[], &[], 0., 1.).is_err());
    assert!(merit(&[f64::MAX], &[], &[], 1., 1.).is_err());
}

#[test]
fn fixed_surface_receives_no_dynamic_dof_but_retains_its_geometric_gap() {
    let old = vec![DVec3::new(0.2, 0.2, 0.01), DVec3::ZERO, DVec3::X, DVec3::Y];
    let top = Topology::new(4, vec![[1, 2, 3]]).unwrap();
    let pairs = [Pair::VertexFace([0, 0])];
    let p = Problem {
        topology: &top,
        pairs: &pairs,
        old: &old,
        cloth_vertices: 1,
        clearance: 0.001,
    };
    for discrete in [false, true] {
        let r = p.evaluate(&old, &[2., 3.], discrete).unwrap();
        assert_eq!(r.force.len(), 3);
        assert!((DVec3::from_slice(&r.force) - DVec3::new(0., 2., 3.)).length() < 1e-12);
        assert!((r.gaps[1] - 0.009).abs() < 1e-12);
    }
}

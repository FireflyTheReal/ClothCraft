use glam::DVec3;
use marvelous_cloth::{
    garment::{GarmentTemplate, template::Corner},
    ogc::{
        ContactSet, CpuContactKernel, OgcSettings, OgcWorld,
        filter::{CandidateFilter, PrimitiveCandidates, SpatialQuery},
    },
    vbd::{Grab, VbdSettings, VbdSolver},
};
use std::sync::Arc;

fn same(a: &ContactSet, b: &ContactSet) {
    let keys = |s: &ContactSet| {
        s.contacts
            .iter()
            .map(|c| (c.kind, c.vertices, c.count, c.mollifier_epsilon.to_bits()))
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(a), keys(b));
    assert_eq!(
        a.radii.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        b.radii.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
    );
    assert_eq!(a.minimum_distance.to_bits(), b.minimum_distance.to_bits());
    assert_eq!(a.incident, b.incident);
    assert_eq!(a.active_contacts, b.active_contacts);
    assert_eq!(a.vertex_face_candidates, b.vertex_face_candidates);
    assert_eq!(a.edge_edge_candidates, b.edge_edge_candidates);
}
/// Exercise the same primitive-candidate entry point used by GPU readback,
/// without using a device. Overinclusive search is intentional in this oracle.
struct Exhaustive;
impl CandidateFilter for Exhaustive {
    fn filter(&mut self, q: SpatialQuery<'_>) -> Result<Option<PrimitiveCandidates>, String> {
        let mut p = PrimitiveCandidates::default();
        for v in 0..q.x.len() {
            for (t, ids) in q.topology.triangles.iter().enumerate() {
                if v < q.moving || ids.iter().any(|&i| i < q.moving) {
                    p.vf.push([v, t]);
                }
            }
        }
        for e in 0..q.topology.edges.len() {
            for f in e + 1..q.topology.edges.len() {
                if q.topology.edges[e]
                    .iter()
                    .chain(&q.topology.edges[f])
                    .any(|&i| i < q.moving)
                {
                    p.ee.push([e, f]);
                }
            }
        }
        Ok(Some(p))
    }
}
#[test]
fn moving_cached_and_direct_contacts_match_scalar_with_identical_bounds() {
    rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap()
        .install(|| {
            let mut x = Vec::new();
            let mut triangles = Vec::new();
            for i in 0..10 {
                let b = x.len();
                let p = DVec3::new(
                    (i % 3) as f64 * 0.013,
                    (i / 3) as f64 * 0.017,
                    0.005 * i as f64,
                );
                x.extend([p, p + DVec3::X * 0.06, p + DVec3::Y * 0.06]);
                triangles.push([b, b + 1, b + 2]);
            }
            let original = x.clone();
            let mut scalar = OgcWorld::new(&x, triangles, 27, [[0, 3]]).unwrap();
            scalar.cpu_kernel = CpuContactKernel::Scalar;
            let mut vector = scalar.clone();
            vector.cpu_kernel = CpuContactKernel::Auto;
            let mut direct_a = scalar.clone();
            let mut direct_b = vector.clone();
            let settings = OgcSettings {
                radius: 0.01,
                ..Default::default()
            };
            for frame in 0..80 {
                for i in 0..27 {
                    x[i] = original[i]
                        + DVec3::new(
                            0.003 * (frame as f64 * 0.1).sin(),
                            0.,
                            0.00001 * (i % 3) as f64 * frame as f64,
                        );
                }
                let rq = if frame < 40 { 0.02 } else { 0.04 };
                let a = scalar.detect(&x, rq, &settings).unwrap();
                let b = vector.detect(&x, rq, &settings).unwrap();
                same(&a, &b);
                let a = direct_a
                    .detect_with_filter(&x, rq, &settings, &mut Exhaustive)
                    .unwrap();
                let b = direct_b
                    .detect_with_filter(&x, rq, &settings, &mut Exhaustive)
                    .unwrap();
                same(&a, &b);
                same(&b, &vector.detect(&x, rq, &settings).unwrap());
            }
        });
}

#[test]
fn unequal_sewing_pull_hold_release_has_identical_trajectory() {
    rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap()
        .install(|| {
            let mut t = GarmentTemplate::two_panel_demo();
            for (i, p) in t.panels.iter_mut().enumerate() {
                p.pattern.width = 0.15;
                p.pattern.height = if i == 0 { 0.30 } else { 0.345 };
                p.pattern.nx = 3;
                p.pattern.ny = 4;
                p.placement.rotation_degrees = DVec3::ZERO;
                p.placement.translation = if i == 0 {
                    DVec3::new(-0.15, 1., 0.)
                } else {
                    DVec3::new(0.004, 1., 0.006)
                };
                p.pins = vec![if i == 0 {
                    Corner::TopLeft
                } else {
                    Corner::TopRight
                }];
            }
            t.seams[0].allow_ease = true;
            t.seams[0].notches.clear();
            let g = Arc::new(t.compile().unwrap());
            let mut a = g.initial_state.clone();
            let mut b = a.clone();
            let mut sa = VbdSolver::new(g.clone(), None, &a).unwrap();
            sa.world.cpu_kernel = CpuContactKernel::Scalar;
            let mut sb = VbdSolver::new(g.clone(), None, &b).unwrap();
            sb.world.cpu_kernel = CpuContactKernel::Auto;
            let settings = VbdSettings {
                dt: 1. / 60.,
                iterations: 16,
                ..Default::default()
            };
            let vertices = g.mesh.triangles.last().unwrap().vertices;
            let point = |s: &marvelous_cloth::model::State| {
                vertices.iter().map(|&v| s.x[v] / 3.).sum::<DVec3>()
            };
            let mut anchor = DVec3::ZERO;
            let mut release = DVec3::ZERO;
            for step in 0..180 {
                if step == 40 {
                    anchor = point(&a);
                }
                if step == 100 {
                    release = point(&a);
                }
                let u = ((step as f64 - 39.) / 40.).clamp(0., 1.);
                let grab = (40..100).contains(&step).then_some(Grab {
                    patch: Vec::new(),
                    vertices,
                    weights: [1. / 3.; 3],
                    target: anchor + DVec3::Z * (0.04 * u * u * (3. - 2. * u)),
                    stiffness: 100.,
                });
                let ma = sa.step(&mut a, &settings, grab.as_ref()).unwrap();
                let mb = sb.step(&mut b, &settings, grab.as_ref()).unwrap();
                let bits = |xs: &[DVec3]| {
                    xs.iter()
                        .flat_map(|p| p.to_array().map(f64::to_bits))
                        .collect::<Vec<_>>()
                };
                assert_eq!(bits(&a.x), bits(&b.x), "positions at {step}");
                assert_eq!(bits(&a.v), bits(&b.v), "velocity at {step}");
                assert_eq!(a.time.to_bits(), b.time.to_bits());
                assert_eq!(ma.minimum_distance_m, mb.minimum_distance_m);
                assert_eq!(ma.contacts, mb.contacts);
                assert_eq!(ma.truncated_vertices, mb.truncated_vertices);
            }
            assert!(
                (point(&a) - release).length() > 1e-3,
                "cloth must move after releasing the shared target"
            );
        });
}

use marvelous_cloth::{
    math::{Scalar, cross, dot, sub},
    ogc::{
        feature_hierarchy::Pair,
        topology::Topology,
        two_sided_contact::{MinimumContact, Settings},
    },
};

/// Isolated point against a flat two-triangle patch. The reference target
/// geometry and all quadrature coefficients are fixed for this experiment.
pub fn energy<S: Scalar>(x: [[S; 3]; 5], cfg: Settings, measures: bool) -> S {
    let top = Topology::new(5, vec![[1, 2, 3], [2, 1, 4]]).unwrap();
    let model = MinimumContact::new(&top);
    let reference = [
        [0., 0., 0.],
        [-0.1, 0., -0.1],
        [0.1, 0., 0.1],
        [0.1, 0., -0.1],
        [-0.1, 0., 0.1],
    ];
    let scale_area = 0.02;
    let mut total = S::c(0.);
    for (f, t) in top.triangles.iter().enumerate() {
        let [a, b, c] = t.map(|i| reference[i]);
        let n = cross(sub(b, a), sub(c, a));
        let w = if measures {
            0.5 * dot(n, n).sqrt() / scale_area
        } else {
            1.
        };
        total = total + model.energy(Pair::VertexFace([0, f]), &x, w, cfg).unwrap();
    }
    for (e, ids) in top.edges.iter().enumerate() {
        let d = sub(reference[ids[0]], reference[ids[1]]);
        let w = if measures { dot(d, d) / scale_area } else { 1. };
        total = total + model.energy(Pair::VertexEdge([0, e]), &x, w, cfg).unwrap();
    }
    for v in 1..5 {
        let neighbors = &top.neighbors[v];
        let mean = neighbors
            .iter()
            .map(|i| {
                let d = sub(reference[v], reference[*i]);
                dot(d, d).sqrt()
            })
            .sum::<f64>()
            / neighbors.len() as f64;
        let w = if measures {
            mean * mean / scale_area
        } else {
            1.
        };
        total = total
            + model
                .energy(Pair::VertexVertex([0, v]), &x, w, cfg)
                .unwrap();
    }
    total
}

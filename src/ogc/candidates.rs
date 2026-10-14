//! Conservative temporal broad phase. Reuse only candidate IDs, NEVER distances,
//! feasible regions, contact forces or trust balls. If each moving vertex moves
//! at most s, a primitive-pair distance changes by at most 2s (convexity + triangle
//! inequality). An rq+2s search therefore covers the next rq searches.
use super::{
    bvh::{Bounds, Bvh},
    topology::Topology,
};
use glam::DVec3;
use rayon::prelude::*;

#[derive(Clone, Debug)]
pub(super) struct Candidates {
    centers: Vec<DVec3>,
    radius: f64,
    skin: f64,
    pub vf: Vec<[usize; 2]>,
    pub ee: Vec<[usize; 2]>,
}
impl Candidates {
    pub fn covers(&self, moving: &[DVec3], radius: f64) -> bool {
        radius <= self.radius
            && moving.len() == self.centers.len()
            && moving
                .iter()
                .zip(&self.centers)
                .all(|(a, b)| a.distance_squared(*b) <= self.skin * self.skin)
    }
    pub fn build(
        top: &Topology,
        trees: (&Bvh, &Bvh),
        x: &[DVec3],
        moving: usize,
        radius: f64,
        skin: f64,
        adjacent: impl Fn(usize, usize) -> bool + Sync,
    ) -> Self {
        let search = radius + 2. * skin + 1e-12;
        let bounds = Bounds::points(x[..moving].iter().copied()).inflate(search);
        let lanes = rayon::current_num_threads().min(2);
        let batches: Vec<_> = (0..lanes)
            .into_par_iter()
            .map(|lane| {
                let mut vf = Vec::new();
                let mut ee = Vec::new();
                for v in (lane..x.len()).step_by(lanes) {
                    if v >= moving && !bounds.overlaps(Bounds::points([x[v]])) {
                        continue;
                    }
                    trees
                        .0
                        .query(Bounds::points([x[v]]).inflate(search), v >= moving, |t| {
                            let ids = top.triangles[t];
                            if ids.iter().any(|i| adjacent(v, *i)) {
                                return;
                            }
                            // Candidate construction needs only a conservative
                            // lower bound. Exact VF/EE distances are recomputed
                            // by scan every step, including this first one.
                            if Bounds::points([x[v]])
                                .distance_squared(Bounds::points(ids.map(|i| x[i])))
                                <= search * search
                            {
                                vf.push([v, t]);
                            }
                        });
                }
                for e in (lane..top.edges.len()).step_by(lanes) {
                    let ids = top.edges[e];
                    if ids.iter().all(|v| *v >= moving) {
                        continue;
                    }
                    trees.1.query(
                        Bounds::points(ids.map(|i| x[i])).inflate(search),
                        false,
                        |f| {
                            let js = top.edges[f];
                            if f == e
                                || (f < e && js.iter().any(|i| *i < moving))
                                || ids.iter().any(|i| js.iter().any(|j| adjacent(*i, *j)))
                            {
                                return;
                            }
                            if Bounds::points(ids.map(|i| x[i]))
                                .distance_squared(Bounds::points(js.map(|i| x[i])))
                                <= search * search
                            {
                                ee.push([e, f]);
                            }
                        },
                    );
                }
                (vf, ee)
            })
            .collect();
        let mut vf = Vec::new();
        let mut ee = Vec::new();
        for (a, b) in batches {
            vf.extend(a);
            ee.extend(b);
        }
        Self {
            centers: x[..moving].to_vec(),
            radius,
            skin,
            vf,
            ee,
        }
    }
}

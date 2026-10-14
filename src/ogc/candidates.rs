//! Conservative temporal broad phase. Cache IDs only; exact distances and balls
//! are updated every step. Movement <= s keeps an rq+2s envelope conservative.
use super::{bvh::Bvh, topology::Topology};
use glam::DVec3;

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
        trees: (&Bvh, &Bvh, &Bvh),
        x: &[DVec3],
        moving: usize,
        radius: f64,
        skin: f64,
        adjacent: impl Fn(usize, usize) -> bool + Sync,
    ) -> Self {
        let search = radius + 2. * skin + 1e-12;
        let (vf, ee) = rayon::join(
            || {
                let mut vf = Vec::new();
                trees.0.pairs_with(trees.1, search, |v, t| {
                    if !top.triangles[t].iter().any(|i| adjacent(v, *i)) {
                        vf.push([v, t]);
                    }
                });
                vf
            },
            || {
                let mut ee = Vec::new();
                trees.2.self_pairs(search, |e, f| {
                    let ids = top.edges[e];
                    let js = top.edges[f];
                    if !ids.iter().any(|i| js.iter().any(|j| adjacent(*i, *j))) {
                        // Preserve legacy geometric operand order: moving edge
                        // first for fixed/moving pairs, canonical otherwise.
                        ee.push(if ids.iter().all(|i| *i >= moving) {
                            [f, e]
                        } else {
                            [e, f]
                        });
                    }
                });
                ee
            },
        );
        Self {
            centers: x[..moving].to_vec(),
            radius,
            skin,
            vf,
            ee,
        }
    }
}

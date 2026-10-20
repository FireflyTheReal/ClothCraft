//! Conservative temporal broad phase. Cache BVH leaf pairs, not expanded
//! primitive pairs. Movement <= s keeps an rq+2s envelope conservative.
use super::bvh::Bvh;
use glam::DVec3;

#[derive(Clone, Debug)]
pub(super) struct Candidates {
    centers: Vec<DVec3>,
    radius: f64,
    skin: f64,
    pub pair_count: usize,
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
        trees: (&Bvh, &Bvh, &Bvh),
        x: &[DVec3],
        moving: usize,
        radius: f64,
        skin: f64,
    ) -> Self {
        let search = radius + 2. * skin + 1e-12;
        let (vf, ee) = rayon::join(
            || trees.0.leaf_pairs(trees.1, search, false),
            || trees.2.leaf_pairs(trees.2, search, true),
        );
        let pair_count = vf
            .iter()
            .map(|[a, b]| trees.0.leaf_ids(*a).len() * trees.1.leaf_ids(*b).len())
            .sum::<usize>()
            + ee.iter()
                .map(|[a, b]| {
                    let n = trees.2.leaf_ids(*a).len();
                    if a == b {
                        n * (n - 1) / 2
                    } else {
                        n * trees.2.leaf_ids(*b).len()
                    }
                })
                .sum::<usize>();
        Self {
            pair_count,
            centers: x[..moving].to_vec(),
            radius,
            skin,
            vf,
            ee,
        }
    }
}

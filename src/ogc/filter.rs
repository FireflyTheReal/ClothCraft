//! Optional acceleration of candidate rejection. Geometry, feasible regions and
//! movement bounds remain in the f64 detector. Implementations must retain every
//! nonadjacent pair whose primitive distance is within the padded query radius.
use super::{bvh::Bvh, topology::Topology};
use glam::DVec3;

pub struct SpatialQuery<'a> {
    /// Identifies immutable topology, fixed geometry and local seam exclusions.
    pub id: u64,
    pub x: &'a [DVec3],
    pub topology: &'a Topology,
    pub triangles: &'a Bvh,
    pub edges: &'a Bvh,
    pub moving: usize,
    pub excluded_neighbors: &'a [Vec<usize>],
    pub padded_radius2: f64,
}

#[derive(Default)]
pub struct PrimitiveCandidates {
    pub vf: Vec<[usize; 2]>,
    pub ee: Vec<[usize; 2]>,
    pub retries: usize,
    pub timings: [f64; 3],
}

pub trait CandidateFilter {
    /// None keeps the complete CPU detector, for unsupported input/device limits.
    fn filter(&mut self, query: SpatialQuery<'_>) -> Result<Option<PrimitiveCandidates>, String>;
}

//! Sparse separating half-spaces for very close convex primitive pairs.
//! Every unconverted pair retains its original isotropic bound. The query
//! envelope still bounds all vertices, including pairs not returned by search.
use glam::DVec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HalfSpace {
    /// Forbidden direction, expressed against the last detection center.
    pub normal: DVec3,
    pub limit: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DirectionalBounds {
    offsets: Vec<usize>,
    planes: Vec<HalfSpace>,
}
impl DirectionalBounds {
    pub fn vertex_count(&self) -> usize {
        self.offsets.windows(2).filter(|r| r[0] != r[1]).count()
    }
    pub fn at(&self, vertex: usize) -> &[HalfSpace] {
        if self.offsets.is_empty() {
            return &[];
        }
        &self.planes[self.offsets[vertex]..self.offsets[vertex + 1]]
    }
    pub fn contains(&self, vertex: usize, delta: DVec3, tolerance: f64) -> bool {
        self.at(vertex)
            .iter()
            .all(|p| p.normal.dot(delta) <= p.limit + tolerance)
    }
    pub fn truncate(&self, vertex: usize, delta: DVec3) -> (DVec3, bool) {
        let mut alpha = 1.0_f64;
        for p in self.at(vertex) {
            let toward = p.normal.dot(delta);
            if toward > p.limit {
                alpha = alpha.min(p.limit / toward);
            }
        }
        (delta * alpha, alpha < 1.)
    }
    pub(super) fn from_entries(mut entries: Vec<(usize, HalfSpace)>, moving: usize) -> Self {
        if entries.is_empty() {
            return Self::default();
        }
        entries.sort_by(|(a, p), (b, q)| {
            a.cmp(b)
                .then(p.normal.x.total_cmp(&q.normal.x))
                .then(p.normal.y.total_cmp(&q.normal.y))
                .then(p.normal.z.total_cmp(&q.normal.z))
                .then(p.limit.total_cmp(&q.limit))
        });
        entries.dedup();
        let mut offsets = vec![0; moving + 1];
        for (v, _) in &entries {
            offsets[v + 1] += 1;
        }
        for v in 0..moving {
            offsets[v + 1] += offsets[v];
        }
        Self {
            offsets,
            planes: entries.into_iter().map(|(_, p)| p).collect(),
        }
    }
}

pub(super) struct Builder {
    pub distance: Vec<f64>,
    pub entries: Vec<(usize, HalfSpace)>,
    threshold: f64,
    relaxation: f64,
    moving: usize,
}
impl Builder {
    pub fn new(count: usize, moving: usize, query: f64, relaxation: f64) -> Self {
        Self {
            distance: vec![query; count],
            entries: Vec::new(),
            threshold: query / 64.,
            relaxation,
            moving,
        }
    }
    pub fn pair(&mut self, a: &[usize], b: &[usize], closest: [DVec3; 2], d: f64, x: &[DVec3]) {
        if d < self.threshold && self.separate(a, b, closest, d, x) {
            return;
        }
        for &v in a.iter().chain(b) {
            self.distance[v] = self.distance[v].min(d);
        }
    }
    fn separate(
        &mut self,
        a: &[usize],
        b: &[usize],
        [pa, pb]: [DVec3; 2],
        d: f64,
        x: &[DVec3],
    ) -> bool {
        if d <= 0. || !d.is_finite() {
            return false;
        }
        let normal = (pa - pb) / d;
        // Verify support separation on ALL primitive vertices, rather than
        // assuming the floating-point closest-point calculation is exact.
        let amin = a
            .iter()
            .map(|&v| normal.dot(x[v] - pb))
            .fold(f64::INFINITY, f64::min);
        let bmax = b
            .iter()
            .map(|&v| normal.dot(x[v] - pb))
            .fold(f64::NEG_INFINITY, f64::max);
        let scale = a
            .iter()
            .chain(b)
            .map(|&v| x[v].abs().max_element())
            .fold(1.0_f64, f64::max);
        let error = 128. * f64::EPSILON * scale;
        let gap = amin - bmax;
        if !gap.is_finite() || gap * self.relaxation <= 2. * error {
            return false;
        }
        let middle = bmax + 0.5 * gap;
        let gamma = 2. * self.relaxation;
        for (&v, sign) in a.iter().map(|v| (v, -1.)).chain(b.iter().map(|v| (v, 1.))) {
            if v >= self.moving {
                continue;
            }
            let slack = -sign * (normal.dot(x[v] - pb) - middle);
            self.entries.push((
                v,
                HalfSpace {
                    normal: normal * sign,
                    limit: gamma * slack - error,
                },
            ));
        }
        true
    }
}

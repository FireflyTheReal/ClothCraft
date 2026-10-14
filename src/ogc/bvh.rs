//! Refit BVH for both primitive families. No fixed query buffer: no silent drops.
use glam::DVec3;
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub min: DVec3,
    pub max: DVec3,
}
impl Bounds {
    pub fn points(p: impl IntoIterator<Item = DVec3>) -> Self {
        p.into_iter().fold(
            Self {
                min: DVec3::splat(f64::INFINITY),
                max: DVec3::splat(f64::NEG_INFINITY),
            },
            |b, p| Self {
                min: b.min.min(p),
                max: b.max.max(p),
            },
        )
    }
    pub fn inflate(self, r: f64) -> Self {
        Self {
            min: self.min - DVec3::splat(r),
            max: self.max + DVec3::splat(r),
        }
    }
    pub fn overlaps(self, b: Self) -> bool {
        self.min.cmple(b.max).all() && self.max.cmpge(b.min).all()
    }
    /// Conservative lower bound on the distance between enclosed primitives.
    pub fn distance_squared(self, b: Self) -> f64 {
        (self.min - b.max)
            .max(b.min - self.max)
            .max(DVec3::ZERO)
            .length_squared()
    }
    fn union(self, b: Self) -> Self {
        Self {
            min: self.min.min(b.min),
            max: self.max.max(b.max),
        }
    }
}
#[derive(Clone, Debug)]
struct Node {
    bounds: Bounds,
    children: Option<[usize; 2]>,
    start: usize,
    end: usize,
    dynamic: bool,
}
#[derive(Clone, Debug, Default)]
pub struct Bvh {
    nodes: Vec<Node>,
    order: Vec<usize>,
    roots: Vec<usize>,
    primitive_bounds: Vec<Bounds>,
    dynamic_nodes: usize,
}
impl Bvh {
    pub fn new(bounds: &[Bounds], dynamic: &[bool]) -> Self {
        let mut b = Self {
            nodes: Vec::new(),
            order: (0..bounds.len())
                .filter(|i| dynamic[*i])
                .chain((0..bounds.len()).filter(|i| !dynamic[*i]))
                .collect(),
            roots: Vec::new(),
            primitive_bounds: bounds.to_vec(),
            dynamic_nodes: 0,
        };
        // Separate roots: a draping cloth must not stretch mixed cloth/avatar
        // nodes across the entire body. Primitive IDs remain unchanged.
        let split = dynamic.iter().filter(|d| **d).count();
        for (start, end) in [(0, split), (split, bounds.len())] {
            if start < end {
                let root = b.build(bounds, dynamic, start, end);
                b.roots.push(root);
                if start == 0 && split > 0 {
                    b.dynamic_nodes = b.nodes.len();
                }
            }
        }
        b
    }
    fn build(&mut self, bounds: &[Bounds], dynamic: &[bool], start: usize, end: usize) -> usize {
        let box_ = self.order[start + 1..end]
            .iter()
            .fold(bounds[self.order[start]], |b, i| b.union(bounds[*i]));
        let id = self.nodes.len();
        self.nodes.push(Node {
            bounds: box_,
            children: None,
            start,
            end,
            dynamic: self.order[start..end].iter().any(|i| dynamic[*i]),
        });
        if end - start > 8 {
            let d = box_.max - box_.min;
            let axis = if d.x > d.y && d.x > d.z {
                0
            } else if d.y > d.z {
                1
            } else {
                2
            };
            self.order[start..end].sort_unstable_by(|a, b| {
                (bounds[*a].min[axis] + bounds[*a].max[axis])
                    .total_cmp(&(bounds[*b].min[axis] + bounds[*b].max[axis]))
            });
            let mid = (start + end) / 2;
            let a = self.build(bounds, dynamic, start, mid);
            let b = self.build(bounds, dynamic, mid, end);
            self.nodes[id].children = Some([a, b]);
        }
        id
    }
    pub fn refit(&mut self, bounds: &[Bounds]) {
        self.primitive_bounds.copy_from_slice(bounds);
        for i in (0..self.nodes.len()).rev() {
            self.nodes[i].bounds = if let Some([a, b]) = self.nodes[i].children {
                self.nodes[a].bounds.union(self.nodes[b].bounds)
            } else {
                self.order[self.nodes[i].start + 1..self.nodes[i].end]
                    .iter()
                    .fold(bounds[self.order[self.nodes[i].start]], |b, i| {
                        b.union(bounds[*i])
                    })
            };
        }
    }
    /// Refit only the moving root. The caller guarantees fixed geometry.
    pub fn refit_dynamic(&mut self, mut bounds: impl FnMut(usize) -> Bounds) {
        for i in (0..self.dynamic_nodes).rev() {
            self.nodes[i].bounds = if let Some([a, b]) = self.nodes[i].children {
                self.nodes[a].bounds.union(self.nodes[b].bounds)
            } else {
                let ids = &self.order[self.nodes[i].start..self.nodes[i].end];
                for id in ids {
                    self.primitive_bounds[*id] = bounds(*id);
                }
                ids[1..]
                    .iter()
                    .fold(self.primitive_bounds[ids[0]], |b, id| {
                        b.union(self.primitive_bounds[*id])
                    })
            };
        }
    }
    pub fn query(&self, bounds: Bounds, dynamic_only: bool, mut visit: impl FnMut(usize)) {
        if self.nodes.is_empty() {
            return;
        }
        // Median-balanced tree: depth is bounded by usize::BITS. This stack is
        // independent of candidate count and avoids one allocation per query.
        let mut stack = [0usize; 64];
        let mut len = self.roots.len();
        stack[..len].copy_from_slice(&self.roots);
        while len > 0 {
            len -= 1;
            let i = stack[len];
            let n = &self.nodes[i];
            if (dynamic_only && !n.dynamic) || !n.bounds.overlaps(bounds) {
                continue;
            }
            if let Some([a, b]) = n.children {
                stack[len] = a;
                stack[len + 1] = b;
                len += 2;
            } else {
                for id in &self.order[n.start..n.end] {
                    // Leaf boxes are broad; do not run exact feature geometry
                    // for other primitives in the same leaf.
                    if self.primitive_bounds[*id].overlaps(bounds) {
                        visit(*id);
                    }
                }
            }
        }
    }
}

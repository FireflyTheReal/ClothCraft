//! Read-only research candidate hierarchy. No force, bounds or solver changes.
//! Direct queries include lower features even when a parent is seam-excluded.
use super::{
    bvh::{Bounds, Bvh},
    feature_mollifier,
    primitive_distance::{point_segment, point_triangle, segment_segment},
    topology::Topology,
};
use glam::DVec3;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Pair {
    VertexFace([usize; 2]),
    EdgeEdge([usize; 2]),
    VertexEdge([usize; 2]),
    VertexVertex([usize; 2]),
}
impl Pair {
    pub fn vertices(self, top: &Topology) -> ([usize; 4], usize) {
        match self {
            Self::VertexFace([v, f]) => {
                let [a, b, c] = top.triangles[f];
                ([v, a, b, c], 4)
            }
            Self::EdgeEdge([e, f]) => {
                let [a, b] = top.edges[e];
                let [c, d] = top.edges[f];
                ([a, b, c, d], 4)
            }
            Self::VertexEdge([v, e]) => {
                let [a, b] = top.edges[e];
                ([v, a, b, 0], 3)
            }
            Self::VertexVertex([a, b]) => ([a, b, 0, 0], 2),
        }
    }
    pub fn distance(self, top: &Topology, x: &[DVec3]) -> f64 {
        match self {
            Self::VertexFace([v, f]) => {
                let [a, b, c] = top.triangles[f];
                point_triangle([x[v], x[a], x[b], x[c]].map(|p| p.to_array()))
            }
            Self::EdgeEdge([e, f]) => {
                let [a, b] = top.edges[e];
                let [c, d] = top.edges[f];
                segment_segment([x[a], x[b], x[c], x[d]].map(|p| p.to_array()))
            }
            Self::VertexEdge([v, e]) => {
                let [a, b] = top.edges[e];
                point_segment(x[v].to_array(), x[a].to_array(), x[b].to_array())
            }
            Self::VertexVertex([a, b]) => x[a].distance(x[b]),
        }
    }
    pub fn factor(self, top: &Topology, x: &[DVec3], width: f64) -> Result<f64, String> {
        let (ids, _) = self.vertices(top);
        let p = ids.map(|i| x[i].to_array());
        match self {
            Self::VertexFace(_) => feature_mollifier::vertex_face(p, width),
            Self::EdgeEdge(_) => feature_mollifier::edge_edge(p, width),
            Self::VertexEdge(_) => feature_mollifier::vertex_edge([p[0], p[1], p[2]], width),
            Self::VertexVertex(_) if self.distance(top, x) > 0. => Ok(1.),
            Self::VertexVertex(_) => Err("Coincident hierarchy vertices".into()),
        }
    }
    pub fn lower(self, top: &Topology) -> Vec<Self> {
        match self {
            Self::VertexFace([v, f]) => top.triangle_edges[f]
                .iter()
                .map(|e| Self::VertexEdge([v, *e]))
                .collect(),
            Self::EdgeEdge([e, f]) => top.edges[e]
                .iter()
                .map(|v| Self::VertexEdge([*v, f]))
                .chain(top.edges[f].iter().map(|v| Self::VertexEdge([*v, e])))
                .collect(),
            Self::VertexEdge([v, e]) => top.edges[e]
                .iter()
                .map(|a| Self::VertexVertex([v.min(*a), v.max(*a)]))
                .collect(),
            Self::VertexVertex(_) => Vec::new(),
        }
    }
    pub fn kind_index(self) -> usize {
        match self {
            Self::VertexFace(_) => 0,
            Self::EdgeEdge(_) => 1,
            Self::VertexEdge(_) => 2,
            Self::VertexVertex(_) => 3,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct Hierarchy {
    pub radius_m: f64,
    pub pairs: Vec<Pair>,
    pub duplicate_visits: usize,
    #[serde(skip)]
    snapshot: Vec<DVec3>,
    #[serde(skip)]
    edges: Vec<[usize; 2]>,
    #[serde(skip)]
    triangles: Vec<[usize; 3]>,
    #[serde(skip)]
    triangle_edges: Vec<[usize; 3]>,
    #[serde(skip)]
    moving: usize,
}
#[derive(Debug, Serialize)]
pub struct Coverage {
    pub active_by_kind: [usize; 4],
    pub zero_weight_by_kind: [usize; 4],
    pub missing_active_lower_pairs: Vec<Pair>,
    pub uncovered_zero_weight_pairs: Vec<Pair>,
    pub factor_errors: Vec<(Pair, String)>,
    pub static_lower_witnesses: usize,
}

impl Hierarchy {
    /// Snapshot query, not a temporal cache. A caller planning movement must
    /// provide a proven query envelope and refresh before leaving it.
    pub fn build(
        top: &Topology,
        x: &[DVec3],
        moving: usize,
        excluded: &BTreeSet<[usize; 2]>,
        radius: f64,
    ) -> Result<Self, String> {
        if x.len() != top.neighbors.len()
            || moving > x.len()
            || !radius.is_finite()
            || radius <= 0.
            || x.iter().any(|p| !p.is_finite())
            || excluded.iter().any(|p| p[0] > p[1] || p[1] >= x.len())
        {
            return Err("Invalid feature hierarchy input".into());
        }
        for t in &top.triangles {
            let [a, b, c] = t.map(|i| x[i]);
            let area_squared = (b - a).cross(c - a).length_squared();
            if !area_squared.is_finite() || area_squared <= 1e-30 {
                return Err("Degenerate hierarchy triangle".into());
            }
        }
        let allowed = |a: &[usize], b: &[usize]| {
            a.iter().any(|i| *i < moving) || b.iter().any(|i| *i < moving)
        };
        let disjoint = |a: &[usize], b: &[usize]| {
            a.iter().all(|i| {
                b.iter()
                    .all(|j| i != j && !excluded.contains(&[(*i).min(*j), (*i).max(*j)]))
            })
        };
        let points = Bvh::new(
            &x.iter().map(|p| Bounds::points([*p])).collect::<Vec<_>>(),
            &(0..x.len()).map(|i| i < moving).collect::<Vec<_>>(),
        );
        let edges = Bvh::new(
            &top.edges
                .iter()
                .map(|e| Bounds::points(e.map(|i| x[i])))
                .collect::<Vec<_>>(),
            &top.edges
                .iter()
                .map(|e| e.iter().any(|i| *i < moving))
                .collect::<Vec<_>>(),
        );
        let faces = Bvh::new(
            &top.triangles
                .iter()
                .map(|t| Bounds::points(t.map(|i| x[i])))
                .collect::<Vec<_>>(),
            &top.triangles
                .iter()
                .map(|t| t.iter().any(|i| *i < moving))
                .collect::<Vec<_>>(),
        );
        // Padding is broadphase-only. Exact finite distances retain the
        // caller's radius; no force shell or geometric guard is changed.
        let scale = x
            .iter()
            .map(|p| p.abs().max_element())
            .fold(radius.max(1.), f64::max);
        let query = radius + 64. * f64::EPSILON * scale;
        let mut pairs = Vec::new();
        let mut add = |pair: Pair, a: &[usize], b: &[usize]| {
            if allowed(a, b) && disjoint(a, b) && pair.distance(top, x) <= radius {
                pairs.push(pair);
            }
        };
        points.pairs_with(&faces, query, |v, f| {
            add(Pair::VertexFace([v, f]), &[v], &top.triangles[f])
        });
        edges.self_pairs(query, |e, f| {
            add(Pair::EdgeEdge([e, f]), &top.edges[e], &top.edges[f])
        });
        points.pairs_with(&edges, query, |v, e| {
            add(Pair::VertexEdge([v, e]), &[v], &top.edges[e])
        });
        points.self_pairs(query, |a, b| add(Pair::VertexVertex([a, b]), &[a], &[b]));
        pairs.sort_unstable();
        let before = pairs.len();
        pairs.dedup();
        Ok(Self {
            radius_m: radius,
            duplicate_visits: before - pairs.len(),
            pairs,
            snapshot: x.to_vec(),
            edges: top.edges.clone(),
            triangles: top.triangles.clone(),
            triangle_edges: top.triangle_edges.clone(),
            moving,
        })
    }

    /// Check geometric feature handoff only. Directional factors, quadrature,
    /// force magnitude, and collision-free trajectories are outside this audit.
    pub fn coverage(
        &self,
        top: &Topology,
        x: &[DVec3],
        activation_radius: f64,
        width: f64,
    ) -> Result<Coverage, String> {
        if x != self.snapshot
            || top.edges != self.edges
            || top.triangles != self.triangles
            || top.triangle_edges != self.triangle_edges
        {
            return Err(
                "Feature hierarchy coverage requires its original geometry and topology".into(),
            );
        }
        if !activation_radius.is_finite()
            || activation_radius <= 0.
            || activation_radius > self.radius_m
            || !width.is_finite()
            || width <= 0.
        {
            return Err("Coverage radius must lie inside the built hierarchy".into());
        }
        let mut result = Coverage {
            active_by_kind: [0; 4],
            zero_weight_by_kind: [0; 4],
            missing_active_lower_pairs: Vec::new(),
            uncovered_zero_weight_pairs: Vec::new(),
            factor_errors: Vec::new(),
            static_lower_witnesses: 0,
        };
        for &pair in &self.pairs {
            if pair.distance(top, x) >= activation_radius {
                continue;
            }
            result.active_by_kind[pair.kind_index()] += 1;
            let mut stack = pair.lower(top);
            let mut covered = false;
            while let Some(child) = stack.pop() {
                if child.distance(top, x) >= activation_radius {
                    continue;
                }
                let (ids, count) = child.vertices(top);
                let is_static = ids[..count].iter().all(|i| *i >= self.moving);
                if is_static {
                    // A mixed fixed/moving parent can vanish onto a completely
                    // fixed boundary. No unknown participates in that child.
                    if child.factor(top, x, width).is_ok_and(|m| m > 0.) {
                        covered = true;
                        result.static_lower_witnesses += 1;
                    }
                } else if self.pairs.binary_search(&child).is_err() {
                    result.missing_active_lower_pairs.push(child);
                } else if child.factor(top, x, width).is_ok_and(|m| m > 0.) {
                    covered = true;
                }
                stack.extend(child.lower(top));
            }
            match pair.factor(top, x, width) {
                Err(e) => result.factor_errors.push((pair, e)),
                Ok(0.) => {
                    result.zero_weight_by_kind[pair.kind_index()] += 1;
                    if !covered {
                        result.uncovered_zero_weight_pairs.push(pair);
                    }
                }
                Ok(_) => {}
            }
        }
        result.missing_active_lower_pairs.sort_unstable();
        result.missing_active_lower_pairs.dedup();
        Ok(result)
    }
}

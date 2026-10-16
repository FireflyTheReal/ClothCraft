use super::{
    bvh::{Bounds, Bvh},
    candidates::Candidates,
    geometry::{self, Feature},
    potential::{Contact, Kind},
    topology::Topology,
};
use glam::DVec3;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::time::Instant;

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct DetectionProfile {
    pub refit_ms: f64,
    pub candidates_ms: f64,
    pub narrow_ms: f64,
    pub candidate_pairs: usize,
    /// Cached pairs rejected by current AABB distance before exact geometry.
    pub aabb_rejections: usize,
}
impl std::ops::AddAssign for DetectionProfile {
    fn add_assign(&mut self, rhs: Self) {
        self.refit_ms += rhs.refit_ms;
        self.candidates_ms += rhs.candidates_ms;
        self.narrow_ms += rhs.narrow_ms;
        self.candidate_pairs += rhs.candidate_pairs;
        self.aabb_rejections += rhs.aabb_rejections;
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OgcSettings {
    /// Offset radius in metres. This is a contact shell, not material thickness.
    pub radius: f64,
    /// Per-feature-pair spring stiffness N/m (paper kc).
    pub stiffness: f64,
    /// Eq.21 requires strictly less than 1/2.
    pub relaxation: f64,
    /// Fraction of moving vertices reaching their ball before refreshing it.
    pub refresh_fraction: f64,
}
impl Default for OgcSettings {
    fn default() -> Self {
        Self {
            radius: 0.003,
            stiffness: 100_000.0,
            relaxation: 0.45,
            refresh_fraction: 0.01,
        }
    }
}
impl OgcSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !self.radius.is_finite()
            || self.radius <= 0.0
            || !self.stiffness.is_finite()
            || self.stiffness <= 0.0
            || !self.relaxation.is_finite()
            || self.relaxation <= 0.0
            || self.relaxation >= 0.5
            || !self.refresh_fraction.is_finite()
            || !(0.0..=1.0).contains(&self.refresh_fraction)
        {
            return Err("Invalid OGC radius, stiffness or trust-region parameters".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ContactSet {
    pub profile: DetectionProfile,
    /// Nearby terms retained until the next detection, including zero energy.
    pub contacts: Vec<Contact>,
    /// Terms with positive energy at the detection configuration.
    pub active_contacts: usize,
    pub incident: Vec<Vec<usize>>,
    pub centers: Vec<DVec3>,
    pub radii: Vec<f64>,
    pub minimum_distance: f64,
    pub vertex_face_candidates: usize,
    pub edge_edge_candidates: usize,
    pub broad_phase_rebuilt: bool,
}
impl ContactSet {
    /// Equation 27, relative to the LAST DETECTION, never reset per local solve.
    /// Used inside the variational local step; the candidate is energy-checked.
    pub fn truncate(&self, v: usize, proposed: DVec3) -> (DVec3, bool) {
        let d = proposed - self.centers[v];
        let len = d.length();
        let b = self.radii[v];
        if len > b {
            (self.centers[v] + d * (b / len), true)
        } else {
            (proposed, false)
        }
    }
    pub fn value(&self, x: &[DVec3], settings: &OgcSettings) -> f64 {
        self.contacts
            .iter()
            .map(|c| c.value(x, settings.radius, settings.stiffness))
            .sum()
    }
}

/// Combined moving cloth + fixed avatar topology. Bounds always include BOTH
/// VF directions AND EE, even when a feasible-region/mollifier rejects a force.
#[derive(Clone, Debug)]
pub struct OgcWorld {
    pub topology: Topology,
    pub moving: usize,
    pub fixed: Vec<DVec3>,
    triangles: Bvh,
    edges: Bvh,
    points: Bvh,
    rest_edge_length2: Vec<f64>,
    excluded: BTreeSet<[usize; 2]>,
    excluded_neighbors: Vec<Vec<usize>>,
    moving_faces: Vec<usize>,
    candidates: Option<Candidates>,
}
impl OgcWorld {
    pub fn new(
        x: &[DVec3],
        triangles: Vec<[usize; 3]>,
        moving: usize,
        excluded: impl IntoIterator<Item = [usize; 2]>,
    ) -> Result<Self, String> {
        if moving > x.len() || x.iter().any(|v| !v.is_finite()) {
            return Err("Invalid OGC positions".into());
        }
        let topology = Topology::new(x.len(), triangles)?;
        for t in &topology.triangles {
            let [a, b, c] = t.map(|i| x[i]);
            if (b - a).cross(c - a).length_squared() < 1e-24 {
                return Err("OGC degenerate initial face".into());
            }
        }
        let moving_faces = topology
            .triangles
            .iter()
            .enumerate()
            .filter_map(|(i, t)| t.iter().any(|v| *v < moving).then_some(i))
            .collect();
        let tb: Vec<_> = topology
            .triangles
            .iter()
            .map(|t| Bounds::points(t.map(|i| x[i])))
            .collect();
        let eb: Vec<_> = topology
            .edges
            .iter()
            .map(|e| Bounds::points(e.map(|i| x[i])))
            .collect();
        let triangles = Bvh::new(
            &tb,
            &topology
                .triangles
                .iter()
                .map(|t| t.iter().any(|i| *i < moving))
                .collect::<Vec<_>>(),
        );
        let edges = Bvh::new(
            &eb,
            &topology
                .edges
                .iter()
                .map(|t| t.iter().any(|i| *i < moving))
                .collect::<Vec<_>>(),
        );
        let rest_edge_length2 = topology
            .edges
            .iter()
            .map(|e| x[e[0]].distance_squared(x[e[1]]))
            .collect();
        let points = Bvh::new(
            &x.iter().map(|p| Bounds::points([*p])).collect::<Vec<_>>(),
            &(0..x.len()).map(|i| i < moving).collect::<Vec<_>>(),
        );
        let excluded: BTreeSet<_> = excluded
            .into_iter()
            .map(|[a, b]| [a.min(b), a.max(b)])
            .collect();
        let mut excluded_neighbors = vec![Vec::new(); x.len()];
        for [a, b] in &excluded {
            if *b >= x.len() {
                return Err("OGC invalid excluded vertex".into());
            }
            excluded_neighbors[*a].push(*b);
            excluded_neighbors[*b].push(*a);
        }
        for adjacent in &mut excluded_neighbors {
            adjacent.sort_unstable();
        }
        Ok(Self {
            topology,
            moving,
            fixed: x[moving..].to_vec(),
            triangles,
            edges,
            points,
            rest_edge_length2,
            excluded,
            excluded_neighbors,
            moving_faces,
            candidates: None,
        })
    }
    pub fn positions(&self, moving: &[DVec3]) -> Vec<DVec3> {
        moving.iter().chain(&self.fixed).copied().collect()
    }
    fn adjacent(&self, a: usize, b: usize) -> bool {
        a == b || self.excluded_neighbors[a].binary_search(&b).is_ok()
    }
    fn refit(&mut self, x: &[DVec3]) -> Result<(), String> {
        if x.len() != self.topology.neighbors.len() || x.iter().any(|v| !v.is_finite()) {
            return Err("OGC invalid position array".into());
        }
        if x[self.moving..] != self.fixed {
            return Err("OGC fixed geometry changed; rebuild the world".into());
        }
        for t in self
            .moving_faces
            .iter()
            .map(|i| &self.topology.triangles[*i])
        {
            let [a, b, c] = t.map(|i| x[i]);
            if (b - a).cross(c - a).length_squared() < 1e-24 {
                return Err("OGC degenerate face; step rejected".into());
            }
        }
        self.triangles
            .refit_dynamic(|i| Bounds::points(self.topology.triangles[i].map(|v| x[v])));
        self.edges
            .refit_dynamic(|i| Bounds::points(self.topology.edges[i].map(|v| x[v])));
        self.points.refit_dynamic(|i| Bounds::points([x[i]]));
        Ok(())
    }
    pub fn validate_start(&mut self, x: &[DVec3]) -> Result<(), String> {
        self.refit(x)?;
        for (i, t) in self.topology.triangles.iter().enumerate() {
            let mut hit = None;
            self.triangles.query(
                Bounds::points(t.map(|i| x[i])).inflate(1e-10),
                t.iter().all(|v| *v >= self.moving),
                |j| {
                    let q = self.topology.triangles[j];
                    if j <= i
                        || (t.iter().all(|v| *v >= self.moving)
                            && q.iter().all(|v| *v >= self.moving))
                        || t.iter().any(|a| {
                            q.iter()
                                .any(|b| self.excluded.contains(&[(*a).min(*b), (*a).max(*b)]))
                        })
                    {
                        return;
                    }
                    for k in 0..3 {
                        // A non-coplanar segment incident to a known shared
                        // vertex meets this plane ONLY at that endpoint. Testing
                        // it numerically can invent an interior hit at t≈1 when
                        // the determinant is tiny. Opposite edges are still
                        // checked; coplanar overlaps are caught by VF/EE distance.
                        if (!q.contains(&t[k])
                            && !q.contains(&t[(k + 1) % 3])
                            && geometry::segment_triangle(
                                x[t[k]],
                                x[t[(k + 1) % 3]],
                                q.map(|v| x[v]),
                            ))
                            || (!t.contains(&q[k])
                                && !t.contains(&q[(k + 1) % 3])
                                && geometry::segment_triangle(
                                    x[q[k]],
                                    x[q[(k + 1) % 3]],
                                    t.map(|v| x[v]),
                                ))
                        {
                            hit = Some(j);
                        }
                    }
                },
            );
            if let Some(j) = hit {
                return Err(format!(
                    "OGC initial intersection between faces {i} and {j}; reset/rearrange before simulating"
                ));
            }
        }
        let s = self.detect(
            x,
            1e-5,
            &OgcSettings {
                radius: 1e-5,
                ..Default::default()
            },
        )?;
        if s.minimum_distance <= 1e-10 {
            return Err(
                "OGC initial touching/coplanar features: separation must be positive".into(),
            );
        }
        Ok(())
    }
    pub fn detect(
        &mut self,
        x: &[DVec3],
        query_radius: f64,
        settings: &OgcSettings,
    ) -> Result<ContactSet, String> {
        self.detect_impl::<true>(x, query_radius, settings)
    }
    fn detect_impl<const PRUNE: bool>(
        &mut self,
        x: &[DVec3],
        query_radius: f64,
        settings: &OgcSettings,
    ) -> Result<ContactSet, String> {
        settings.validate()?;
        if !query_radius.is_finite() || query_radius < settings.radius {
            return Err("OGC query radius must cover the contact radius".into());
        }
        let start = Instant::now();
        self.refit(x)?;
        let refit_ms = start.elapsed().as_secs_f64() * 1000.;
        let broad_start = Instant::now();
        // Independent narrow-phase lanes: deterministic min reduction and
        // canonical contact union, no atomics or discarded candidates.
        let lanes = rayon::current_num_threads().min(2);
        let broad_phase_rebuilt = self
            .candidates
            .as_ref()
            .is_none_or(|c| !c.covers(&x[..self.moving], query_radius));
        if broad_phase_rebuilt {
            self.candidates = Some(Candidates::build(
                &self.topology,
                (&self.points, &self.triangles, &self.edges),
                x,
                self.moving,
                // Quantize upward so tiny changes of inertial speed do not
                // invalidate an otherwise conservative candidate envelope.
                ((query_radius / settings.radius).ceil() * settings.radius).max(query_radius),
                settings.radius,
                |a, b| self.adjacent(a, b),
            ));
        }
        let candidates_ms = broad_start.elapsed().as_secs_f64() * 1000.;
        let narrow_start = Instant::now();
        // Keep a roundoff band at the threshold, including translated scenes.
        // This only admits extra exact queries; it never shrinks the search.
        let scale = x
            .iter()
            .map(|p| p.abs().max_element())
            .fold(query_radius, f64::max);
        let padded_radius2 = (query_radius + 64. * f64::EPSILON * scale).powi(2);
        let batches: Vec<_> = (0..lanes)
            .into_par_iter()
            .map(|lane| self.scan::<PRUNE>(x, query_radius, padded_radius2, lane, lanes))
            .collect();
        let mut distance = vec![query_radius; x.len()];
        let mut found = Vec::new();
        let mut vf_count = 0;
        let mut ee_count = 0;
        let mut aabb_rejections = 0;
        for (distances, contacts, vf, ee, rejected) in batches {
            for (d, v) in distance.iter_mut().zip(distances) {
                *d = d.min(v);
            }
            found.extend(contacts);
            vf_count += vf;
            ee_count += ee;
            aabb_rejections += rejected;
        }
        // Sort/merge once instead of allocating a tree node for every retained
        // nearby term. Canonical order remains the same for colors/reductions.
        found.sort_unstable_by_key(|c: &Contact| (c.kind, c.vertices));
        found.dedup_by_key(|c| (c.kind, c.vertices));
        let contacts = found;
        let active_contacts = contacts
            .iter()
            .filter(|c| c.value(x, settings.radius, settings.stiffness) > 0.)
            .count();
        let mut incident = vec![Vec::new(); self.moving];
        for (c, contact) in contacts.iter().enumerate() {
            for v in &contact.vertices[..contact.count] {
                if *v < self.moving {
                    incident[*v].push(c);
                }
            }
        }
        let minimum_distance = distance[..self.moving]
            .iter()
            .copied()
            .fold(query_radius, f64::min);
        Ok(ContactSet {
            profile: DetectionProfile {
                refit_ms,
                candidates_ms,
                narrow_ms: narrow_start.elapsed().as_secs_f64() * 1000.,
                candidate_pairs: self.candidates.as_ref().unwrap().vf.len()
                    + self.candidates.as_ref().unwrap().ee.len(),
                aabb_rejections,
            },
            contacts,
            active_contacts,
            incident,
            centers: x[..self.moving].to_vec(),
            radii: distance[..self.moving]
                .iter()
                .map(|d| settings.relaxation * d)
                .collect(),
            minimum_distance,
            vertex_face_candidates: vf_count,
            edge_edge_candidates: ee_count,
            broad_phase_rebuilt,
        })
    }
    fn scan<const PRUNE: bool>(
        &self,
        x: &[DVec3],
        query_radius: f64,
        padded_radius2: f64,
        lane: usize,
        lanes: usize,
    ) -> DetectionBatch {
        let top = &self.topology;
        let mut distance = vec![query_radius; x.len()];
        let mut found = Vec::new();
        let mut vf_count = 0;
        let mut ee_count = 0;
        let mut rejected = 0;
        let mut add = |kind: Kind, mut ids: [usize; 4], count: usize, epsilon: f64| {
            if kind == Kind::PointPoint && ids[0] > ids[1] {
                ids.swap(0, 1);
            }
            if kind == Kind::PointEdge && ids[1] > ids[2] {
                ids.swap(1, 2);
            }
            if kind == Kind::PointFace {
                ids[1..].sort_unstable();
            }
            if kind == Kind::EdgeEdge {
                ids[..2].sort_unstable();
                ids[2..].sort_unstable();
                if ids[0] > ids[2] {
                    ids.swap(0, 2);
                    ids.swap(1, 3);
                }
            }
            found.push(Contact {
                kind,
                vertices: ids,
                count,
                mollifier_epsilon: epsilon,
            });
        };
        let candidates = self.candidates.as_ref().unwrap();
        for pair in candidates.vf.iter().skip(lane).step_by(lanes) {
            let [v, t] = *pair;
            if PRUNE
                && self
                    .points
                    .primitive_bounds(v)
                    .distance_squared(self.triangles.primitive_bounds(t))
                    > padded_radius2
            {
                rejected += 1;
                continue;
            }
            let ids = top.triangles[t];
            let (p, f) = geometry::closest_triangle(x[v], t, x, top);
            let d = x[v].distance(p);
            if d >= query_radius {
                continue;
            }
            vf_count += 1;
            distance[v] = distance[v].min(d);
            for i in ids {
                distance[i] = distance[i].min(d);
            }
            // Keep nearby zero-energy terms too: a local iterate can cross r
            // before the next detection. The potential itself activates at r.
            match f {
                Feature::Vertex(i) if geometry::vertex_feasible(x[v], i, x, top) => {
                    add(Kind::PointPoint, [v, i, 0, 0], 2, 0.0)
                }
                Feature::Edge(e) if geometry::edge_feasible(x[v], e, x, top) => {
                    let [a, b] = top.edges[e];
                    add(Kind::PointEdge, [v, a, b, 0], 3, 0.0)
                }
                Feature::Face(_) => add(Kind::PointFace, [v, ids[0], ids[1], ids[2]], 4, 0.0),
                _ => {}
            }
        }
        for pair in candidates.ee.iter().skip(lane).step_by(lanes) {
            let [e, f] = *pair;
            if PRUNE
                && self
                    .edges
                    .primitive_bounds(e)
                    .distance_squared(self.edges.primitive_bounds(f))
                    > padded_radius2
            {
                rejected += 1;
                continue;
            }
            let ids = &top.edges[e];
            let js = top.edges[f];
            let [a, b] = ids.map(|v| x[v]);
            let [c, d] = js.map(|v| x[v]);
            let (s, t) = geometry::closest_segments(a, b, c, d);
            let p = a + (b - a) * s;
            let q = c + (d - c) * t;
            let dist = p.distance(q);
            if dist >= query_radius {
                continue;
            }
            ee_count += 1;
            for v in ids.iter().chain(&js) {
                distance[*v] = distance[*v].min(dist);
            }
            let end_a = if s <= 0.0 {
                Some(ids[0])
            } else if s >= 1.0 {
                Some(ids[1])
            } else {
                None
            };
            let end_b = if t <= 0.0 {
                Some(js[0])
            } else if t >= 1.0 {
                Some(js[1])
            } else {
                None
            };
            if end_a.is_some_and(|i| !geometry::vertex_feasible(q, i, x, top))
                || end_b.is_some_and(|j| !geometry::vertex_feasible(p, j, x, top))
            {
                continue;
            }
            match (end_a, end_b) {
                (Some(i), Some(j)) => add(Kind::PointPoint, [i, j, 0, 0], 2, 0.0),
                (Some(i), None) => add(Kind::PointEdge, [i, js[0], js[1], 0], 3, 0.0),
                (None, Some(j)) => add(Kind::PointEdge, [j, ids[0], ids[1], 0], 3, 0.0),
                (None, None) => add(
                    Kind::EdgeEdge,
                    [ids[0], ids[1], js[0], js[1]],
                    4,
                    1e-3 * self.rest_edge_length2[e] * self.rest_edge_length2[f],
                ),
            }
        }
        (distance, found, vf_count, ee_count, rejected)
    }
}

type DetectionBatch = (Vec<f64>, Vec<Contact>, usize, usize, usize);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_boxes_preserve_exact_contacts_and_balls_during_cache_reuse() {
        for offset in [DVec3::ZERO, DVec3::new(1e6, -2e6, 3e6)] {
            let mut rest = Vec::new();
            let mut triangles = Vec::new();
            for layer in 0..3 {
                for y in 0..5 {
                    for x in 0..5 {
                        rest.push(
                            offset
                                + DVec3::new(
                                    x as f64 * 0.008,
                                    y as f64 * 0.008,
                                    layer as f64 * 0.003,
                                ),
                        );
                    }
                }
                for y in 0..4 {
                    for x in 0..4 {
                        let a = layer * 25 + y * 5 + x;
                        triangles.extend([[a, a + 1, a + 5], [a + 1, a + 6, a + 5]]);
                    }
                }
            }
            let mut pruned = OgcWorld::new(&rest, triangles, 50, [[0, 25], [1, 26]]).unwrap();
            let mut reference = pruned.clone();
            let settings = OgcSettings::default();
            let mut rejections = 0;
            let mut reuse = 0;
            let mut rebuilds = 0;
            for frame in 0..24 {
                let mut x = rest.clone();
                for (i, p) in x[..50].iter_mut().enumerate() {
                    let a = frame as f64 * 0.15 + i as f64 * 0.31;
                    *p += DVec3::new(a.sin() * 0.0004, a.cos() * 0.0003, a.sin() * 0.0002);
                    if frame >= 18 {
                        p.x += 0.004;
                    }
                }
                let rq = match frame {
                    0..=11 => 0.006,
                    12..=15 => 0.009,
                    _ => 0.004,
                };
                let a = pruned.detect(&x, rq, &settings).unwrap();
                let b = reference.detect_impl::<false>(&x, rq, &settings).unwrap();
                assert_eq!(a.radii, b.radii);
                assert_eq!(a.minimum_distance, b.minimum_distance);
                assert_eq!(a.active_contacts, b.active_contacts);
                assert_eq!(a.vertex_face_candidates, b.vertex_face_candidates);
                assert_eq!(a.edge_edge_candidates, b.edge_edge_candidates);
                assert_eq!(a.incident, b.incident);
                assert_eq!(a.broad_phase_rebuilt, b.broad_phase_rebuilt);
                let keys = |c: &ContactSet| {
                    c.contacts
                        .iter()
                        .map(|c| (c.kind, c.vertices, c.count, c.mollifier_epsilon.to_bits()))
                        .collect::<Vec<_>>()
                };
                assert_eq!(keys(&a), keys(&b));
                rejections += a.profile.aabb_rejections;
                rebuilds += usize::from(a.broad_phase_rebuilt);
                reuse += usize::from(!a.broad_phase_rebuilt);
            }
            assert!(rejections > 0 && reuse > 0 && rebuilds > 1);
        }
    }
}

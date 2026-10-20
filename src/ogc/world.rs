use super::{
    ContactIncidence,
    bvh::{Bounds, Bvh},
    candidates::Candidates,
    filter::{CandidateFilter, PrimitiveCandidates, SpatialQuery},
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
    pub geometry_ms: f64,
    pub canonical_ms: f64,
    pub incidence_ms: f64,
    /// Accelerator wall time, including uploads, synchronization and readback.
    pub filter_ms: f64,
    pub filter_pairs: usize,
    pub gpu_filtered: bool,
    pub spatial_queries: usize,
    pub spatial_retries: usize,
    pub filter_prepare_ms: f64,
    pub filter_dispatch_ms: f64,
    pub filter_decode_ms: f64,
    pub candidate_pairs: usize,
    pub candidate_blocks: usize,
    /// Cached pairs rejected by current AABB distance before exact geometry.
    pub aabb_rejections: usize,
}
impl std::ops::AddAssign for DetectionProfile {
    fn add_assign(&mut self, rhs: Self) {
        self.refit_ms += rhs.refit_ms;
        self.candidates_ms += rhs.candidates_ms;
        self.narrow_ms += rhs.narrow_ms;
        self.geometry_ms += rhs.geometry_ms;
        self.canonical_ms += rhs.canonical_ms;
        self.incidence_ms += rhs.incidence_ms;
        self.filter_ms += rhs.filter_ms;
        self.filter_pairs += rhs.filter_pairs;
        self.gpu_filtered |= rhs.gpu_filtered;
        self.spatial_queries += rhs.spatial_queries;
        self.spatial_retries += rhs.spatial_retries;
        self.filter_prepare_ms += rhs.filter_prepare_ms;
        self.filter_dispatch_ms += rhs.filter_dispatch_ms;
        self.filter_decode_ms += rhs.filter_decode_ms;
        self.candidate_pairs += rhs.candidate_pairs;
        self.candidate_blocks += rhs.candidate_blocks;
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
    pub incident: ContactIncidence,
    pub centers: Vec<DVec3>,
    pub radii: Vec<f64>,
    pub directional: super::DirectionalBounds,
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
        let (proposed, clipped) = if len > b {
            (self.centers[v] + d * (b / len), true)
        } else {
            (proposed, false)
        };
        if self.directional.at(v).is_empty() {
            return (proposed, clipped);
        }
        let (delta, planar) = self.directional.truncate(v, proposed - self.centers[v]);
        (self.centers[v] + delta, clipped || planar)
    }
    pub fn contains(&self, v: usize, p: DVec3, tolerance: f64) -> bool {
        p.is_finite()
            && p.distance(self.centers[v]) <= self.radii[v] + tolerance
            && self.directional.contains(
                v,
                p - self.centers[v],
                tolerance.min(32. * f64::EPSILON * p.abs().max_element().max(1.)),
            )
    }
    /// Clip a local Newton segment from its current feasible iterate. The
    /// regions themselves stay centered at the original detection. Scaling
    /// the complete displacement from that center would undo tangential
    /// motion already accepted by the predictor or a previous local solve.
    pub fn truncate_step(&self, v: usize, current: DVec3, proposed: DVec3) -> (DVec3, bool) {
        if self.directional.at(v).is_empty() {
            return self.truncate(v, proposed);
        }
        let offset = current - self.centers[v];
        let d = proposed - current;
        let mut alpha = 1.0_f64;
        for p in self.directional.at(v) {
            let toward = p.normal.dot(d);
            let slack = (p.limit - p.normal.dot(offset)).max(0.);
            if toward > slack {
                alpha = alpha.min(slack / toward);
            }
        }
        if (offset + d).length() > self.radii[v] {
            let a = d.length_squared();
            let b = offset.dot(d);
            let c = (offset.length_squared() - self.radii[v].powi(2)).min(0.);
            if a > 0. {
                let root = (b * b - a * c).max(0.).sqrt();
                let t = if b >= 0. {
                    -c / (b + root).max(f64::MIN_POSITIVE)
                } else {
                    (-b + root) / a
                };
                alpha = alpha.min(t.clamp(0., 1.));
            }
        }
        if alpha < 1. {
            alpha *= 1. - 16. * f64::EPSILON;
        }
        (current + d * alpha, alpha < 1.)
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
    /// CPU narrow-phase implementation. Auto uses f64 AVX when supported;
    /// Scalar is the portable oracle. Does not change the physical model.
    pub cpu_kernel: super::CpuContactKernel,
    /// Sparse directional restrictions. VBD enables this; raw geometry queries
    /// default to isotropic bounds for diagnostics and baseline comparisons.
    pub directional_bounds: bool,
    /// Opt-in CPU experiment. Changes EE energy, never separation bounds.
    pub experimental_interior_edges: bool,
    /// CPU-only finite-primitive potential, with a conservative force halo.
    pub experimental_primitive_distances: bool,
    spatial_id: u64,
    pub topology: Topology,
    pub moving: usize,
    pub fixed: Vec<DVec3>,
    triangles: Bvh,
    edges: Bvh,
    points: Bvh,
    rest_edge_length2: Vec<f64>,
    excluded: BTreeSet<[usize; 2]>,
    moving_faces: Vec<usize>,
    candidates: Option<Candidates>,
    excluded_neighbors: Vec<Vec<usize>>,
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
            cpu_kernel: super::CpuContactKernel::Auto,
            directional_bounds: false,
            experimental_interior_edges: false,
            experimental_primitive_distances: false,
            spatial_id: {
                static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            },
            excluded_neighbors,
            topology,
            moving,
            fixed: x[moving..].to_vec(),
            triangles,
            edges,
            points,
            rest_edge_length2,
            excluded,
            moving_faces,
            candidates: None,
        })
    }
    fn adjacent(&self, a: usize, b: usize) -> bool {
        a == b || self.excluded_neighbors[a].binary_search(&b).is_ok()
    }
    /// Topological eligibility for read-only contact research, including this
    /// world's exact seam exclusions. No distance/cone test or force selection.
    pub fn vertex_face_eligible(&self, vertex: usize, face: usize) -> Result<bool, String> {
        if vertex >= self.topology.neighbors.len() || face >= self.topology.triangles.len() {
            return Err("Invalid vertex/face eligibility query".into());
        }
        let ids = self.topology.triangles[face];
        Ok(
            (vertex < self.moving || ids.iter().any(|i| *i < self.moving))
                && ids.iter().all(|i| !self.adjacent(vertex, *i)),
        )
    }
    pub fn positions(&self, moving: &[DVec3]) -> Vec<DVec3> {
        moving.iter().chain(&self.fixed).copied().collect()
    }
    /// Read-only CPU research audit; never installed as a force law.
    pub fn audit_wire_quadrature(
        &self,
        x: &[DVec3],
        settings: &OgcSettings,
    ) -> Result<super::wire_quadrature::Audit, String> {
        settings.validate()?;
        self.validate_positions(x)?;
        Ok(super::wire_quadrature::audit(
            &self.topology,
            x,
            self.moving,
            &self.excluded,
            &self.rest_edge_length2,
            settings,
        ))
    }
    /// Research-only snapshot hierarchy, using exactly this world's seam
    /// exclusions. Does not alter production candidates, bounds or forces.
    pub fn feature_hierarchy(
        &self,
        x: &[DVec3],
        query_radius: f64,
    ) -> Result<super::feature_hierarchy::Hierarchy, String> {
        self.validate_positions(x)?;
        super::feature_hierarchy::Hierarchy::build(
            &self.topology,
            x,
            self.moving,
            &self.excluded,
            query_radius,
        )
    }
    fn validate_positions(&self, x: &[DVec3]) -> Result<(), String> {
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
        Ok(())
    }
    fn refit_validated(&mut self, x: &[DVec3]) {
        self.triangles
            .refit_dynamic(|i| Bounds::points(self.topology.triangles[i].map(|v| x[v])));
        self.edges
            .refit_dynamic(|i| Bounds::points(self.topology.edges[i].map(|v| x[v])));
        self.points.refit_dynamic(|i| Bounds::points([x[i]]));
    }
    fn refit(&mut self, x: &[DVec3]) -> Result<(), String> {
        self.validate_positions(x)?;
        self.refit_validated(x);
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
        self.detect_impl::<true>(x, query_radius, settings, None)
    }
    pub fn detect_with_filter(
        &mut self,
        x: &[DVec3],
        query_radius: f64,
        settings: &OgcSettings,
        filter: &mut dyn CandidateFilter,
    ) -> Result<ContactSet, String> {
        self.detect_impl::<true>(x, query_radius, settings, Some(filter))
    }
    fn detect_impl<const PRUNE: bool>(
        &mut self,
        x: &[DVec3],
        query_radius: f64,
        settings: &OgcSettings,
        filter: Option<&mut dyn CandidateFilter>,
    ) -> Result<ContactSet, String> {
        settings.validate()?;
        if self.experimental_interior_edges && self.experimental_primitive_distances {
            return Err("Select only one experimental contact law".into());
        }
        if !query_radius.is_finite() || query_radius < settings.radius {
            return Err("OGC query radius must cover the contact radius".into());
        }
        let start = Instant::now();
        let accelerated = filter.is_some();
        if accelerated {
            self.validate_positions(x)?;
        } else {
            self.refit(x)?;
        }
        let mut refit_ms = start.elapsed().as_secs_f64() * 1000.;
        let scale = x
            .iter()
            .map(|p| p.abs().max_element())
            .fold(query_radius, f64::max);
        // A pair outside this force halo cannot enter the activation radius
        // while both primitives remain in their original radius <= relax*q.
        // Geometric bounds still use q, not the enlarged search radius.
        let search_radius = if self.experimental_primitive_distances {
            query_radius.max(settings.radius + 2. * settings.relaxation * query_radius)
                + 64. * f64::EPSILON * scale
        } else {
            query_radius
        };
        let padded_radius2 = (search_radius + 64. * f64::EPSILON * scale).powi(2);
        let spatial_start = Instant::now();
        let direct = if let Some(filter) = filter {
            filter.filter(SpatialQuery {
                id: self.spatial_id,
                x,
                topology: &self.topology,
                triangles: &self.triangles,
                edges: &self.edges,
                moving: self.moving,
                excluded_neighbors: &self.excluded_neighbors,
                padded_radius2,
            })?
        } else {
            None
        };
        let spatial_ms = spatial_start.elapsed().as_secs_f64() * 1000.;
        if direct.is_none() && accelerated {
            let point_start = Instant::now();
            self.refit_validated(x);
            refit_ms += point_start.elapsed().as_secs_f64() * 1000.;
        }
        let broad_start = Instant::now();
        let lanes = rayon::current_num_threads().min(2);
        let broad_phase_rebuilt = direct.is_none()
            && self
                .candidates
                .as_ref()
                .is_none_or(|c| !c.covers(&x[..self.moving], search_radius));
        if broad_phase_rebuilt {
            self.candidates = Some(Candidates::build(
                (&self.points, &self.triangles, &self.edges),
                x,
                self.moving,
                ((search_radius / settings.radius).ceil() * settings.radius).max(search_radius),
                2. * settings.radius,
            ));
        }
        let candidates_ms = broad_start.elapsed().as_secs_f64() * 1000.;
        let filter_ms = spatial_ms;
        let candidate_pairs = if direct.is_some() {
            0
        } else {
            self.candidates.as_ref().unwrap().pair_count
        };
        let candidate_blocks = if direct.is_some() {
            0
        } else {
            let c = self.candidates.as_ref().unwrap();
            c.vf.len() + c.ee.len()
        };
        let filter_pairs = direct
            .as_ref()
            .map_or(candidate_pairs, |p| p.vf.len() + p.ee.len());
        let narrow_start = Instant::now();
        let batches: Vec<_> = (0..lanes)
            .into_par_iter()
            .map(|lane| {
                self.scan::<PRUNE>(
                    x,
                    (query_radius, settings.relaxation, search_radius),
                    padded_radius2,
                    lane,
                    lanes,
                    direct.as_ref(),
                )
            })
            .collect();
        let mut distance = vec![query_radius; x.len()];
        let mut bound_distance = self.directional_bounds.then(|| vec![query_radius; x.len()]);
        let mut planes = Vec::new();
        let mut found = Vec::new();
        let mut vf_count = 0;
        let mut ee_count = 0;
        let mut aabb_rejections = 0;
        for (distances, contacts, vf, ee, rejected, directional) in batches {
            for (d, v) in distance.iter_mut().zip(distances) {
                *d = d.min(v);
            }
            found.extend(contacts);
            vf_count += vf;
            ee_count += ee;
            aabb_rejections += rejected;
            if let Some(builder) = directional {
                for (d, v) in bound_distance
                    .as_mut()
                    .unwrap()
                    .iter_mut()
                    .zip(builder.distance)
                {
                    *d = d.min(v);
                }
                planes.extend(builder.entries);
            }
        }
        let geometry_ms = narrow_start.elapsed().as_secs_f64() * 1000.;
        let canonical_start = Instant::now();
        // Sort/merge once instead of allocating a tree node for every retained
        // nearby term. Canonical order remains the same for colors/reductions.
        let contacts = super::canonical::canonicalize(found, x.len());
        let active_contacts = contacts
            .iter()
            .filter(|c| c.value(x, settings.radius, settings.stiffness) > 0.)
            .count();
        let canonical_ms = canonical_start.elapsed().as_secs_f64() * 1000.;
        let incidence_start = Instant::now();
        let incident = ContactIncidence::new(self.moving, &contacts);
        let minimum_distance = distance[..self.moving]
            .iter()
            .copied()
            .fold(query_radius, f64::min);
        Ok(ContactSet {
            profile: DetectionProfile {
                refit_ms,
                candidates_ms,
                filter_ms,
                filter_pairs,
                gpu_filtered: direct.is_some(),
                spatial_queries: if direct.is_some() {
                    x.len() + self.topology.edges.len()
                } else {
                    0
                },
                spatial_retries: direct.as_ref().map_or(0, |p| p.retries),
                filter_prepare_ms: direct.as_ref().map_or(0., |p| p.timings[0]),
                filter_dispatch_ms: direct.as_ref().map_or(0., |p| p.timings[1]),
                filter_decode_ms: direct.as_ref().map_or(0., |p| p.timings[2]),
                narrow_ms: narrow_start.elapsed().as_secs_f64() * 1000.,
                geometry_ms,
                canonical_ms,
                incidence_ms: incidence_start.elapsed().as_secs_f64() * 1000.,
                candidate_pairs,
                candidate_blocks,
                aabb_rejections,
            },
            contacts,
            active_contacts,
            incident,
            centers: x[..self.moving].to_vec(),
            radii: bound_distance.as_ref().unwrap_or(&distance)[..self.moving]
                .iter()
                .map(|d| settings.relaxation * d)
                .collect(),
            directional: super::DirectionalBounds::from_entries(planes, self.moving),
            minimum_distance,
            vertex_face_candidates: vf_count,
            edge_edge_candidates: ee_count,
            broad_phase_rebuilt,
        })
    }
    fn scan<const PRUNE: bool>(
        &self,
        x: &[DVec3],
        query: (f64, f64, f64),
        padded_radius2: f64,
        lane: usize,
        lanes: usize,
        direct: Option<&PrimitiveCandidates>,
    ) -> DetectionBatch {
        if direct.is_some() {
            self.scan_impl::<PRUNE, true>(x, query, padded_radius2, lane, lanes, direct)
        } else {
            self.scan_impl::<PRUNE, false>(x, query, padded_radius2, lane, lanes, direct)
        }
    }
    fn scan_impl<const PRUNE: bool, const DIRECT: bool>(
        &self,
        x: &[DVec3],
        query: (f64, f64, f64),
        padded_radius2: f64,
        lane: usize,
        lanes: usize,
        direct: Option<&PrimitiveCandidates>,
    ) -> DetectionBatch {
        let (query_radius, relaxation, search_radius) = query;
        let top = &self.topology;
        let mut distance = vec![query_radius; x.len()];
        let mut directional = self.directional_bounds.then(|| {
            super::directional::Builder::new(x.len(), self.moving, query_radius, relaxation)
        });
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
            if matches!(kind, Kind::PointFace | Kind::PointTriangle) {
                ids[1..].sort_unstable();
            }
            if matches!(
                kind,
                Kind::EdgeEdge | Kind::EdgeEdgeInterior | Kind::SegmentSegment
            ) {
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
        let candidates = self.candidates.as_ref();
        let vf_len = if DIRECT {
            direct.unwrap().vf.len()
        } else {
            candidates.unwrap().vf.len()
        };
        for block in (lane..vf_len).step_by(lanes) {
            let (index, mask) = (block, u64::MAX);
            let [vp, tp] = if DIRECT {
                [0, 0]
            } else {
                candidates.unwrap().vf[index]
            };
            let (vs, ts, mask) = if DIRECT {
                let p = direct.unwrap();
                (&p.vf[block][..1], &p.vf[block][1..], 1)
            } else {
                (self.points.leaf_ids(vp), self.triangles.leaf_ids(tp), mask)
            };
            if PRUNE
                && !DIRECT
                && self
                    .points
                    .leaf_bounds(vp)
                    .distance_squared(self.triangles.leaf_bounds(tp))
                    > padded_radius2
            {
                rejected += vs.len() * ts.len();
                continue;
            }
            for (i, j) in LeafPairs::new(vs.len(), ts.len(), false, mask) {
                let (v, t) = (vs[i], ts[j]);
                if PRUNE
                    && !DIRECT
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
                if ids.iter().any(|i| self.adjacent(v, *i)) {
                    continue;
                }
                let (p, f) = geometry::closest_triangle(x[v], t, x, top);
                let d = x[v].distance(p);
                if d >= search_radius {
                    continue;
                }
                vf_count += 1;
                distance[v] = distance[v].min(d);
                for i in ids {
                    distance[i] = distance[i].min(d);
                }
                if let Some(builder) = &mut directional {
                    builder.pair(&[v], &ids, [x[v], p], d, x);
                }
                // Keep nearby zero-energy terms too: a local iterate can cross r
                // before the next detection. The potential itself activates at r.
                if self.experimental_primitive_distances {
                    add(Kind::PointTriangle, [v, ids[0], ids[1], ids[2]], 4, 0.0);
                    continue;
                }
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
        }
        let edge_kernel = super::simd::EdgeKernel::new(self.cpu_kernel);
        let mut edge_ids = [[0; 2]; 4];
        let mut edge_points = [[DVec3::ZERO; 4]; 4];
        let mut pending = 0;
        let mut finish_edge = |e: usize, f: usize, hit: super::simd::EdgeClosest| {
            let ids = &top.edges[e];
            let js = top.edges[f];
            let super::simd::EdgeClosest {
                s,
                t,
                p,
                q,
                distance: dist,
            } = hit;
            if dist >= search_radius {
                return;
            }
            ee_count += 1;
            for v in ids.iter().chain(&js) {
                distance[*v] = distance[*v].min(dist);
            }
            if let Some(builder) = &mut directional {
                builder.pair(ids, &js, [p, q], dist, x);
            }
            if self.experimental_primitive_distances {
                add(
                    Kind::SegmentSegment,
                    [ids[0], ids[1], js[0], js[1]],
                    4,
                    1e-3 * self.rest_edge_length2[e] * self.rest_edge_length2[f],
                );
                return;
            }
            if self.experimental_interior_edges {
                // Retain zero-energy pairs outside the interior too. Their
                // participation is evaluated/differentiated at every iterate,
                // so entering the support does not add a finite energy term.
                add(
                    Kind::EdgeEdgeInterior,
                    [ids[0], ids[1], js[0], js[1]],
                    4,
                    1e-3 * self.rest_edge_length2[e] * self.rest_edge_length2[f],
                );
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
                return;
            }
            match (end_a, end_b) {
                (Some(i), Some(j)) => add(Kind::PointPoint, [i, j, 0, 0], 2, 0.0),
                (Some(i), None) => add(Kind::PointEdge, [i, js[0], js[1], 0], 3, 0.0),
                (None, Some(j)) => add(Kind::PointEdge, [j, ids[0], ids[1], 0], 3, 0.0),
                (None, None) if !self.experimental_interior_edges => add(
                    Kind::EdgeEdge,
                    [ids[0], ids[1], js[0], js[1]],
                    4,
                    1e-3 * self.rest_edge_length2[e] * self.rest_edge_length2[f],
                ),
                (None, None) => {}
            }
        };
        let ee_len = if DIRECT {
            direct.unwrap().ee.len()
        } else {
            candidates.unwrap().ee.len()
        };
        for block in (lane..ee_len).step_by(lanes) {
            let (index, mask) = (block, u64::MAX);
            let [ep, fp] = if DIRECT {
                [0, 1]
            } else {
                candidates.unwrap().ee[index]
            };
            let (es, fs, mask) = if DIRECT {
                let p = direct.unwrap();
                (&p.ee[block][..1], &p.ee[block][1..], 1)
            } else {
                (self.edges.leaf_ids(ep), self.edges.leaf_ids(fp), mask)
            };
            if PRUNE
                && !DIRECT
                && self
                    .edges
                    .leaf_bounds(ep)
                    .distance_squared(self.edges.leaf_bounds(fp))
                    > padded_radius2
            {
                rejected += if ep == fp {
                    es.len() * (es.len() - 1) / 2
                } else {
                    es.len() * fs.len()
                };
                continue;
            }
            for (i, j) in LeafPairs::new(es.len(), fs.len(), ep == fp, mask) {
                let (a, b) = (es[i], fs[j]);
                let (mut e, mut f) = (a.min(b), a.max(b));
                if top.edges[e].iter().all(|i| *i >= self.moving) {
                    std::mem::swap(&mut e, &mut f);
                }
                if PRUNE
                    && !DIRECT
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
                if ids.iter().any(|i| js.iter().any(|j| self.adjacent(*i, *j))) {
                    continue;
                }
                let [a, b] = ids.map(|v| x[v]);
                let [c, d] = js.map(|v| x[v]);
                if edge_kernel.is_wide() {
                    edge_ids[pending] = [e, f];
                    edge_points[pending] = [a, b, c, d];
                    pending += 1;
                    if pending == 4 {
                        for ([e, f], hit) in
                            edge_ids.into_iter().zip(edge_kernel.batch(edge_points))
                        {
                            finish_edge(e, f, hit);
                        }
                        pending = 0;
                    }
                } else {
                    finish_edge(e, f, super::simd::edge_scalar([a, b, c, d]));
                }
            }
        }
        for i in 0..pending {
            let [e, f] = edge_ids[i];
            finish_edge(e, f, super::simd::edge_scalar(edge_points[i]));
        }
        (distance, found, vf_count, ee_count, rejected, directional)
    }
}

struct LeafPairs(u64);
impl LeafPairs {
    fn new(left: usize, right: usize, diagonal: bool, mask: u64) -> Self {
        if mask != u64::MAX {
            return Self(mask);
        }
        let mut valid = 0;
        for i in 0..left {
            let row = (1u64 << right) - 1;
            let row = if diagonal {
                row & !((1 << (i + 1)) - 1)
            } else {
                row
            };
            valid |= row << (i * 8);
        }
        Self(valid)
    }
}
impl Iterator for LeafPairs {
    type Item = (usize, usize);
    fn next(&mut self) -> Option<Self::Item> {
        if self.0 == 0 {
            return None;
        }
        let bit = self.0.trailing_zeros() as usize;
        self.0 &= self.0 - 1;
        Some((bit / 8, bit % 8))
    }
}

type DetectionBatch = (
    Vec<f64>,
    Vec<Contact>,
    usize,
    usize,
    usize,
    Option<super::directional::Builder>,
);

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
                let b = reference
                    .detect_impl::<false>(&x, rq, &settings, None)
                    .unwrap();
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

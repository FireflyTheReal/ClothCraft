//! Read-only assembly audit for the unmollified wire-density formula in
//! Convergent IPC §4.3. No quadrature area weights or solver integration.
//! We reuse OGC's explicit seam exclusions and remove the excluded vertices'
//! incident edges consistently, so correction degrees belong to that subgraph.
//! A negative density is a rejection witness, not something to clamp to zero.
use super::{
    OgcSettings,
    bvh::{Bounds, Bvh},
    potential::activation,
    primitive_distance::{point_segment, segment_segment},
    topology::Topology,
};
use glam::DVec3;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize)]
pub struct Row {
    pub source_edge: usize,
    pub source_vertices: [usize; 2],
    /// Transient diagnostic IDs, never authored/project references.
    pub target_edges: Vec<usize>,
    pub active_pairs: usize,
    pub components: usize,
    /// A capsule/graph analogue of the paper's local-scale condition; it is
    /// reported separately from the directly measured sign of the density.
    pub components_with_multiple_vertices: usize,
    pub max_vertices_in_component: usize,
    pub minimum_distance_m: f64,
    pub primitive_sum_j: f64,
    pub subtraction_j: f64,
    pub signed_density_j: f64,
    /// Deliberately inconsistent comparison: OGC mollifies the positive terms
    /// only, with this world's initialization lengths. Not a candidate law.
    pub positive_only_mollified_density_j: f64,
    /// Independent graph experiment: subtract only component merges, never
    /// cycle closures, in ascending geometric-distance order. Value only;
    /// differentiability at changes of merge order is NOT established.
    pub forest_density_j: f64,
    pub cycle_corrections_skipped: usize,
    pub maximum_primitive_j: f64,
}
#[derive(Debug, Serialize)]
pub struct Audit {
    pub radius_m: f64,
    pub stiffness_n_m: f64,
    pub source_edges: usize,
    pub active_source_edges: usize,
    pub active_pairs: usize,
    pub singular_source_edges: usize,
    pub negative_source_edges: usize,
    pub positive_only_mollified_negative_sources: usize,
    pub forest_below_maximum_primitive: usize,
    pub cyclic_sources: usize,
    pub below_maximum_primitive: usize,
    pub sources_with_multiple_vertices: usize,
    pub max_vertices_in_component: usize,
    pub minimum_signed_density_j: f64,
    pub minimum_positive_only_mollified_density_j: f64,
    pub minimum_density_to_maximum_ratio: f64,
    /// Worst 32 measured signed densities, deterministically ordered.
    pub worst_rows: Vec<Row>,
    /// Largest 32 active target graphs; includes crowded positive densities.
    pub largest_graph_rows: Vec<Row>,
    pub worst_positive_only_mollified_rows: Vec<Row>,
    /// Full edge scans and global allowed degrees, without BVH/active degrees.
    /// They share the finite-distance kernel; this is an assembly oracle.
    pub oracle_checked_sources: usize,
    pub oracle_mismatch_sources: usize,
    pub oracle_max_density_error_j: f64,
    pub oracle_max_pair_count_error: usize,
}

fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

pub(super) fn audit(
    top: &Topology,
    x: &[DVec3],
    moving: usize,
    excluded: &BTreeSet<[usize; 2]>,
    reference_edge_length2: &[f64],
    cfg: &OgcSettings,
) -> Audit {
    let boxes: Vec<_> = top
        .edges
        .iter()
        .map(|e| Bounds::points(e.map(|i| x[i])))
        .collect();
    let dynamic: Vec<_> = top
        .edges
        .iter()
        .map(|e| e.iter().any(|i| *i < moving))
        .collect();
    let bvh = Bvh::new(&boxes, &dynamic);
    let excluded = |a: usize, b: usize| a == b || excluded.contains(&[a.min(b), a.max(b)]);
    let rows: Vec<_> = top
        .edges
        .par_iter()
        .enumerate()
        .filter_map(|(source, ids)| {
            let [a, b] = ids.map(|i| x[i].to_array());
            let mut pairs = Vec::new();
            bvh.query(
                boxes[source].inflate(cfg.radius),
                !dynamic[source],
                |target| {
                    let other = top.edges[target];
                    if ids.iter().any(|i| other.iter().any(|j| excluded(*i, *j))) {
                        return;
                    }
                    let [c, d] = other.map(|i| x[i].to_array());
                    let distance = segment_segment([a, b, c, d]);
                    if distance < cfg.radius {
                        pairs.push((target, distance));
                    }
                },
            );
            pairs.sort_unstable_by_key(|p| p.0);
            if pairs.is_empty() {
                return None;
            }
            // Every incident allowed edge of a vertex inside the capsule must
            // occur in this complete active list: d(segment, edge) <= d(segment, v).
            let mut incident: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
            let mut primitive_sum = 0.;
            let mut mollified_sum = 0.;
            let mut maximum: f64 = 0.;
            let mut minimum = f64::INFINITY;
            for (local, &(edge, d)) in pairs.iter().enumerate() {
                let e = activation(d, cfg.radius, cfg.stiffness);
                primitive_sum += e;
                let target_ids = top.edges[edge];
                let cross2 = (x[ids[1]] - x[ids[0]])
                    .cross(x[target_ids[1]] - x[target_ids[0]])
                    .length_squared();
                let epsilon = 1e-3 * reference_edge_length2[source] * reference_edge_length2[edge];
                let q = cross2 / epsilon;
                let weight = if cross2 < 1e-30 {
                    0.
                } else if q < 1. {
                    q * (2. - q)
                } else {
                    1.
                };
                mollified_sum += e * weight;
                maximum = maximum.max(e);
                minimum = minimum.min(d);
                for v in top.edges[edge] {
                    incident.entry(v).or_default().push(local);
                }
            }
            let mut parent: Vec<_> = (0..pairs.len()).collect();
            let mut inside = Vec::new();
            let mut subtraction = 0.;
            let mut forest_subtraction = 0.;
            let mut cycle_corrections_skipped = 0;
            let mut events: Vec<_> = incident
                .iter()
                .filter_map(|(&vertex, edges)| {
                    let d = point_segment(x[vertex].to_array(), a, b);
                    (d < cfg.radius).then_some((d, vertex, edges))
                })
                .collect();
            events.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            for (d, _vertex, edges) in events {
                subtraction += (edges.len() - 1) as f64 * activation(d, cfg.radius, cfg.stiffness);
                for &edge in &edges[1..] {
                    let r = root(&mut parent, edges[0]);
                    let s = root(&mut parent, edge);
                    if r != s {
                        // Each component contains an incident edge whose
                        // distance is <= d, so this subtraction cannot
                        // exceed either component's maximum barrier.
                        forest_subtraction += activation(d, cfg.radius, cfg.stiffness);
                        parent[s] = r;
                    } else {
                        cycle_corrections_skipped += 1;
                    }
                }
                inside.push(edges[0]);
            }
            let mut component_vertices: BTreeMap<usize, usize> = BTreeMap::new();
            for i in 0..pairs.len() {
                let r = root(&mut parent, i);
                component_vertices.entry(r).or_default();
            }
            for i in inside {
                let r = root(&mut parent, i);
                *component_vertices.get_mut(&r).unwrap() += 1;
            }
            Some(Row {
                source_edge: source,
                source_vertices: *ids,
                target_edges: pairs.iter().map(|p| p.0).collect(),
                active_pairs: pairs.len(),
                components: component_vertices.len(),
                components_with_multiple_vertices: component_vertices
                    .values()
                    .filter(|n| **n > 1)
                    .count(),
                max_vertices_in_component: component_vertices.values().copied().max().unwrap_or(0),
                minimum_distance_m: minimum,
                primitive_sum_j: primitive_sum,
                subtraction_j: subtraction,
                signed_density_j: primitive_sum - subtraction,
                positive_only_mollified_density_j: mollified_sum - subtraction,
                forest_density_j: primitive_sum - forest_subtraction,
                cycle_corrections_skipped,
                maximum_primitive_j: maximum,
            })
        })
        .collect();
    let mut largest: Vec<_> = rows.iter().collect();
    largest.sort_by(|a, b| {
        b.active_pairs
            .cmp(&a.active_pairs)
            .then(a.source_edge.cmp(&b.source_edge))
    });
    let largest_graph_rows = largest.into_iter().take(32).cloned().collect();
    let mut result = Audit {
        radius_m: cfg.radius,
        stiffness_n_m: cfg.stiffness,
        source_edges: top.edges.len(),
        active_source_edges: rows.len(),
        active_pairs: rows.iter().map(|r| r.active_pairs).sum(),
        singular_source_edges: 0,
        negative_source_edges: 0,
        positive_only_mollified_negative_sources: 0,
        forest_below_maximum_primitive: 0,
        cyclic_sources: 0,
        below_maximum_primitive: 0,
        sources_with_multiple_vertices: 0,
        max_vertices_in_component: 0,
        minimum_signed_density_j: 0.,
        minimum_positive_only_mollified_density_j: 0.,
        minimum_density_to_maximum_ratio: 1.,
        worst_rows: Vec::new(),
        largest_graph_rows,
        worst_positive_only_mollified_rows: Vec::new(),
        oracle_checked_sources: 0,
        oracle_mismatch_sources: 0,
        oracle_max_density_error_j: 0.,
        oracle_max_pair_count_error: 0,
    };
    for row in rows {
        if !row.signed_density_j.is_finite() {
            result.singular_source_edges += 1;
            continue;
        }
        let tolerance = 1e-12 * row.primitive_sum_j.max(1.);
        result.negative_source_edges += usize::from(row.signed_density_j < -tolerance);
        result.forest_below_maximum_primitive += usize::from(
            !row.forest_density_j.is_finite()
                || row.forest_density_j + tolerance < row.maximum_primitive_j,
        );
        result.cyclic_sources += usize::from(row.cycle_corrections_skipped > 0);
        result.positive_only_mollified_negative_sources +=
            usize::from(row.positive_only_mollified_density_j < -tolerance);
        result.minimum_positive_only_mollified_density_j = result
            .minimum_positive_only_mollified_density_j
            .min(row.positive_only_mollified_density_j);
        result.below_maximum_primitive +=
            usize::from(row.signed_density_j + tolerance < row.maximum_primitive_j);
        result.sources_with_multiple_vertices +=
            usize::from(row.components_with_multiple_vertices > 0);
        result.max_vertices_in_component = result
            .max_vertices_in_component
            .max(row.max_vertices_in_component);
        result.minimum_signed_density_j = result.minimum_signed_density_j.min(row.signed_density_j);
        result.minimum_density_to_maximum_ratio = result
            .minimum_density_to_maximum_ratio
            .min(row.signed_density_j / row.maximum_primitive_j);
        result.worst_positive_only_mollified_rows.push(row.clone());
        result.worst_rows.push(row);
    }
    result.worst_rows.sort_by(|a, b| {
        a.signed_density_j
            .total_cmp(&b.signed_density_j)
            .then(a.source_edge.cmp(&b.source_edge))
    });
    result.worst_rows.truncate(32);
    result.worst_positive_only_mollified_rows.sort_by(|a, b| {
        a.positive_only_mollified_density_j
            .total_cmp(&b.positive_only_mollified_density_j)
            .then(a.source_edge.cmp(&b.source_edge))
    });
    result.worst_positive_only_mollified_rows.truncate(32);
    for row in &result.worst_rows {
        let [a, b] = row.source_vertices;
        let mut degrees = vec![0usize; x.len()];
        let mut total = 0.;
        let mut count = 0usize;
        for [c, d] in &top.edges {
            if (a >= moving && b >= moving && *c >= moving && *d >= moving)
                || [a, b]
                    .iter()
                    .any(|i| [*c, *d].iter().any(|j| excluded(*i, *j)))
            {
                continue;
            }
            degrees[*c] += 1;
            degrees[*d] += 1;
            let distance = segment_segment([a, b, *c, *d].map(|i| x[i].to_array()));
            total += activation(distance, cfg.radius, cfg.stiffness);
            count += usize::from(distance < cfg.radius);
        }
        for (v, degree) in degrees.into_iter().enumerate() {
            if degree > 1 {
                let ab = x[b] - x[a];
                let t = ((x[v] - x[a]).dot(ab) / ab.length_squared()).clamp(0., 1.);
                total -= (degree - 1) as f64
                    * activation(x[v].distance(x[a] + ab * t), cfg.radius, cfg.stiffness);
            }
        }
        result.oracle_checked_sources += 1;
        if !total.is_finite() {
            result.oracle_mismatch_sources += 1;
            continue;
        }
        result.oracle_mismatch_sources += usize::from(
            count != row.active_pairs
                || (total - row.signed_density_j).abs() > 1e-10 * (1. + row.primitive_sum_j),
        );
        result.oracle_max_density_error_j = result
            .oracle_max_density_error_j
            .max((total - row.signed_density_j).abs());
        result.oracle_max_pair_count_error = result
            .oracle_max_pair_count_error
            .max(count.abs_diff(row.active_pairs));
    }
    result
}

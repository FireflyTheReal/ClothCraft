//! Passive feature-refresh diagnostics shared by production and research solvers.
use super::{ContactSet, OgcSettings};
use glam::DVec3;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct RefreshEvent {
    pub sweep: usize,
    pub delta_j: f64,
    /// Diagnostic geometry only: not a resumable dynamic checkpoint.
    pub dynamic_positions_m: Vec<DVec3>,
    /// Last detection geometry, to distinguish feature switches from cache misses.
    pub previous_detection_positions_m: Vec<DVec3>,
    pub largest_feature_changes: Vec<FeatureChange>,
}
#[derive(Clone, Debug, Serialize)]
pub struct FeatureChange {
    pub kind: String,
    pub vertices: [usize; 4],
    pub mollifier_epsilon_bits: u64,
    pub delta_j: f64,
}
pub fn refresh_event(
    old: &ContactSet,
    new: &ContactSet,
    x: &[DVec3],
    n: usize,
    s: &OgcSettings,
    sweep: usize,
    delta_j: f64,
) -> RefreshEvent {
    let mut changes = std::collections::BTreeMap::new();
    for (set, sign) in [(old, -1.), (new, 1.)] {
        for c in &set.contacts {
            *changes
                .entry((c.kind, c.vertices, c.mollifier_epsilon.to_bits()))
                .or_insert(0.) += sign * c.value(x, s.radius, s.stiffness);
        }
    }
    let mut largest: Vec<_> = changes
        .into_iter()
        .filter(|(_, delta)| *delta != 0.)
        .map(|((kind, vertices, bits), delta)| FeatureChange {
            kind: format!("{kind:?}"),
            vertices,
            mollifier_epsilon_bits: bits,
            delta_j: delta,
        })
        .collect();
    largest.sort_by(|a, b| b.delta_j.abs().total_cmp(&a.delta_j.abs()));
    largest.truncate(32);
    RefreshEvent {
        sweep,
        delta_j,
        dynamic_positions_m: x[..n].to_vec(),
        previous_detection_positions_m: old.centers.clone(),
        largest_feature_changes: largest,
    }
}

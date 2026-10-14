//! Observable deformation, measured against the immutable material-space rest.
use super::CompiledGarment;
use crate::{energy, model::State};

/// Positive axial extension: area-weighted 95th percentile and maximum, in %.
/// Not stress, residual or a convergence certificate; compression is omitted.
pub fn extension_percent(garment: &CompiledGarment, state: &State) -> [f64; 2] {
    let mut values: Vec<_> = garment
        .mesh
        .triangles
        .iter()
        .map(|t| {
            let stretch = energy::stretches(&state.x, t, &garment.materials[t.material]);
            (100. * (stretch[0].max(stretch[1]) - 1.).max(0.), t.area)
        })
        .collect();
    values.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = values.iter().map(|(_, a)| a).sum();
    let mut area = 0.;
    let p95 = values
        .iter()
        .find_map(|(e, a)| {
            area += a;
            (area >= 0.95 * total).then_some(*e)
        })
        .unwrap_or(0.);
    [p95, values.last().map_or(0., |v| v.0)]
}

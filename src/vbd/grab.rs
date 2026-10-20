//! Surface brush attachments. These add implicit spring energies; they never
//! project positions or bypass collision constraints.
use super::Grab;
use glam::DVec3;

#[derive(Clone, Debug)]
pub struct PatchAttachment {
    pub vertex: usize,
    /// Initial offset from the cursor, retained during the entire gesture.
    pub offset: DVec3,
    /// Mass-weighted radial influence, normalized over the brush.
    pub weight: f64,
}

impl Grab {
    /// Keep total spring stiffness independent of brush size or vertex count.
    pub fn point_stiffness(&self) -> f64 {
        self.stiffness * if self.patch.is_empty() { 1. } else { 0.25 }
    }

    pub fn patch_stiffness(&self) -> f64 {
        self.stiffness * 0.75
    }
    pub fn attachment(&self, vertex: usize) -> Option<&PatchAttachment> {
        self.patch
            .binary_search_by_key(&vertex, |a| a.vertex)
            .ok()
            .map(|i| &self.patch[i])
    }

    pub fn valid_patch(&self, vertices: usize) -> bool {
        self.patch.iter().all(|a| {
            a.vertex < vertices
                && a.offset.is_finite()
                && a.weight.is_finite()
                && a.weight > 0.
                && a.weight <= 1.
        }) && self.patch.windows(2).all(|a| a[0].vertex < a[1].vertex)
            && (self.patch.is_empty()
                || (self.patch.iter().map(|a| a.weight).sum::<f64>() - 1.).abs() < 1e-8)
    }

    pub fn patch_energy(&self, x: &[DVec3]) -> f64 {
        self.patch
            .iter()
            .map(|a| {
                0.5 * self.patch_stiffness()
                    * a.weight
                    * x[a.vertex].distance_squared(self.target + a.offset)
            })
            .sum()
    }
}

/// Build a brush on the picked panel's connected triangle component. Geodesic
/// edge distance prevents grabbing a nearby disconnected piece or the opposite
/// side of a folded sheet just because it is close in world space.
pub fn surface_patch(
    mesh: &crate::model::Mesh,
    x: &[DVec3],
    faces: std::ops::Range<usize>,
    picked: [usize; 3],
    center: DVec3,
    radius: f64,
) -> Vec<PatchAttachment> {
    use std::{cmp::Reverse, collections::BinaryHeap};
    if !radius.is_finite()
        || radius <= 0.
        || !center.is_finite()
        || x.len() != mesh.masses.len()
        || x.len() != mesh.pins.len()
        || faces.start > faces.end
        || faces.end > mesh.triangles.len()
        || picked.iter().any(|&v| v >= x.len())
        || x.iter().any(|p| !p.is_finite())
        || mesh.triangles[faces.clone()]
            .iter()
            .any(|t| t.vertices.iter().any(|&v| v >= x.len()))
        || !mesh.triangles[faces.clone()]
            .iter()
            .any(|t| t.vertices == picked)
    {
        return Vec::new();
    }
    let mut adjacency = vec![Vec::new(); x.len()];
    for t in &mesh.triangles[faces] {
        for (a, b) in [
            (t.vertices[0], t.vertices[1]),
            (t.vertices[1], t.vertices[2]),
            (t.vertices[2], t.vertices[0]),
        ] {
            let length = x[a].distance(x[b]);
            adjacency[a].push((b, length));
            adjacency[b].push((a, length));
        }
    }
    let mut distance = vec![f64::INFINITY; x.len()];
    let mut queue = BinaryHeap::new();
    for v in picked {
        let d = x[v].distance(center);
        distance[v] = d;
        queue.push(Reverse((d.to_bits(), v)));
    }
    while let Some(Reverse((bits, v))) = queue.pop() {
        let d = f64::from_bits(bits);
        if d > radius || d > distance[v] {
            continue;
        }
        for &(u, length) in &adjacency[v] {
            let next = d + length;
            if next < radius && next < distance[u] {
                distance[u] = next;
                queue.push(Reverse((next.to_bits(), u)));
            }
        }
    }
    let mut patch: Vec<_> = distance
        .iter()
        .enumerate()
        .filter_map(|(v, &d)| {
            let weight = mesh.masses[v] * (1. - d / radius).max(0.).powi(2);
            (weight > 0. && mesh.pins[v].is_none()).then_some(PatchAttachment {
                vertex: v,
                offset: x[v] - center,
                weight,
            })
        })
        .collect();
    let sum = patch.iter().map(|a| a.weight).sum::<f64>();
    for a in &mut patch {
        a.weight /= sum;
    }
    patch
}

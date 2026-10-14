//! Persistent material coordinates -> transient barycentric attachments.
//! Useful for grain guides, future decorative yarns or surface landmarks.
//! These attachments introduce no additional mechanical degrees of freedom.
use super::CompiledGarment;
use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialPath {
    pub panel: String,
    pub points: Vec<DVec2>,
}

/// Rebuild after remeshing. Never persist these vertex indices as authoring data.
#[derive(Clone, Debug)]
pub struct EmbeddedPoint {
    pub vertices: [usize; 3],
    pub weights: [f64; 3],
    pub uv: DVec2,
}
impl EmbeddedPoint {
    pub fn evaluate(&self, x: &[DVec3]) -> Result<DVec3, String> {
        if self.vertices.iter().any(|i| *i >= x.len()) {
            return Err("Attachment does not belong to this state".into());
        }
        let p = self
            .vertices
            .iter()
            .zip(self.weights)
            .map(|(i, w)| x[*i] * w)
            .sum::<DVec3>();
        if !p.is_finite() {
            return Err("Non-finite attachment position".into());
        }
        Ok(p)
    }
}
fn weights(g: &CompiledGarment, triangle: usize, uv: DVec2) -> [f64; 3] {
    let t = &g.mesh.triangles[triangle];
    let b = t.inv_dm * (uv - t.rest[0]);
    [1.0 - b.x - b.y, b.x, b.y]
}
impl CompiledGarment {
    /// Boundary ties resolve to the first triangle in deterministic mesh order.
    pub fn embed_point(&self, panel: &str, uv: DVec2) -> Result<EmbeddedPoint, String> {
        if !uv.is_finite() {
            return Err("Non-finite material coordinate".into());
        }
        let range = self
            .panels
            .iter()
            .find(|p| p.id == panel)
            .ok_or_else(|| format!("Unknown panel {panel}"))?;
        for triangle in range.triangles.clone() {
            let w = weights(self, triangle, uv);
            if w.iter().all(|v| *v >= -1e-10 && *v <= 1.0 + 1e-10) {
                return Ok(EmbeddedPoint {
                    vertices: self.mesh.triangles[triangle].vertices,
                    weights: w,
                    uv,
                });
            }
        }
        Err(format!("Material coordinate {uv:?} outside panel {panel}"))
    }
    /// Split every segment at triangle crossings, so the attached path follows
    /// the piecewise-linear deformed surface even where it bends sharply.
    pub fn embed_path(&self, path: &MaterialPath) -> Result<Vec<EmbeddedPoint>, String> {
        if path.points.len() < 2 || path.points.len() > 10_000 {
            return Err("Material path requires 2..10000 control points".into());
        }
        for p in &path.points {
            self.embed_point(&path.panel, *p)?;
        }
        let range = self.panels.iter().find(|p| p.id == path.panel).unwrap();
        let mut result = vec![self.embed_point(&path.panel, path.points[0])?];
        for segment in path.points.windows(2) {
            if segment[0].distance_squared(segment[1]) < 1e-24 {
                continue;
            }
            let mut breaks = vec![0.0, 1.0];
            for triangle in range.triangles.clone() {
                let a = weights(self, triangle, segment[0]);
                let b = weights(self, triangle, segment[1]);
                let mut lo = 0.0_f64;
                let mut hi = 1.0_f64;
                for i in 0..3 {
                    let slope = b[i] - a[i];
                    if slope.abs() < 1e-14 {
                        if a[i] < -1e-10 {
                            hi = -1.0;
                            break;
                        }
                    } else if slope > 0.0 {
                        lo = lo.max(-a[i] / slope);
                    } else {
                        hi = hi.min(-a[i] / slope);
                    }
                }
                if hi >= lo && lo <= 1.0 && hi >= 0.0 {
                    breaks.extend([lo.max(0.0), hi.min(1.0)]);
                }
            }
            breaks.sort_by(f64::total_cmp);
            breaks.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
            for interval in breaks.windows(2) {
                // Reject paths crossing holes if arbitrary panels are added later.
                self.embed_point(
                    &path.panel,
                    segment[0].lerp(segment[1], (interval[0] + interval[1]) * 0.5),
                )?;
                result
                    .push(self.embed_point(&path.panel, segment[0].lerp(segment[1], interval[1]))?);
            }
        }
        Ok(result)
    }
}

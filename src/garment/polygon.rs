//! Small simple-polygon mesher with named boundary edges. No holes or curves yet.
use super::meshing::{self, MeshingSettings};
use crate::model::{Hinge, Material, Mesh, Triangle};
use glam::{DMat2, DVec2};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolygonPattern {
    /// Counterclockwise in the numerical (u,v) plane; v is displayed downward.
    pub points: Vec<DVec2>,
    /// Name of the directed edge i -> (i+1)%n; names survive refinement.
    pub edges: Vec<String>,
    pub subdivisions: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meshing: Option<MeshingSettings>,
}
pub struct MeshedPolygon {
    pub mesh: Mesh,
    pub boundaries: BTreeMap<String, Vec<usize>>,
}
fn cross(a: DVec2, b: DVec2, c: DVec2) -> f64 {
    (b - a).perp_dot(c - a)
}
fn intersects(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> bool {
    let eps = 1e-12;
    if a.min(b).x > c.max(d).x + eps
        || c.min(d).x > a.max(b).x + eps
        || a.min(b).y > c.max(d).y + eps
        || c.min(d).y > a.max(b).y + eps
    {
        return false;
    }
    cross(a, b, c) * cross(a, b, d) <= eps * eps && cross(c, d, a) * cross(c, d, b) <= eps * eps
}
impl PolygonPattern {
    pub fn meshing_settings(&self) -> MeshingSettings {
        self.meshing.clone().unwrap_or_else(|| MeshingSettings {
            target_edge_m: 0.1 / 2.0_f64.powi(self.subdivisions as i32),
            ..Default::default()
        })
    }
    pub fn build(&self, m: &Material) -> Result<MeshedPolygon, String> {
        m.validate()?;
        let n = self.points.len();
        if !(3..=64).contains(&n)
            || self.edges.len() != n
            || self.subdivisions > 4
            || self.points.iter().any(|p| !p.is_finite())
        {
            return Err(
                "Polygon requires 3..64 points, one edge name per point and 0..4 subdivisions"
                    .into(),
            );
        }
        let mut names = BTreeSet::new();
        for name in &self.edges {
            if name.is_empty()
                || name.len() > 80
                || !name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
                || !names.insert(name)
            {
                return Err("Invalid/duplicate polygon edge name".into());
            }
        }
        let mut area = 0.0;
        for i in 0..n {
            let (a, b) = (self.points[i], self.points[(i + 1) % n]);
            area += a.perp_dot(b) * 0.5;
            if a.distance(b) < 1e-6 {
                return Err("Zero-length polygon edge".into());
            }
            for j in i + 1..n {
                if j == i + 1 || (i == 0 && j == n - 1) {
                    continue;
                }
                if intersects(a, b, self.points[j], self.points[(j + 1) % n]) {
                    return Err("Self-intersecting polygon".into());
                }
            }
        }
        if area <= 1e-8 {
            return Err("Polygon must have positive signed area in (u,v)".into());
        }
        let settings = self.meshing_settings();
        let meshing::TriangulatedContour {
            uv,
            faces,
            boundaries,
        } = meshing::triangulate(&self.points, &self.edges, &settings)?;
        let count = uv.len();
        let mut mesh = Mesh {
            uv,
            triangles: vec![],
            hinges: vec![],
            rest_area: vec![0.0; count],
            masses: vec![0.0; count],
            pins: vec![None; count],
        };
        let mut edges = BTreeMap::<(usize, usize), (usize, f64)>::new();
        for vertices in faces {
            let rest = vertices.map(|i| mesh.uv[i]);
            let dm = DMat2::from_cols(rest[1] - rest[0], rest[2] - rest[0]);
            let area = dm.determinant() * 0.5;
            if area <= 1e-14 {
                return Err("Degenerate polygon triangle".into());
            }
            for i in vertices {
                mesh.rest_area[i] += area / 3.0;
            }
            let [a, b, c] = vertices;
            for (a, b, c) in [(a, b, c), (b, c, a), (c, a, b)] {
                let key = (a.min(b), a.max(b));
                if let Some((d, other_area)) = edges.remove(&key) {
                    let e = mesh.uv[b] - mesh.uv[a];
                    mesh.hinges.push(Hinge {
                        vertices: [a, b, c, d],
                        weight: 3.0 * e.length_squared() / (area + other_area),
                        across: DVec2::new(-e.y, e.x).normalize(),
                        material: 0,
                    });
                } else {
                    edges.insert(key, (c, area));
                }
            }
            mesh.triangles.push(Triangle {
                vertices,
                rest,
                inv_dm: dm.inverse(),
                area,
                material: 0,
            });
        }
        for i in 0..count {
            mesh.masses[i] = mesh.rest_area[i] * m.density;
        }
        Ok(MeshedPolygon { mesh, boundaries })
    }
}

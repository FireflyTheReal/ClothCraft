//! Named material contours, holes and internal sewing paths.
use super::meshing::{self, MeshingSettings};
use crate::model::{Hinge, Material, Mesh, Triangle};
use glam::{DMat2, DVec2};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Upper bound for a stored notch marker depth, in metres. Notches are draft
/// markings, so this is a sanity bound, not a manufacturing specification.
pub const MAX_NOTCH_DEPTH_M: f64 = 0.2;
/// Upper bound for a stored seam-allowance width, in metres. The cutting contour
/// produced from it must still fit inside the panel; see `export`.
pub const MAX_SEAM_ALLOWANCE_M: f64 = 0.1;
/// Draft budget: notches on one named boundary and on one polygon.
pub const MAX_NOTCHES_PER_BOUNDARY: usize = 64;
pub const MAX_NOTCHES_PER_POLYGON: usize = 256;

/// One physical notch (cut mark) on a named boundary.
///
/// The position is a fraction of the *directed* boundary's sampled rest
/// arclength, so it is defined by the material contour alone. It is not a
/// simulation vertex index and never changes rest geometry.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EdgeNotch {
    /// Fraction of the directed boundary's rest arclength, strictly inside (0,1).
    pub t: f64,
    /// Marker depth measured outward from the material, in metres (> 0).
    pub depth_m: f64,
}

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
    /// Two cubic control points per named edge; absent entries are straight.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub curves: BTreeMap<String, [DVec2; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<super::domain::Hole>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<super::domain::InternalLine>,
    /// Required length of each named boundary, keyed by the stable boundary
    /// identifier, in metres. A target is a 2D specification to compare against;
    /// it is never enforced while loading a document and never touches the 3D
    /// side. Empty maps serialise to nothing, so historical files are unchanged.
    #[serde(
        default,
        deserialize_with = "deserialize_target_lengths",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub target_lengths: BTreeMap<String, f64>,
    /// Physical cut marks keyed by the stable named boundary, ordered by
    /// arclength fraction. Empty maps serialise to nothing, so historical files
    /// and physical cache signatures are unchanged when no marking is stored.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub notches: BTreeMap<String, Vec<EdgeNotch>>,
    /// Seam-allowance (cutting-guide) width in metres per named boundary. A
    /// missing entry means no allowance. This is a draft specification; it never
    /// moves the sewing contour or the simulation mesh.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub seam_allowances: BTreeMap<String, f64>,
}
/// Tolerant reader for stored targets. A hand-edited file can hold `null` (what a
/// non-finite float serialises to) or another unusable value. Those become `NaN`
/// so `target_length` ignores them and `target_diagnostics` reports them, instead
/// of failing the whole document load.
fn deserialize_target_lengths<'de, D>(deserializer: D) -> Result<BTreeMap<String, f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    #[allow(dead_code)]
    enum Raw {
        Number(f64),
        Text(String),
        Bool(bool),
        List(Vec<Raw>),
        Map(BTreeMap<String, Raw>),
        Null,
    }
    Ok(BTreeMap::<String, Raw>::deserialize(deserializer)?
        .into_iter()
        .map(|(edge, raw)| {
            (
                edge,
                match raw {
                    Raw::Number(value) => value,
                    _ => f64::NAN,
                },
            )
        })
        .collect())
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
    pub fn edge_points(&self, i: usize) -> Result<Vec<DVec2>, String> {
        let a = self.points[i];
        let b = self.points[(i + 1) % self.points.len()];
        if let Some([c, d]) = self.curves.get(&self.edges[i]) {
            super::curves::flatten([a, *c, *d, b], super::curves::FLATNESS_M)
        } else {
            Ok(vec![a, b])
        }
    }
    pub fn meshing_settings(&self) -> MeshingSettings {
        self.meshing.clone().unwrap_or_else(|| MeshingSettings {
            target_edge_m: 0.1 / 2.0_f64.powi(self.subdivisions as i32),
            ..Default::default()
        })
    }
    /// Sampled path of a named outer boundary and its total rest arclength.
    /// Markings are restricted to outer boundaries; hole edges are separate.
    pub fn boundary_path(&self, edge: &str) -> Result<(Vec<DVec2>, f64), String> {
        let index = self
            .edges
            .iter()
            .position(|name| name == edge)
            .ok_or("Boundary not found")?;
        let path = self.edge_points(index)?;
        let total = path.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
        Ok((path, total))
    }
    /// The stored notch as a material-space segment `[on_boundary, tip]`. The
    /// marker extends outward from the material, i.e. to the right of the CCW
    /// outer boundary. Position depends only on the material contour.
    pub fn notch_segment(&self, edge: &str, notch: &EdgeNotch) -> Result<[DVec2; 2], String> {
        if !notch.t.is_finite() || notch.t <= 0. || notch.t >= 1. {
            return Err("Notch fraction must be strictly inside (0,1)".into());
        }
        if !notch.depth_m.is_finite() || notch.depth_m <= 0. || notch.depth_m > MAX_NOTCH_DEPTH_M {
            return Err(format!("Notch depth must be in (0, {MAX_NOTCH_DEPTH_M}] m"));
        }
        let (path, total) = self.boundary_path(edge)?;
        if total <= 1e-9 {
            return Err("Notch boundary has zero length".into());
        }
        let target = total * notch.t;
        let mut consumed = 0.;
        for pair in path.windows(2) {
            let tangent = pair[1] - pair[0];
            let length = tangent.length();
            if length <= 1e-12 {
                continue;
            }
            if consumed + length >= target {
                let base = pair[0] + tangent * ((target - consumed) / length);
                let normal = DVec2::new(tangent.y, -tangent.x) / length;
                return Ok([base, base + normal * notch.depth_m]);
            }
            consumed += length;
        }
        Err("Notch cannot be placed on this boundary".into())
    }
    /// Every stored notch marker for one named boundary, in arclength order.
    pub fn notch_segments(&self, edge: &str) -> Result<Vec<[DVec2; 2]>, String> {
        match self.notches.get(edge) {
            Some(list) => list.iter().map(|n| self.notch_segment(edge, n)).collect(),
            None => Ok(vec![]),
        }
    }
    /// Outward seam-allowance cutting contour with joined corners and
    /// self-intersection cleanup; `None` when no allowance is stored.
    /// See [`super::shapes::cutting_outline`] for the ring convention.
    pub fn cutting_outline(&self) -> Result<Option<Vec<Vec<DVec2>>>, String> {
        super::shapes::cutting_outline(self)
    }
    /// Rebase stored markings when the named boundary is split into itself and
    /// `second` at rest-arclength fraction `at_t`. A notch keeps its material
    /// position; a notch exactly on the split is rejected instead of moved.
    pub fn split_markings(&mut self, edge: &str, second: &str, at_t: f64) -> Result<(), String> {
        if !at_t.is_finite() || at_t <= 0. || at_t >= 1. {
            return Err("Split fraction must be strictly inside (0,1)".into());
        }
        if let Some(list) = self.notches.remove(edge) {
            let (mut first, mut rest) = (vec![], vec![]);
            for mut notch in list {
                if (notch.t - at_t).abs() < 1e-9 {
                    return Err("A notch sits exactly on the split point".into());
                }
                if notch.t < at_t {
                    notch.t /= at_t;
                    first.push(notch);
                } else {
                    notch.t = (notch.t - at_t) / (1. - at_t);
                    rest.push(notch);
                }
            }
            if !first.is_empty() {
                self.notches.insert(edge.to_string(), first);
            }
            if !rest.is_empty() {
                self.notches.insert(second.to_string(), rest);
            }
        }
        if let Some(width) = self.seam_allowances.get(edge).copied() {
            self.seam_allowances.insert(second.to_string(), width);
        }
        Ok(())
    }
    /// Rebase stored markings when the straight boundary `outgoing` disappears
    /// into `incoming` by removing their shared point. The caller guarantees
    /// both boundaries are straight.
    pub fn merge_markings(
        &mut self,
        incoming: &str,
        outgoing: &str,
        incoming_m: f64,
        outgoing_m: f64,
    ) -> Result<(), String> {
        let total = incoming_m + outgoing_m;
        if !total.is_finite() || total <= 1e-9 {
            return Err("Merged boundary has zero length".into());
        }
        let mut merged = vec![];
        if let Some(list) = self.notches.remove(incoming) {
            for mut notch in list {
                notch.t *= incoming_m / total;
                merged.push(notch);
            }
        }
        if let Some(list) = self.notches.remove(outgoing) {
            for mut notch in list {
                notch.t = (incoming_m + notch.t * outgoing_m) / total;
                merged.push(notch);
            }
        }
        merged.sort_by(|a, b| a.t.total_cmp(&b.t));
        if merged.windows(2).any(|w| w[1].t - w[0].t < 1e-9) {
            return Err("Notches coincide after removing this point".into());
        }
        if !merged.is_empty() {
            self.notches.insert(incoming.to_string(), merged);
        }
        let a = self.seam_allowances.get(incoming).copied();
        let b = self.seam_allowances.remove(outgoing);
        match (a, b) {
            (Some(x), Some(y)) if (x - y).abs() > 1e-12 => {
                Err("The merged boundaries have different seam allowances; clear one first".into())
            }
            (Some(x), _) => {
                self.seam_allowances.insert(incoming.to_string(), x);
                Ok(())
            }
            (None, Some(y)) => {
                self.seam_allowances.insert(incoming.to_string(), y);
                Ok(())
            }
            (None, None) => Ok(()),
        }
    }
    /// Stored target for a named boundary, when it is a usable specification.
    /// Unknown boundaries, non-finite values and non-positive values are treated
    /// as "no target", so a hand-edited file cannot leak a bad number into a
    /// readout or into geometry.
    pub fn target_length(&self, edge: &str) -> Option<f64> {
        if !self.edges.iter().any(|name| name == edge) {
            return None;
        }
        self.target_lengths
            .get(edge)
            .copied()
            .filter(|value| value.is_finite() && *value > 0.)
    }
    /// Reports every stored target that is not a usable specification: an
    /// unknown boundary, a non-finite value or a non-positive value. Readouts
    /// treat those entries as absent instead of trusting them.
    pub fn target_diagnostics(&self) -> Vec<String> {
        self.target_lengths
            .iter()
            .filter(|(edge, value)| {
                !self.edges.iter().any(|name| name == *edge) || !value.is_finite() || **value <= 0.
            })
            .map(|(edge, value)| format!("Ignored target length for boundary {edge}: {value}"))
            .collect()
    }
    pub fn build(&self, m: &Material) -> Result<MeshedPolygon, String> {
        m.validate()?;
        let n = self.points.len();
        if !(3..=4096).contains(&n)
            || self.edges.len() != n
            || self.subdivisions > 4
            || self.points.iter().any(|p| !p.is_finite())
        {
            return Err(
                "Polygon requires 3..4096 points, one edge name per point and 0..4 subdivisions"
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
        if self
            .curves
            .iter()
            .any(|(name, points)| !names.contains(name) || points.iter().any(|p| !p.is_finite()))
        {
            return Err("Invalid Bézier edge name or control point".into());
        }
        // Draft markings are defined on stable named outer boundaries only. They
        // are validated here so a hand-edited document with an unknown boundary,
        // an out-of-range fraction or an absurd depth is rejected, never meshed.
        for (edge, list) in &self.notches {
            if !names.contains(edge) {
                return Err("Notch references an unknown boundary".into());
            }
            if list.len() > MAX_NOTCHES_PER_BOUNDARY {
                return Err(format!(
                    "At most {MAX_NOTCHES_PER_BOUNDARY} notches per boundary"
                ));
            }
            let mut previous = f64::NEG_INFINITY;
            for notch in list {
                if notch.t <= previous {
                    return Err(
                        "Notches on one boundary must have strictly increasing fractions".into(),
                    );
                }
                previous = notch.t;
                self.notch_segment(edge, notch)?;
            }
        }
        if self.notches.values().map(Vec::len).sum::<usize>() > MAX_NOTCHES_PER_POLYGON {
            return Err(format!(
                "At most {MAX_NOTCHES_PER_POLYGON} notches per pattern"
            ));
        }
        for (edge, width) in &self.seam_allowances {
            if !names.contains(edge)
                || !width.is_finite()
                || *width <= 0.
                || *width > MAX_SEAM_ALLOWANCE_M
            {
                return Err(format!(
                    "Seam allowance needs a known boundary and a width in (0, {MAX_SEAM_ALLOWANCE_M}] m"
                ));
            }
        }
        let mut points = vec![];
        let mut segments = vec![];
        let mut owners = vec![];
        for i in 0..n {
            let samples = self.edge_points(i)?;
            for (j, p) in samples[..samples.len() - 1].iter().enumerate() {
                points.push(*p);
                segments.push(format!("@part_{i}_{j}"));
                owners.push(i);
            }
            if points.len() > 4096 {
                return Err("Contour subdivision budget exceeded".into());
            }
        }
        let n = points.len();
        let mut area = 0.0;
        for i in 0..n {
            let (a, b) = (points[i], points[(i + 1) % n]);
            area += a.perp_dot(b) * 0.5;
            if a.distance(b) < 1e-6 {
                return Err("Zero-length polygon edge".into());
            }
            for j in i + 1..n {
                if j == i + 1 || (i == 0 && j == n - 1) {
                    continue;
                }
                if intersects(a, b, points[j], points[(j + 1) % n]) {
                    return Err("Self-intersecting polygon".into());
                }
            }
        }
        if area <= 1e-8 {
            return Err("Polygon must have positive signed area in (u,v)".into());
        }
        super::domain::validate(self, &points)?;
        let settings = self.meshing_settings();
        let meshing::TriangulatedContour {
            uv,
            faces,
            boundaries: parts,
        } = if self.holes.is_empty() && self.lines.is_empty() {
            meshing::triangulate(&points, &segments, &settings)?
        } else {
            meshing::triangulate_domain(&points, &segments, &self.holes, &self.lines, &settings)?
        };
        let mut boundaries = BTreeMap::<String, Vec<usize>>::new();
        for (part, owner) in segments.iter().zip(owners) {
            let chain = boundaries.entry(self.edges[owner].clone()).or_default();
            let ids = &parts[part];
            if chain.is_empty() {
                chain.extend_from_slice(ids);
            } else {
                if chain.last() != ids.first() {
                    return Err("Disconnected curved boundary".into());
                }
                chain.extend_from_slice(&ids[1..]);
            }
        }
        for hole in &self.holes {
            for name in &hole.edges {
                boundaries.insert(name.clone(), parts[name].clone());
            }
        }
        for line in &self.lines {
            boundaries.insert(line.id.clone(), parts[&line.id].clone());
        }
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

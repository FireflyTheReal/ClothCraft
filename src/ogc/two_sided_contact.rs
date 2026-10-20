//! Experimental two-sided local-minimum contact potential.
//! Uses GCP Appendix A/D factors, but omits the oriented-volume exterior
//! test deliberately. This is NOT the published GCP model or a cloth backend.
//! Pair weights are explicit fixed dimensionless quadrature coefficients.
//! All incident geometry participates in derivatives; no frozen normals.
use super::{
    feature_hierarchy::Pair, feature_mollifier, potential::activation, primitive_distance,
    topology::Topology,
};
use crate::math::{Scalar, cross, dot, sub};

#[derive(Clone, Copy)]
pub struct Settings {
    pub radius: f64,
    pub stiffness: f64,
    pub width: f64,
    pub alpha: f64,
}
impl Settings {
    fn validate(self) -> Result<(), String> {
        if [self.radius, self.stiffness, self.width, self.alpha]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.)
            || self.alpha > 1.
        {
            return Err("Invalid two-sided contact parameters".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub enum Feature {
    Vertex(usize),
    Edge(usize),
    Face,
}
pub struct MinimumContact<'a> {
    pub topology: &'a Topology,
    vertex_faces: Vec<Vec<usize>>,
}
fn mul<S: Scalar>(v: [S; 3], s: S) -> [S; 3] {
    v.map(|a| a * s)
}
fn add<S: Scalar>(a: [S; 3], b: [S; 3]) -> [S; 3] {
    std::array::from_fn(|i| a[i] + b[i])
}
fn unit<S: Scalar>(v: [S; 3]) -> Result<[S; 3], String> {
    let n = dot(v, v);
    if !n.value().is_finite() || n.value() <= 0. {
        return Err("Undefined two-sided contact direction".into());
    }
    Ok(mul(v, S::c(1.) / n.sqrt()))
}
/// GCP Appendix C cumulative cubic, scaled to [-alpha, 0].
pub fn smooth_minimum<S: Scalar>(phi: S, alpha: f64) -> S {
    let z = phi * S::c(3. / alpha);
    if z.value() <= -3. {
        S::c(0.)
    } else if z.value() < -2. {
        let a = z + S::c(3.);
        a * a * a / S::c(6.)
    } else if z.value() < -1. {
        (S::c(3.) - S::c(9.) * z - S::c(9.) * z * z - S::c(2.) * z * z * z) / S::c(6.)
    } else if z.value() < 0. {
        S::c(1.) + z * z * z / S::c(6.)
    } else {
        S::c(1.)
    }
}
impl<'a> MinimumContact<'a> {
    pub fn new(topology: &'a Topology) -> Self {
        let mut vertex_faces = vec![Vec::new(); topology.neighbors.len()];
        for (f, t) in topology.triangles.iter().enumerate() {
            for &v in t {
                vertex_faces[v].push(f);
            }
        }
        Self {
            topology,
            vertex_faces,
        }
    }
    /// `away` points from the other closest point toward this feature.
    /// Exposed for analytic and derivative tests, not geometric culling.
    pub fn minimum_factor<S: Scalar>(
        &self,
        feature: Feature,
        x: &[[S; 3]],
        away: [S; 3],
        alpha: f64,
    ) -> Result<S, String> {
        if !alpha.is_finite() || alpha <= 0. || alpha > 1. {
            return Err("Invalid minimum-factor alpha".into());
        }
        let direction = unit(away)?;
        let mut g = S::c(1.);
        match feature {
            Feature::Face => {}
            Feature::Edge(e) => {
                let [a, b] = self.topology.edges[e];
                let tangent = unit(sub(x[b], x[a]))?;
                for &o in &self.topology.opposites[e] {
                    let ray = sub(x[o], x[a]);
                    let inward = unit(sub(ray, mul(tangent, dot(ray, tangent))))?;
                    g = g * smooth_minimum(dot(inward, direction), alpha);
                }
            }
            Feature::Vertex(v) => {
                for &f in &self.vertex_faces[v] {
                    let t = self.topology.triangles[f];
                    let k = t.iter().position(|i| *i == v).unwrap();
                    let a = unit(sub(x[t[(k + 1) % 3]], x[v]))?;
                    let b = unit(sub(x[t[(k + 2) % 3]], x[v]))?;
                    let bisector = unit(add(a, b))?;
                    for ray in [a, b, bisector] {
                        g = g * smooth_minimum(dot(ray, direction), alpha);
                    }
                }
            }
        }
        Ok(g)
    }
    /// Caller supplies the full candidate halo and excludes seams/adjacency.
    /// A pair stencil includes BOTH one-rings, not only Pair::vertices.
    /// Fixed `weight` is dimensionless; `stiffness` is N/m, output joules.
    pub fn energy<S: Scalar>(
        &self,
        pair: Pair,
        x: &[[S; 3]],
        weight: f64,
        cfg: Settings,
    ) -> Result<S, String> {
        cfg.validate()?;
        if !weight.is_finite() || weight < 0. || x.len() != self.vertex_faces.len() {
            return Err("Invalid two-sided contact geometry/weight".into());
        }
        if weight == 0. {
            return Ok(S::c(0.));
        }
        let top = self.topology;
        let (m, distance) = match pair {
            Pair::VertexFace([v, f]) => {
                let [a, b, c] = top.triangles[f];
                let p = [x[v], x[a], x[b], x[c]];
                (
                    feature_mollifier::vertex_face(p, cfg.width)?,
                    primitive_distance::point_triangle(p),
                )
            }
            Pair::EdgeEdge([e, f]) => {
                let [a, b] = top.edges[e];
                let [c, d] = top.edges[f];
                let p = [x[a], x[b], x[c], x[d]];
                (
                    feature_mollifier::edge_edge(p, cfg.width)?,
                    primitive_distance::segment_segment(p),
                )
            }
            Pair::VertexEdge([v, e]) => {
                let [a, b] = top.edges[e];
                (
                    feature_mollifier::vertex_edge([x[v], x[a], x[b]], cfg.width)?,
                    primitive_distance::point_segment(x[v], x[a], x[b]),
                )
            }
            Pair::VertexVertex([a, b]) => {
                let r = sub(x[a], x[b]);
                (S::c(1.), dot(r, r).sqrt())
            }
        };
        if !distance.value().is_finite() || distance.value() <= 0. {
            return Err("Two-sided contact requires strictly separated geometry".into());
        }
        if m.value() == 0. || distance.value() >= cfg.radius {
            return Ok(S::c(0.));
        }
        // Positive mollifiers require interior closest points. Compute them
        // with AD only now, avoiding undefined parallel/endpoint derivatives.
        let (a, b, away) = match pair {
            Pair::VertexFace([v, f]) => {
                let [i, j, k] = top.triangles[f];
                let n = cross(sub(x[j], x[i]), sub(x[k], x[i]));
                let r = mul(n, dot(sub(x[v], x[i]), n) / dot(n, n));
                (Feature::Vertex(v), Feature::Face, r)
            }
            Pair::VertexEdge([v, e]) => {
                let [i, j] = top.edges[e];
                let edge = sub(x[j], x[i]);
                let r = sub(x[v], x[i]);
                (
                    Feature::Vertex(v),
                    Feature::Edge(e),
                    sub(r, mul(edge, dot(r, edge) / dot(edge, edge))),
                )
            }
            Pair::EdgeEdge([e, f]) => {
                let [i, j] = top.edges[e];
                let [k, l] = top.edges[f];
                let n = cross(sub(x[j], x[i]), sub(x[l], x[k]));
                let nn = dot(n, n);
                if nn.value() <= 0. {
                    return Err("Nonzero parallel EE feature factor".into());
                }
                let r = mul(n, dot(sub(x[i], x[k]), n) / nn);
                (Feature::Edge(e), Feature::Edge(f), r)
            }
            Pair::VertexVertex([a, b]) => (Feature::Vertex(a), Feature::Vertex(b), sub(x[a], x[b])),
        };
        let ga = self.minimum_factor(a, x, away, cfg.alpha)?;
        let gb = self.minimum_factor(b, x, mul(away, S::c(-1.)), cfg.alpha)?;
        Ok(S::c(weight) * m * ga * gb * activation(distance, cfg.radius, cfg.stiffness))
    }
}

//! Independent nonlinear orthotropic membrane and flat-rest hinge energies.
//! Not an implementation of Kim's exact BW or Stable Discrete Bending models.
use crate::{
    math::{Jet, Scalar, cross, dot, sub},
    model::{Hinge, Material, Triangle},
};
use glam::DVec3;

/// Smooth tension/compression blend; p(0)=0, asymptotic positive part.
fn axial<S: Scalar>(strain: S, stiffness: f64, m: &Material) -> S {
    let delta = S::c(m.compression_transition);
    let p = (strain + (strain * strain + delta * delta).sqrt() - delta) * S::c(0.5);
    S::c(0.5 * stiffness)
        * (S::c(m.compression_ratio) * strain * strain + S::c(1.0 - m.compression_ratio) * p * p)
}

pub fn membrane<S: Scalar>(x: [[S; 3]; 3], t: &Triangle, m: &Material) -> S {
    let ds = [sub(x[1], x[0]), sub(x[2], x[0])];
    let f: [[S; 3]; 2] = [t.inv_dm.x_axis, t.inv_dm.y_axis]
        .map(|c| std::array::from_fn(|k| ds[0][k] * S::c(c.x) + ds[1][k] * S::c(c.y)));
    let (s, c) = m.grain_degrees.to_radians().sin_cos();
    let w = std::array::from_fn(|i| f[0][i] * S::c(c) + f[1][i] * S::c(s));
    let v = std::array::from_fn(|i| f[0][i] * S::c(-s) + f[1][i] * S::c(c));
    let ew = (dot(w, w) - S::c(1.0)) * S::c(0.5);
    let ev = (dot(v, v) - S::c(1.0)) * S::c(0.5);
    let shear = dot(w, v);
    S::c(t.area)
        * (axial(ew, m.warp, m) + axial(ev, m.weft, m) + S::c(0.5 * m.shear) * shear * shear)
}

pub fn bending<S: Scalar>(x: [[S; 3]; 4], h: &Hinge, m: &Material) -> S {
    let edge = sub(x[1], x[0]);
    let n1 = cross(edge, sub(x[2], x[0]));
    let n2 = cross(sub(x[3], x[0]), edge);
    let length1 = dot(n1, n1).sqrt();
    let length2 = dot(n2, n2).sqrt();
    // 0.5*|unit(n1)-unit(n2)|² equals 1-cos(theta), but avoids cancellation
    // (and tiny negative energy) for nearly flat hinges. No derivative clipping.
    let difference = std::array::from_fn(|i| n1[i] / length1 - n2[i] / length2);
    let (s, c) = m.grain_degrees.to_radians().sin_cos();
    let aw = h.across.x * c + h.across.y * s;
    let af = -h.across.x * s + h.across.y * c;
    // Directional interpolation is a baseline, not a calibrated orthotropic shell tensor.
    S::c(0.5 * h.weight * (m.bend_warp * aw * aw + m.bend_weft * af * af))
        * dot(difference, difference)
}

#[derive(Clone)]
pub struct Element {
    pub dofs: Vec<usize>,
    pub energy: f64,
    pub gradient: Vec<f64>,
    pub hessian: Vec<f64>,
}
impl Element {
    pub fn from_jet<const N: usize>(vertices: &[usize], jet: Jet<N>) -> Self {
        Self {
            dofs: vertices
                .iter()
                .flat_map(|v| (0..3).map(move |k| 3 * v + k))
                .collect(),
            energy: jet.v,
            gradient: jet.g.to_vec(),
            hessian: jet.h.into_iter().flatten().collect(),
        }
    }
}
pub fn membrane_element(x: &[DVec3], t: &Triangle, m: &Material) -> Element {
    let p = std::array::from_fn(|i| {
        std::array::from_fn(|k| Jet::<9>::variable(x[t.vertices[i]][k], 3 * i + k))
    });
    Element::from_jet(&t.vertices, membrane(p, t, m))
}
pub fn bending_element(x: &[DVec3], h: &Hinge, m: &Material) -> Element {
    let p = std::array::from_fn(|i| {
        std::array::from_fn(|k| Jet::<12>::variable(x[h.vertices[i]][k], 3 * i + k))
    });
    Element::from_jet(&h.vertices, bending(p, h, m))
}
pub fn stretches(x: &[DVec3], t: &Triangle, m: &Material) -> [f64; 2] {
    fiber_frame(x, t, m).stretch
}

/// World-space continuum fiber directions, not explicit yarn geometry.
#[derive(Clone, Debug, serde::Serialize)]
pub struct FiberFrame {
    pub warp: DVec3,
    pub weft: DVec3,
    pub stretch: [f64; 2],
    pub shear_cosine: f64,
    pub valid: bool,
}
pub fn fiber_frame(x: &[DVec3], t: &Triangle, m: &Material) -> FiberFrame {
    let ds = [
        x[t.vertices[1]] - x[t.vertices[0]],
        x[t.vertices[2]] - x[t.vertices[0]],
    ];
    let f = [t.inv_dm.x_axis, t.inv_dm.y_axis].map(|c| ds[0] * c.x + ds[1] * c.y);
    let (s, c) = m.grain_degrees.to_radians().sin_cos();
    let w = f[0] * c + f[1] * s;
    let v = -f[0] * s + f[1] * c;
    let stretch = [w.length(), v.length()];
    let valid = stretch.iter().all(|v| v.is_finite() && *v > 1e-12)
        && w.cross(v).length() > 1e-10 * stretch[0] * stretch[1];
    let warp = w.normalize_or_zero();
    let weft = v.normalize_or_zero();
    FiberFrame {
        warp,
        weft,
        stretch,
        shear_cosine: warp.dot(weft),
        valid,
    }
}

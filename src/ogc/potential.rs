use super::geometry::{edge_edge_distance, point_edge_distance, point_face_distance};
use crate::math::{Jet, Scalar, cross, dot, sub};
use glam::{DMat3, DVec3};

/// Eq. 18 with tau=r/2. The printed eq. 19 has a squared (tau-r), which does
/// NOT match derivatives. C1 requires k'=k*tau*(r-tau); C2 then follows at r/2.
/// k is N/m per pair (not a pressure); energy J, first derivative N.
pub fn activation<S: Scalar>(d: S, r: f64, k: f64) -> S {
    if d.value() >= r {
        return S::c(0.0);
    }
    if d.value() <= 0.0 {
        return S::c(f64::INFINITY);
    }
    let tau = 0.5 * r;
    if d.value() >= tau {
        let g = S::c(r) - d;
        S::c(0.5 * k) * g * g
    } else {
        S::c(0.5 * k * tau * tau) - S::c(k * tau * tau) * (d / S::c(tau)).ln()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    PointPoint,
    PointEdge,
    PointFace,
    EdgeEdge,
}
#[derive(Clone, Debug)]
pub struct Contact {
    pub kind: Kind,
    pub vertices: [usize; 4],
    pub count: usize,
    pub mollifier_epsilon: f64,
}
impl Contact {
    pub fn energy<S: Scalar>(&self, p: [[S; 3]; 4], r: f64, k: f64) -> S {
        let d = match self.kind {
            Kind::PointPoint => {
                let v = sub(p[0], p[1]);
                dot(v, v).sqrt()
            }
            Kind::PointEdge => point_edge_distance(p[0], p[1], p[2]),
            Kind::PointFace => point_face_distance(p[0], p[1], p[2], p[3]),
            Kind::EdgeEdge => {
                let n = cross(sub(p[1], p[0]), sub(p[3], p[2]));
                // Exactly parallel edges have zero mollified EE contribution.
                if dot(n, n).value() < 1e-30 {
                    return S::c(0.0);
                }
                edge_edge_distance(p[0], p[1], p[2], p[3])
            }
        };
        let mut e = activation(d, r, k);
        if self.kind == Kind::EdgeEdge {
            let n = cross(sub(p[1], p[0]), sub(p[3], p[2]));
            let q = dot(n, n) / S::c(self.mollifier_epsilon);
            if q.value() < 1.0 {
                e = e * q * (S::c(2.0) - q);
            }
        }
        e
    }
    pub fn value(&self, x: &[DVec3], r: f64, k: f64) -> f64 {
        self.energy(self.vertices.map(|i| x[i].to_array()), r, k)
    }
    pub fn block(&self, x: &[DVec3], vertex: usize, r: f64, k: f64) -> (f64, DVec3, DMat3) {
        let p = self.vertices.map(|i| {
            std::array::from_fn(|a| {
                if i == vertex {
                    Jet::<3>::variable(x[i][a], a)
                } else {
                    Jet::c(x[i][a])
                }
            })
        });
        let j = self.energy(p, r, k);
        (
            j.v,
            DVec3::from_array(j.g),
            DMat3::from_cols_array(&std::array::from_fn::<_, 9, _>(|i| j.h[i % 3][i / 3])),
        )
    }
}

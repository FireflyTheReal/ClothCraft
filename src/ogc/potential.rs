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
    /// CPU research variant: compact, C2 interior participation. Not OGC's law.
    EdgeEdgeInterior,
    /// CPU research: closest distance to the finite triangle, no cone filter.
    PointTriangle,
    /// CPU research: finite segments, with the existing parallel mollifier.
    SegmentSegment,
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
        if matches!(self.kind, Kind::PointTriangle | Kind::SegmentSegment) {
            let values = p.map(|v| v.map(|s| s.value()));
            let d = if self.kind == Kind::PointTriangle {
                super::primitive_distance::point_triangle(values)
            } else {
                super::primitive_distance::segment_segment(values)
            };
            if d >= r {
                return S::c(0.);
            }
        }
        if self.kind == Kind::EdgeEdgeInterior
            && !has_interior_closest_point(p.map(|v| DVec3::from_array(v.map(|s| s.value()))))
        {
            // The compact potential is identically zero here. Test values
            // before constructing any AD expressions for dormant pairs.
            return S::c(0.);
        }
        self.energy_core(p, r, k)
    }

    fn energy_core<S: Scalar>(&self, p: [[S; 3]; 4], r: f64, k: f64) -> S {
        let mut weight = S::c(1.0);
        let d = match self.kind {
            Kind::PointPoint => {
                let v = sub(p[0], p[1]);
                dot(v, v).sqrt()
            }
            Kind::PointEdge => point_edge_distance(p[0], p[1], p[2]),
            Kind::PointFace => point_face_distance(p[0], p[1], p[2], p[3]),
            Kind::PointTriangle => super::primitive_distance::point_triangle(p),
            Kind::EdgeEdge | Kind::EdgeEdgeInterior | Kind::SegmentSegment => {
                let n = cross(sub(p[1], p[0]), sub(p[3], p[2]));
                // Exactly parallel edges have zero mollified EE contribution.
                if dot(n, n).value() < 1e-30 {
                    return S::c(0.0);
                }
                if self.kind == Kind::EdgeEdgeInterior {
                    weight = interior_weight(p, r);
                    // Outside the support, avoid evaluating the infinite-line
                    // barrier (which can be singular although segments are apart).
                    if weight.value() <= 0.0 {
                        return S::c(0.0);
                    }
                }
                if self.kind == Kind::SegmentSegment {
                    super::primitive_distance::segment_segment(p)
                } else {
                    edge_edge_distance(p[0], p[1], p[2], p[3])
                }
            }
        };
        let mut e = activation(d, r, k);
        if self.kind == Kind::EdgeEdgeInterior {
            e = e * weight;
        }
        if matches!(
            self.kind,
            Kind::EdgeEdge | Kind::EdgeEdgeInterior | Kind::SegmentSegment
        ) {
            let n = cross(sub(p[1], p[0]), sub(p[3], p[2]));
            let q = dot(n, n) / S::c(self.mollifier_epsilon);
            if q.value() < 1.0 {
                e = e * q * (S::c(2.0) - q);
            }
        }
        e
    }
    pub fn value(&self, x: &[DVec3], r: f64, k: f64) -> f64 {
        let p = self.vertices.map(|i| x[i].to_array());
        if matches!(self.kind, Kind::PointTriangle | Kind::SegmentSegment) {
            // Scalar evaluation needs no AD support prepass. Classify once.
            self.energy_core(p, r, k)
        } else {
            self.energy(p, r, k)
        }
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

fn has_interior_closest_point(p: [DVec3; 4]) -> bool {
    let u = p[1] - p[0];
    let v = p[3] - p[2];
    let w = p[0] - p[2];
    let aa = u.length_squared();
    let ee = v.length_squared();
    let bb = u.dot(v);
    let cc = u.dot(w);
    let ff = v.dot(w);
    let den = u.cross(v).length_squared();
    let s = bb * ff - cc * ee;
    let t = aa * ff - bb * cc;
    den >= 1e-30 && s > 0. && s < den && t > 0. && t < den
}

/// Independent experimental regularization of the open EE block boundaries.
/// The four signed distances to endpoint planes are measured perpendicular to
/// the opposite line. Support reaches full weight one contact radius inside.
/// This changes the contact potential, including tangential derivatives. It
/// does not replace, widen, or otherwise modify OGC's separation bounds.
fn interior_weight<S: Scalar>(p: [[S; 3]; 4], r: f64) -> S {
    let u = sub(p[1], p[0]);
    let v = sub(p[3], p[2]);
    let w = sub(p[0], p[2]);
    let aa = dot(u, u);
    let bb = dot(u, v);
    let ee = dot(v, v);
    let cc = dot(u, w);
    let ff = dot(v, w);
    // Cross norm avoids cancellation for nearly parallel lines.
    let n = cross(u, v);
    let den = dot(n, n);
    let s = (bb * ff - cc * ee) / den;
    let t = (aa * ff - bb * cc) / den;
    let a = (den / ee).sqrt() / S::c(r);
    let b = (den / aa).sqrt() / S::c(r);
    ramp(s * a) * ramp((S::c(1.) - s) * a) * ramp(t * b) * ramp((S::c(1.) - t) * b)
}

fn ramp<S: Scalar>(x: S) -> S {
    if x.value() <= 0. {
        S::c(0.)
    } else if x.value() >= 1. {
        S::c(1.)
    } else {
        x * x * x * (S::c(10.) + x * (S::c(6.) * x - S::c(15.)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_support_shortcut_matches_full_ad_near_parallel_and_feature_boundaries() {
        for kind in [Kind::PointTriangle, Kind::SegmentSegment] {
            let c = Contact {
                kind,
                vertices: [0, 1, 2, 3],
                count: 4,
                mollifier_epsilon: 1.6e-10,
            };
            for slope in [0., 0.0001, 0.01, 0.1, 1.] {
                for j in 0..41 {
                    let gap = 0.0001 + 0.0001 * j as f64;
                    let values = if kind == Kind::PointTriangle {
                        [
                            [0.01, slope - 0.01, gap],
                            [0., 0., 0.],
                            [0.02, 0., 0.],
                            [0., 0.02, 0.],
                        ]
                    } else {
                        [
                            [0., 0., 0.],
                            [0.02, 0., 0.],
                            [0.005, -0.01 * slope, gap],
                            [0.025, 0.01 * slope, gap],
                        ]
                    };
                    for vertex in 0..4 {
                        let p = std::array::from_fn(|v| {
                            std::array::from_fn(|a| {
                                if v == vertex {
                                    Jet::<3>::variable(values[v][a], a)
                                } else {
                                    Jet::c(values[v][a])
                                }
                            })
                        });
                        let fast = c.energy(p, 0.003, 100_000.);
                        let full = c.energy_core(p, 0.003, 100_000.);
                        assert_eq!(fast.v, full.v);
                        assert_eq!(fast.g, full.g);
                        assert_eq!(fast.h, full.h);
                    }
                }
            }
        }
    }

    #[test]
    fn support_shortcut_matches_full_ad_including_gradients_and_hessians() {
        let c = Contact {
            kind: Kind::EdgeEdgeInterior,
            vertices: [0, 1, 2, 3],
            count: 4,
            mollifier_epsilon: 1.6e-10,
        };
        // Sweep both endpoint boundaries at several edge angles.
        for slope in [0., 0.01, 0.1, 1., 10.] {
            for i in 0..257 {
                let h = -0.005 + 0.03 * i as f64 / 256.;
                let x = [
                    [0., 0., 0.],
                    [0., 0.02, 0.],
                    [-0.01, h - 0.01 * slope, 0.0002],
                    [0.01, h + 0.01 * slope, 0.0002],
                ];
                for vertex in 0..4 {
                    let p = std::array::from_fn(|v| {
                        std::array::from_fn(|a| {
                            if v == vertex {
                                Jet::<3>::variable(x[v][a], a)
                            } else {
                                Jet::c(x[v][a])
                            }
                        })
                    });
                    let fast = c.energy(p, 0.003, 100_000.);
                    let full = c.energy_core(p, 0.003, 100_000.);
                    assert_eq!(fast.v, full.v);
                    assert_eq!(fast.g, full.g);
                    assert_eq!(fast.h, full.h);
                }
            }
        }
    }
}

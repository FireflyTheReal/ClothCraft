//! Scalar energy factors: U = sum(0.5 * k * C^2). Rest data stays material-space.
use crate::{
    garment::CompiledGarment,
    math::{Scalar, cross, dot, sub},
};
use glam::DVec3;

#[derive(Clone)]
pub(super) enum Kind {
    Membrane(usize, usize),
    Bend(usize),
    Linear(Vec<f64>, usize, f64),
}
#[derive(Clone)]
pub(super) struct Constraint {
    pub ids: Vec<usize>,
    pub k: f64,
    pub kind: Kind,
}
impl Constraint {
    pub fn value<S: Scalar>(&self, g: &CompiledGarment, p: &[[S; 3]]) -> S {
        match &self.kind {
            Kind::Membrane(ti, channel) => {
                let t = &g.mesh.triangles[*ti];
                let m = &g.materials[t.material];
                let ds = [sub(p[1], p[0]), sub(p[2], p[0])];
                let f: [[S; 3]; 2] = [t.inv_dm.x_axis, t.inv_dm.y_axis]
                    .map(|a| std::array::from_fn(|i| ds[0][i] * S::c(a.x) + ds[1][i] * S::c(a.y)));
                let (s, c) = m.grain_degrees.to_radians().sin_cos();
                let w = std::array::from_fn(|i| f[0][i] * S::c(c) + f[1][i] * S::c(s));
                let v = std::array::from_fn(|i| f[0][i] * S::c(-s) + f[1][i] * S::c(c));
                if *channel == 4 {
                    return dot(w, v);
                }
                let a = if *channel < 2 { w } else { v };
                let e = (dot(a, a) - S::c(1.)) * S::c(0.5);
                if channel % 2 == 0 {
                    e
                } else {
                    let d = S::c(m.compression_transition);
                    (e + (e * e + d * d).sqrt() - d) * S::c(0.5)
                }
            }
            Kind::Bend(axis) => {
                let edge = sub(p[1], p[0]);
                let a = cross(edge, sub(p[2], p[0]));
                let b = cross(sub(p[3], p[0]), edge);
                a[*axis] / dot(a, a).sqrt() - b[*axis] / dot(b, b).sqrt()
            }
            Kind::Linear(weights, axis, target) => weights
                .iter()
                .zip(p)
                .fold(S::c(-*target), |a, (w, x)| a + S::c(*w) * x[*axis]),
        }
    }
    pub fn at(&self, g: &CompiledGarment, x: &[DVec3]) -> f64 {
        let p: [[f64; 3]; 4] =
            std::array::from_fn(|a| x[self.ids[a.min(self.ids.len() - 1)]].to_array());
        self.value(g, &p)
    }
}
pub(super) fn compile(g: &CompiledGarment) -> Vec<Constraint> {
    let mut out = Vec::new();
    for (i, t) in g.mesh.triangles.iter().enumerate() {
        let m = &g.materials[t.material];
        for (ch, k) in [
            m.warp * m.compression_ratio,
            m.warp * (1. - m.compression_ratio),
            m.weft * m.compression_ratio,
            m.weft * (1. - m.compression_ratio),
            m.shear,
        ]
        .into_iter()
        .enumerate()
        {
            if k > 0. {
                out.push(Constraint {
                    ids: t.vertices.to_vec(),
                    k: k * t.area,
                    kind: Kind::Membrane(i, ch),
                });
            }
        }
    }
    for h in &g.mesh.hinges {
        let m = &g.materials[h.material];
        let (s, c) = m.grain_degrees.to_radians().sin_cos();
        let k = h.weight
            * (m.bend_warp * (h.across.x * c + h.across.y * s).powi(2)
                + m.bend_weft * (-h.across.x * s + h.across.y * c).powi(2));
        for axis in 0..3 {
            if k > 0. {
                out.push(Constraint {
                    ids: h.vertices.to_vec(),
                    k,
                    kind: Kind::Bend(axis),
                });
            }
        }
    }
    for seam in &g.seams {
        for sample in &seam.samples {
            for axis in 0..3 {
                out.push(Constraint {
                    ids: sample.point.terms.iter().map(|(i, _)| *i).collect(),
                    k: seam.stiffness * sample.weight_m,
                    kind: Kind::Linear(
                        sample.point.terms.iter().map(|(_, w)| *w).collect(),
                        axis,
                        0.,
                    ),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{energy, garment::GarmentTemplate, math::Jet};
    #[test]
    fn factorization_matches_existing_energy_and_derivatives_under_deformation() {
        let g = GarmentTemplate::two_panel_demo().compile().unwrap();
        let x: Vec<_> = g
            .initial_state
            .x
            .iter()
            .enumerate()
            .map(|(i, p)| {
                *p + DVec3::new(
                    0.01 * (i as f64).sin(),
                    0.003 * (i as f64).cos(),
                    0.004 * ((i * 7) as f64).sin(),
                )
            })
            .collect();
        let constraints = compile(&g);
        let expected = g
            .mesh
            .triangles
            .iter()
            .map(|t| {
                energy::membrane(
                    t.vertices.map(|i| x[i].to_array()),
                    t,
                    &g.materials[t.material],
                )
            })
            .sum::<f64>()
            + g.mesh
                .hinges
                .iter()
                .map(|h| {
                    energy::bending(
                        h.vertices.map(|i| x[i].to_array()),
                        h,
                        &g.materials[h.material],
                    )
                })
                .sum::<f64>()
            + g.seams.iter().map(|s| s.energy(&x)).sum::<f64>();
        let actual = constraints
            .iter()
            .map(|c| 0.5 * c.k * c.at(&g, &x).powi(2))
            .sum::<f64>();
        assert!((actual - expected).abs() < 1e-10 * expected.max(1.));
        let rotation = glam::DQuat::from_rotation_y(0.7) * glam::DQuat::from_rotation_z(-0.4);
        let transformed: Vec<_> = x
            .iter()
            .map(|p| rotation * *p + DVec3::new(1., 2., -3.))
            .collect();
        let rotated_energy = constraints
            .iter()
            .map(|c| 0.5 * c.k * c.at(&g, &transformed).powi(2))
            .sum::<f64>();
        assert!((rotated_energy - actual).abs() < 1e-10 * actual.max(1.));
        for c in constraints.iter().step_by(37) {
            let p: Vec<_> = c
                .ids
                .iter()
                .enumerate()
                .map(|(i, v)| std::array::from_fn(|a| Jet::<12>::variable(x[*v][a], 3 * i + a)))
                .collect();
            let j = c.value(&g, &p);
            let d: Vec<_> = c
                .ids
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    std::array::from_fn(|a| {
                        crate::solver_lab::dual::Dual::<12>::variable(x[*v][a], 3 * i + a)
                    })
                })
                .collect();
            let dual = c.value(&g, &d);
            for i in 0..c.ids.len() * 3 {
                assert!((dual.g[i] - j.g[i]).abs() < 1e-10);
                let mut plus = x.clone();
                let mut minus = x.clone();
                plus[c.ids[i / 3]][i % 3] += 1e-7;
                minus[c.ids[i / 3]][i % 3] -= 1e-7;
                let fd = (c.at(&g, &plus) - c.at(&g, &minus)) / 2e-7;
                assert!((fd - j.g[i]).abs() < 1e-5 * (1. + j.g[i].abs()));
            }
        }
    }
}

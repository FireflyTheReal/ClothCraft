//! Exact existing material/sewing factors, dense Gauss-Newton search metric.
//! Research assembly only. No contact potential, constitutive modification,
//! rest-shape update, diagonal shift or damping.
use super::{constraints, dual::Dual};
use crate::{garment::CompiledGarment, math::Scalar};
use glam::DVec3;

pub struct System {
    pub energy: f64,
    pub gradient: Vec<f64>,
    pub metric: Vec<f64>,
}
fn validate(
    g: &CompiledGarment,
    x: &[DVec3],
    target: &[DVec3],
    inertia: &[f64],
) -> Result<usize, String> {
    let n = x.len() * 3;
    if x.len() != g.mesh.masses.len()
        || target.len() != x.len()
        || inertia.len() != x.len()
        || n > 192
        || x.iter().chain(target).any(|p| !p.is_finite())
        || inertia.iter().any(|v| !v.is_finite() || *v <= 0.)
    {
        return Err("Invalid dense patch input (at most 64 vertices)".into());
    }
    Ok(n)
}
/// Scalar evaluation of exactly the same objective as `assemble`, for line
/// searches that do not need derivatives or a dense metric.
pub fn energy(
    g: &CompiledGarment,
    x: &[DVec3],
    target: &[DVec3],
    inertia: &[f64],
) -> Result<f64, String> {
    validate(g, x, target, inertia)?;
    let mut energy = 0.;
    for i in 0..x.len() {
        energy += 0.5 * inertia[i] * (x[i] - target[i]).length_squared();
    }
    for c in constraints::compile(g) {
        if c.ids.len() > 4 {
            return Err("Dense patch factors support at most four vertices per element".into());
        }
        // Retain Dual's arithmetic (notably reciprocal multiplication for
        // division), without carrying derivatives. Raw f64 division can differ
        // by an ulp and change borderline line-search decisions.
        let p: Vec<[Dual<0>; 3]> = c
            .ids
            .iter()
            .map(|&i| x[i].to_array().map(Dual::c))
            .collect();
        let q = (c.value(g, &p) * Dual::c(c.k.sqrt())).v;
        energy += 0.5 * q * q;
    }
    if !energy.is_finite() {
        return Err("Nonfinite dense patch energy".into());
    }
    Ok(energy)
}
pub fn assemble(
    g: &CompiledGarment,
    x: &[DVec3],
    target: &[DVec3],
    inertia: &[f64],
) -> Result<System, String> {
    let n = validate(g, x, target, inertia)?;
    let mut system = System {
        energy: 0.,
        gradient: vec![0.; n],
        metric: vec![0.; n * n],
    };
    for i in 0..x.len() {
        let d = x[i] - target[i];
        system.energy += 0.5 * inertia[i] * d.length_squared();
        for k in 0..3 {
            system.gradient[3 * i + k] = inertia[i] * d[k];
            system.metric[(3 * i + k) * n + 3 * i + k] = inertia[i];
        }
    }
    for c in constraints::compile(g) {
        if c.ids.len() > 4 {
            return Err("Dense patch factors support at most four vertices per element".into());
        }
        let p: Vec<[Dual<12>; 3]> = c
            .ids
            .iter()
            .enumerate()
            .map(|(a, &i)| std::array::from_fn(|k| Dual::variable(x[i][k], a * 3 + k)))
            .collect();
        let q = c.value(g, &p) * Dual::c(c.k.sqrt());
        system.energy += 0.5 * q.v * q.v;
        for (a, &i) in c.ids.iter().enumerate() {
            for k in 0..3 {
                let row = i * 3 + k;
                let derivative = q.g[a * 3 + k];
                system.gradient[row] += q.v * derivative;
                for (b, &j) in c.ids.iter().enumerate() {
                    for axis in 0..3 {
                        system.metric[row * n + j * 3 + axis] += derivative * q.g[b * 3 + axis];
                    }
                }
            }
        }
    }
    if !system.energy.is_finite()
        || system
            .gradient
            .iter()
            .chain(&system.metric)
            .any(|v| !v.is_finite())
    {
        return Err("Nonfinite dense patch assembly".into());
    }
    Ok(system)
}

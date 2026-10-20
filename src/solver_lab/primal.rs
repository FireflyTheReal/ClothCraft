//! Full-residual compliant solve. This is a Gauss-Newton experiment, not a
//! claim that OGC-clipped sequential XPBD solves backward Euler exactly.
use super::{constraints::Constraint, dual::Dual};
use crate::{garment::CompiledGarment, math::Scalar, ogc::ContactSet, vbd::VbdSettings};
use glam::DVec3;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct SweepDiagnostic {
    pub sweep: usize,
    /// Frozen feature set used by this sweep; changes at detection refreshes.
    pub objective_j: f64,
    pub target_residual_rms_n: f64,
    pub minimum_stiffness_ratio: f64,
    pub full_stiffness_fraction: f64,
    pub clipped_updates: usize,
    /// Change caused only by refreshing features at identical positions.
    pub contact_refresh_delta_j: f64,
}

pub use crate::ogc::diagnostics::{RefreshEvent, refresh_event};

struct Row {
    ids: [usize; 4],
    gradients: [DVec3; 4],
    count: usize,
}
pub(super) struct System {
    rows: Vec<Row>,
    pub gradient: Vec<DVec3>,
    diagonal: Vec<DVec3>,
    pub energy: f64,
}

#[allow(clippy::too_many_arguments)]
impl System {
    pub fn assemble(
        g: &CompiledGarment,
        constraints: &[Constraint],
        extra: &[Constraint],
        contacts: &ContactSet,
        x: &[DVec3],
        target: &[DVec3],
        inertia: &[f64],
        inverse: &[f64],
        s: &VbdSettings,
    ) -> Result<Self, String> {
        let n = inertia.len();
        let mut system = Self {
            rows: Vec::with_capacity(constraints.len() + extra.len() + contacts.contacts.len()),
            gradient: (0..n).map(|i| (x[i] - target[i]) * inertia[i]).collect(),
            diagonal: inertia.iter().map(|k| DVec3::splat(*k)).collect(),
            energy: (0..n)
                .filter(|i| inverse[*i] > 0.)
                .map(|i| 0.5 * inertia[i] * x[i].distance_squared(target[i]))
                .sum(),
        };
        for c in constraints.iter().chain(extra) {
            let p: [[Dual<12>; 3]; 4] = std::array::from_fn(|a| {
                std::array::from_fn(|k| {
                    Dual::variable(x[c.ids[a.min(c.ids.len() - 1)]][k], a * 3 + k)
                })
            });
            let q = c.value(g, &p) * Dual::c(c.k.sqrt());
            system.add(&c.ids, q, inverse)?;
        }
        for c in &contacts.contacts {
            let p = std::array::from_fn(|a| {
                std::array::from_fn(|k| Dual::<12>::variable(x[c.vertices[a]][k], a * 3 + k))
            });
            let e = c.energy(p, s.contact.radius, s.contact.stiffness);
            if !e.v.is_finite() || e.v < 0. {
                return Err("Invalid contact potential in primal assembly".into());
            }
            if e.v > 0. {
                system.add(&c.vertices[..c.count], (e * Dual::c(2.)).sqrt(), inverse)?;
            }
        }
        for (i, w) in inverse.iter().enumerate() {
            if *w == 0. {
                system.gradient[i] = DVec3::ZERO;
            }
        }
        if !system.energy.is_finite() || system.gradient.iter().any(|p| !p.is_finite()) {
            return Err("Non-finite primal system".into());
        }
        Ok(system)
    }
    fn add(&mut self, ids: &[usize], q: Dual<12>, inverse: &[f64]) -> Result<(), String> {
        if !q.v.is_finite() || q.g.iter().any(|v| !v.is_finite()) {
            return Err("Non-finite compliant constraint".into());
        }
        self.energy += 0.5 * q.v * q.v;
        let mut row = Row {
            ids: [0; 4],
            gradients: [DVec3::ZERO; 4],
            count: ids.len(),
        };
        for (a, i) in ids.iter().copied().enumerate() {
            row.ids[a] = i;
            if inverse.get(i).copied().unwrap_or(0.) == 0. {
                continue;
            }
            let d = DVec3::new(q.g[a * 3], q.g[a * 3 + 1], q.g[a * 3 + 2]);
            row.gradients[a] = d;
            self.gradient[i] += d * q.v;
            self.diagonal[i] += d * d;
        }
        self.rows.push(row);
        Ok(())
    }
    fn multiply(&self, p: &[DVec3], inertia: &[f64], result: &mut [DVec3]) {
        for ((out, v), k) in result.iter_mut().zip(p).zip(inertia) {
            *out = *v * *k;
        }
        for row in &self.rows {
            let d = (0..row.count)
                .filter(|a| row.ids[*a] < p.len())
                .map(|a| row.gradients[a].dot(p[row.ids[a]]))
                .sum::<f64>();
            for a in 0..row.count {
                if row.ids[a] < p.len() {
                    result[row.ids[a]] += row.gradients[a] * d;
                }
            }
        }
    }
    pub fn residual_rms(&self, inverse: &[f64]) -> f64 {
        let count = inverse.iter().filter(|w| **w > 0.).count().max(1);
        (dot(&self.gradient, &self.gradient) / (3 * count) as f64).sqrt()
    }
    pub fn direction(
        &self,
        inertia: &[f64],
        inverse: &[f64],
        limit: usize,
        tolerance: f64,
    ) -> Result<(Vec<DVec3>, usize), String> {
        let mut d = vec![DVec3::ZERO; inertia.len()];
        let mut r: Vec<_> = self.gradient.iter().map(|v| -*v).collect();
        let mut z: Vec<_> = r.iter().zip(&self.diagonal).map(|(v, k)| *v / *k).collect();
        let mut p = z.clone();
        let mut hp = d.clone();
        let mut rz = dot(&r, &z);
        let initial = dot(&r, &r);
        if initial == 0. {
            return Ok((d, 0));
        }
        for iteration in 0..limit {
            self.multiply(&p, inertia, &mut hp);
            let denom = dot(&p, &hp);
            if !denom.is_finite() || denom <= 0. || !rz.is_finite() {
                return Err("Primal PCG lost positive definiteness".into());
            }
            let a = rz / denom;
            for i in 0..d.len() {
                d[i] += p[i] * a;
                r[i] -= hp[i] * a;
            }
            if dot(&r, &r) <= initial * tolerance * tolerance {
                return Ok((d, iteration + 1));
            }
            for i in 0..z.len() {
                z[i] = if inverse[i] > 0. {
                    r[i] / self.diagonal[i]
                } else {
                    DVec3::ZERO
                };
            }
            let next = dot(&r, &z);
            let beta = next / rz;
            for i in 0..p.len() {
                p[i] = z[i] + beta * p[i];
            }
            rz = next;
        }
        Ok((d, limit))
    }
}

fn dot(a: &[DVec3], b: &[DVec3]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x.dot(*y)).sum()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn objective(
    g: &CompiledGarment,
    c: &[Constraint],
    extra: &[Constraint],
    contacts: &ContactSet,
    x: &[DVec3],
    target: &[DVec3],
    inertia: &[f64],
    inverse: &[f64],
    s: &VbdSettings,
) -> f64 {
    (0..inertia.len())
        .filter(|i| inverse[*i] > 0.)
        .map(|i| 0.5 * inertia[i] * x[i].distance_squared(target[i]))
        .sum::<f64>()
        + c.iter()
            .chain(extra)
            .map(|c| 0.5 * c.k * c.at(g, x).powi(2))
            .sum::<f64>()
        + contacts
            .contacts
            .iter()
            .map(|c| c.value(x, s.contact.radius, s.contact.stiffness))
            .sum::<f64>()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn descend(
    g: &CompiledGarment,
    c: &[Constraint],
    extra: &[Constraint],
    contacts: &ContactSet,
    x: &mut [DVec3],
    target: &[DVec3],
    inertia: &[f64],
    inverse: &[f64],
    s: &VbdSettings,
    system: &System,
    mut direction: Vec<DVec3>,
) -> Result<usize, String> {
    let mut clipped = 0;
    // Individual clipping avoids letting one narrow region stop unrelated panels.
    // It can destroy descent of a coupled direction, so test and fall back to
    // scalar diagonally preconditioned gradient descent before the line search.
    for attempt in 0..2 {
        clipped = 0;
        for (i, d) in direction.iter_mut().enumerate() {
            let fraction = super::feasible_fraction(contacts, i, x[i], *d);
            clipped += usize::from(fraction < 1.);
            *d *= fraction;
        }
        if dot(&system.gradient, &direction) < 0. {
            break;
        }
        if attempt == 1 {
            return Ok(clipped);
        }
        for (i, d) in direction.iter_mut().enumerate() {
            // One positive scale per vertex preserves descent after ray clipping.
            *d = -system.gradient[i] / system.diagonal[i].max_element();
        }
    }
    if direction.iter().any(|d| !d.is_finite()) {
        return Err("Non-finite primal direction".into());
    }
    let before = x.to_vec();
    let slope = dot(&system.gradient, &direction);
    for trial in 0..32 {
        let a = 0.5_f64.powi(trial);
        for i in 0..direction.len() {
            x[i] = before[i] + direction[i] * a;
        }
        let energy = objective(g, c, extra, contacts, x, target, inertia, inverse, s);
        if energy.is_finite() && energy <= system.energy + 1e-4 * a * slope {
            return Ok(clipped);
        }
    }
    x.copy_from_slice(&before);
    Ok(clipped)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn diagnostic(
    g: &CompiledGarment,
    c: &[Constraint],
    extra: &[Constraint],
    contacts: &ContactSet,
    x: &[DVec3],
    target: &[DVec3],
    inertia: &[f64],
    inverse: &[f64],
    s: &VbdSettings,
    sweep: usize,
    stiffness: &[f64],
    clipped_updates: usize,
) -> Result<SweepDiagnostic, String> {
    let system = System::assemble(g, c, extra, contacts, x, target, inertia, inverse, s)?;
    Ok(SweepDiagnostic {
        sweep,
        objective_j: system.energy,
        target_residual_rms_n: system.residual_rms(inverse),
        minimum_stiffness_ratio: c
            .iter()
            .zip(stiffness)
            .map(|(c, k)| k / c.k)
            .fold(1., f64::min),
        full_stiffness_fraction: c
            .iter()
            .zip(stiffness)
            .filter(|(c, k)| **k >= c.k * (1. - 1e-12))
            .count() as f64
            / c.len().max(1) as f64,
        clipped_updates,
        contact_refresh_delta_j: 0.,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DMat3;

    #[test]
    fn full_residual_recovers_coupled_linear_solution_from_clipped_prediction() {
        // A displaced start is deliberate: the XPBD g=0 assumption is false.
        let x = DVec3::new(0.02, -0.01, 0.03);
        let y = DVec3::new(0.3, -0.2, 0.15);
        let inertia = [4.];
        let inverse = [0.25];
        let mut system = System {
            rows: vec![],
            gradient: vec![inertia[0] * (x - y)],
            diagonal: vec![DVec3::splat(inertia[0])],
            energy: 0.,
        };
        let mut h = DMat3::IDENTITY * inertia[0];
        let mut rhs = y * inertia[0];
        for (normal, rest, k) in [
            (DVec3::new(1., 2., 0.), 0.1, 40.),
            (DVec3::new(0., 1., -1.), -0.2, 900.),
        ] {
            let mut q = Dual::<12>::c((normal.dot(x) - rest) * f64::sqrt(k));
            for a in 0..3 {
                q.g[a] = normal[a] * f64::sqrt(k);
            }
            system.add(&[0], q, &inverse).unwrap();
            h += DMat3::from_cols(normal * normal.x, normal * normal.y, normal * normal.z) * k;
            rhs += normal * (k * rest);
        }
        let (d, _) = system.direction(&inertia, &inverse, 12, 1e-12).unwrap();
        assert!((x + d[0]).distance(h.inverse() * rhs) < 1e-11);
        let mut hd = vec![DVec3::ZERO];
        system.multiply(&d, &inertia, &mut hd);
        assert!((hd[0] + system.gradient[0]).length() < 1e-9);
    }
}

//! Implicit Euler, regularized Newton, matrix-free element Hessian PCG and Armijo.
use crate::{
    blocksolve::{BlockPcgSettings, BlockPcgStatus, MatMixCsr, MatMixCsrBuilder, block_jacobi_pcg},
    contact::{Collider, Sphere},
    energy::{self, Element},
    garment::{CompiledGarment, seam::CompiledSeam},
    model::{Material, Mesh, State},
};
use glam::DVec3;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SolverSettings {
    pub dt: f64,
    pub gravity: DVec3,
    /// Mass-proportional viscous damping, s^-1, inside the variational solve.
    pub damping: f64,
    pub newton_iterations: usize,
    pub pcg_iterations: usize,
    pub absolute_tolerance: f64,
    pub relative_tolerance: f64,
}
impl Default for SolverSettings {
    fn default() -> Self {
        Self {
            dt: 1.0 / 60.0,
            gravity: DVec3::new(0.0, -9.81, 0.0),
            damping: 0.4,
            newton_iterations: 30,
            pcg_iterations: 250,
            absolute_tolerance: 1e-6,
            relative_tolerance: 1e-4,
        }
    }
}
impl SolverSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !self.dt.is_finite()
            || !(1e-6..=0.1).contains(&self.dt)
            || !self.gravity.is_finite()
            || !self.damping.is_finite()
            || self.damping < 0.0
            || self.newton_iterations == 0
            || self.newton_iterations > 200
            || self.pcg_iterations == 0
            || self.pcg_iterations > 5000
            || !self.absolute_tolerance.is_finite()
            || self.absolute_tolerance <= 0.0
            || !self.relative_tolerance.is_finite()
            || !(0.0..1.0).contains(&self.relative_tolerance)
        {
            return Err("Invalid solver settings".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Metrics {
    pub time_s: f64,
    pub dt_s: f64,
    pub vertices: usize,
    pub triangles: usize,
    pub newton_iterations: usize,
    pub pcg_iterations: usize,
    pub line_search_backtracks: usize,
    pub converged: bool,
    pub residual_rms_n: f64,
    pub tolerance_n: f64,
    pub objective_initial_j: f64,
    pub objective_final_j: f64,
    pub membrane_j: f64,
    pub bending_j: f64,
    pub contact_j: f64,
    pub seam_j: f64,
    pub max_seam_gap_m: f64,
    pub rms_seam_gap_m: f64,
    pub seam_samples: usize,
    pub kinetic_j: f64,
    pub gravity_j: f64,
    pub max_strain: f64,
    pub max_vertex_penetration_m: f64,
    pub initial_penetrating_vertices: usize,
    pub contacting_vertices: usize,
    pub min_y_m: f64,
    pub max_regularization: f64,
    pub assembly_ms: f64,
    pub pcg_ms: f64,
    pub line_search_ms: f64,
    pub step_ms: f64,
    /// Capacity-based estimate for solver element arrays only, not process RSS.
    pub element_storage_bytes: usize,
}

/// Borrowed model view keeps the existing single-panel API and the garment API
/// on exactly the same Newton/PCG implementation.
struct Problem<'a> {
    mesh: &'a Mesh,
    materials: &'a [Material],
    vertex_material: Option<&'a [usize]>,
    seams: &'a [CompiledSeam],
}
impl Problem<'_> {
    fn vertex_material(&self, i: usize) -> &Material {
        &self.materials[self.vertex_material.map_or(0, |v| v[i])]
    }
    fn validate(&self) -> Result<(), String> {
        for m in self.materials {
            m.validate()?;
        }
        let n = self.mesh.uv.len();
        for seam in self.seams {
            seam.validate(n)?;
        }
        if self.materials.is_empty()
            || self.mesh.masses.len() != n
            || self.mesh.rest_area.len() != n
            || self.mesh.pins.len() != n
            || self
                .vertex_material
                .is_some_and(|v| v.len() != n || v.iter().any(|i| *i >= self.materials.len()))
            || self
                .mesh
                .triangles
                .iter()
                .any(|t| t.material >= self.materials.len() || t.vertices.iter().any(|i| *i >= n))
            || self
                .mesh
                .hinges
                .iter()
                .any(|h| h.material >= self.materials.len() || h.vertices.iter().any(|i| *i >= n))
            || self.mesh.masses.iter().any(|m| !m.is_finite() || *m <= 0.0)
        {
            return Err("Invalid material binding or mesh topology".into());
        }
        Ok(())
    }
}

struct System {
    elements: Vec<Element>,
    gradient: Vec<f64>,
    mass_diagonal: Vec<f64>,
    fixed: Vec<bool>,
    energy: f64,
}
impl System {
    fn diagonal(&self, shift: f64) -> Vec<f64> {
        let mut d: Vec<_> = self.mass_diagonal.iter().map(|v| v + shift).collect();
        for e in &self.elements {
            let n = e.dofs.len();
            for (i, &global) in e.dofs.iter().enumerate() {
                d[global] += e.hessian[i * n + i];
            }
        }
        for (i, v) in d.iter_mut().enumerate() {
            *v = if self.fixed[i] { 1.0 } else { v.max(1e-12) };
        }
        d
    }
    fn apply(&self, x: &[f64], shift: f64) -> Vec<f64> {
        let mut out: Vec<_> = x
            .iter()
            .zip(&self.mass_diagonal)
            .map(|(x, d)| x * (d + shift))
            .collect();
        for e in &self.elements {
            let n = e.dofs.len();
            for (i, &global_i) in e.dofs.iter().enumerate() {
                if self.fixed[global_i] {
                    continue;
                }
                for (j, &global_j) in e.dofs.iter().enumerate() {
                    if !self.fixed[global_j] {
                        out[global_i] += e.hessian[i * n + j] * x[global_j];
                    }
                }
            }
        }
        for (i, v) in out.iter_mut().enumerate() {
            if self.fixed[i] {
                *v = x[i];
            }
        }
        out
    }
    /// Assemble the operator that [`Self::apply`] represents, as a mixed
    /// block-CSR matrix with one symmetric 3x3 block per node.
    ///
    /// `apply` and this assembly must describe the same linear operator; the
    /// `blocksolve_assembly_matches_the_matrix_free_operator` test in this module
    /// is the guard. Dirichlet dofs go through [`MatMixCsr::apply_dirichlet`],
    /// which reproduces `apply`'s convention of clearing constrained rows and
    /// columns and leaving a unit diagonal.
    fn assemble_operator(&self, shift: f64) -> MatMixCsr {
        let nodes = self.mass_diagonal.len() / 3;
        let mut builder = MatMixCsrBuilder::new(nodes);
        for node in 0..nodes {
            let mut block = [[0.0_f64; 3]; 3];
            for (k, row) in block.iter_mut().enumerate() {
                row[k] = self.mass_diagonal[3 * node + k] + shift;
            }
            builder.add_block(node, node, block);
        }
        for element in &self.elements {
            let dofs = element.dofs.len();
            let local = dofs / 3;
            // One triangle is enough. The element Hessian is symmetric and
            // `add_block` already stores a lower-ordering block as the transpose
            // of its upper counterpart, so visiting the mirrored pair as well
            // would double-count every off-diagonal block.
            for a in 0..local {
                // `Element::from_jet` numbers dofs as `3 * vertex + component`.
                let node_a = element.dofs[3 * a] / 3;
                debug_assert!(
                    (0..3).all(|k| element.dofs[3 * a + k] == 3 * node_a + k),
                    "element dofs must be node-major (3 * vertex + component)"
                );
                for b in a..local {
                    let node_b = element.dofs[3 * b] / 3;
                    let block = std::array::from_fn(|r| {
                        std::array::from_fn(|c| element.hessian[(3 * a + r) * dofs + (3 * b + c)])
                    });
                    builder.add_block(node_a, node_b, block);
                }
            }
        }
        let mut matrix = builder.build();
        matrix.apply_dirichlet(&self.fixed);
        matrix
    }
    /// Matrix-free point-Jacobi CG: the historical reference path.
    ///
    /// Retained as the fallback for an assembled operator whose nodal diagonal
    /// cannot be inverted, so one degenerate element cannot abort a step the
    /// matrix-free path could still take.
    fn pcg_point_jacobi(&self, shift: f64, limit: usize) -> (Option<Vec<f64>>, usize) {
        let n = self.gradient.len();
        let diag = self.diagonal(shift);
        let mut x = vec![0.0; n];
        let mut r: Vec<_> = self.gradient.iter().map(|v| -v).collect();
        let mut z: Vec<_> = r.iter().zip(&diag).map(|(r, d)| r / d).collect();
        let mut p = z.clone();
        let mut rz = dot(&r, &z);
        let tolerance = 1e-5 * dot(&r, &r).sqrt();
        for it in 0..limit {
            let ap = self.apply(&p, shift);
            let curvature = dot(&p, &ap);
            if !curvature.is_finite() || curvature <= 0.0 {
                return (None, it + 1);
            }
            let alpha = rz / curvature;
            for i in 0..n {
                x[i] += alpha * p[i];
                r[i] -= alpha * ap[i];
            }
            if dot(&r, &r).sqrt() <= tolerance {
                return (Some(x), it + 1);
            }
            for i in 0..n {
                z[i] = r[i] / diag[i];
            }
            let next = dot(&r, &z);
            let beta = next / rz;
            for i in 0..n {
                p[i] = z[i] + beta * p[i];
            }
            rz = next;
        }
        // Inexact Newton direction accepted only if descent and energy decrease are verified.
        (Some(x), limit)
    }
    /// Newton linear solve on the assembled nodal operator, preconditioned by the
    /// inverted nodal diagonal.
    ///
    /// Point-Jacobi keeps one scalar per dof and cannot see the coupling between
    /// the three translational dofs that share a vertex. An assembled 3x3 nodal
    /// block can, which is the structure the shipped CLO timestep uses; see
    /// [`crate::blocksolve`]. The matrix-free path is the fallback when no nodal
    /// preconditioner exists.
    fn pcg(&self, shift: f64, limit: usize) -> (Option<Vec<f64>>, usize) {
        let matrix = self.assemble_operator(shift);
        let Ok(preconditioner) = matrix.block_diagonal_inverse(1e-14) else {
            return self.pcg_point_jacobi(shift, limit);
        };
        let rhs: Vec<f64> = self.gradient.iter().map(|g| -g).collect();
        let mut direction = vec![0.0; rhs.len()];
        let settings = BlockPcgSettings {
            max_iterations: limit,
            ..BlockPcgSettings::default()
        };
        let report = block_jacobi_pcg(&matrix, &preconditioner, &rhs, &mut direction, &settings);
        match report.status {
            BlockPcgStatus::Converged | BlockPcgStatus::Exhausted => {
                (Some(direction), report.iterations)
            }
            BlockPcgStatus::NotPositiveDefinite | BlockPcgStatus::SingularBlock(_) => {
                (None, report.iterations)
            }
        }
    }
}
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}
fn rms(g: &[f64], fixed: &[bool]) -> f64 {
    (dot(g, g) / fixed.iter().filter(|v| !**v).count().max(1) as f64).sqrt()
}

pub struct ReferenceSolver;
impl ReferenceSolver {
    fn geometry_valid(mesh: &Mesh, x: &[DVec3], sphere: Option<&dyn Collider>) -> bool {
        x.iter()
            .all(|p| p.is_finite() && sphere.is_none_or(|s| s.valid_position(*p)))
            && mesh.triangles.iter().all(|t| {
                let [a, b, c] = t.vertices.map(|i| x[i]);
                (b - a).cross(c - a).length() > 2.0 * t.area * 1e-6
            })
    }
    fn inertial(mesh: &Mesh, old: &State, x: &[DVec3], s: &SolverSettings) -> f64 {
        x.iter()
            .enumerate()
            .map(|(i, p)| {
                let y = old.x[i] + s.dt * old.v[i];
                mesh.masses[i]
                    * (0.5 * (*p - y).length_squared() / s.dt.powi(2)
                        + 0.5 * s.damping / s.dt * (*p - old.x[i]).length_squared()
                        - s.gravity.dot(*p))
            })
            .sum()
    }
    fn objective(
        problem: &Problem<'_>,
        old: &State,
        x: &[DVec3],
        s: &SolverSettings,
        sphere: Option<&dyn Collider>,
    ) -> f64 {
        let mesh = problem.mesh;
        if !Self::geometry_valid(mesh, x, sphere) {
            return f64::INFINITY;
        }
        let membrane: f64 = mesh
            .triangles
            .iter()
            .map(|t| {
                energy::membrane(
                    t.vertices.map(|i| x[i].to_array()),
                    t,
                    &problem.materials[t.material],
                )
            })
            .sum();
        let bending: f64 = mesh
            .hinges
            .iter()
            .map(|h| {
                energy::bending(
                    h.vertices.map(|i| x[i].to_array()),
                    h,
                    &problem.materials[h.material],
                )
            })
            .sum();
        let contact: f64 = sphere.map_or(0.0, |sp| {
            x.iter()
                .enumerate()
                .map(|(i, p)| sp.value(*p, problem.vertex_material(i).thickness, mesh.rest_area[i]))
                .sum()
        });
        let seam: f64 = problem.seams.iter().map(|seam| seam.energy(x)).sum();
        Self::inertial(mesh, old, x, s) + membrane + bending + contact + seam
    }
    fn assemble(
        problem: &Problem<'_>,
        old: &State,
        x: &[DVec3],
        s: &SolverSettings,
        sphere: Option<&dyn Collider>,
    ) -> System {
        let mesh = problem.mesh;
        let mut elements: Vec<_> = mesh
            .triangles
            .par_iter()
            .map(|t| energy::membrane_element(x, t, &problem.materials[t.material]))
            .collect();
        elements.extend(
            mesh.hinges
                .par_iter()
                .map(|h| energy::bending_element(x, h, &problem.materials[h.material]))
                .collect::<Vec<_>>(),
        );
        if let Some(sp) = sphere {
            elements.extend(x.iter().enumerate().filter_map(|(i, p)| {
                sp.element(
                    i,
                    *p,
                    problem.vertex_material(i).thickness,
                    mesh.rest_area[i],
                )
            }));
        }
        elements.extend(problem.seams.iter().flat_map(|s| s.elements(x)));
        let n = x.len() * 3;
        let mut g = vec![0.0; n];
        let mut diagonal = vec![0.0; n];
        let mut fixed = vec![false; n];
        for (i, p) in x.iter().enumerate() {
            let delta = mesh.masses[i]
                * ((*p - old.x[i] - s.dt * old.v[i]) / s.dt.powi(2)
                    + s.damping / s.dt * (*p - old.x[i])
                    - s.gravity);
            for k in 0..3 {
                g[3 * i + k] = delta[k];
                diagonal[3 * i + k] = mesh.masses[i] * (1.0 / s.dt.powi(2) + s.damping / s.dt);
                fixed[3 * i + k] = mesh.pins[i].is_some();
            }
        }
        for e in &elements {
            for (i, &d) in e.dofs.iter().enumerate() {
                g[d] += e.gradient[i];
            }
        }
        for i in 0..n {
            if fixed[i] {
                g[i] = 0.0;
            }
        }
        let energy =
            Self::inertial(mesh, old, x, s) + elements.iter().map(|e| e.energy).sum::<f64>();
        System {
            elements,
            gradient: g,
            mass_diagonal: diagonal,
            fixed,
            energy,
        }
    }
    /// Transactional: errors leave positions, velocities and time unchanged.
    /// An iteration-limited step is returned explicitly as converged=false.
    pub fn step(
        mesh: &Mesh,
        state: &mut State,
        m: &Material,
        s: &SolverSettings,
        sphere: Option<&Sphere>,
    ) -> Result<Metrics, String> {
        Self::step_problem(
            Problem {
                mesh,
                materials: std::slice::from_ref(m),
                vertex_material: None,
                seams: &[],
            },
            state,
            s,
            sphere.map(|sp| sp as &dyn Collider),
        )
    }

    pub fn step_garment(
        garment: &CompiledGarment,
        state: &mut State,
        s: &SolverSettings,
        sphere: Option<&Sphere>,
    ) -> Result<Metrics, String> {
        Self::step_problem(
            Problem {
                mesh: &garment.mesh,
                materials: &garment.materials,
                vertex_material: Some(&garment.vertex_material),
                seams: &garment.seams,
            },
            state,
            s,
            sphere.map(|sp| sp as &dyn Collider),
        )
    }

    pub fn step_garment_with_collider(
        garment: &CompiledGarment,
        state: &mut State,
        s: &SolverSettings,
        sphere: Option<&dyn Collider>,
    ) -> Result<Metrics, String> {
        Self::step_problem(
            Problem {
                mesh: &garment.mesh,
                materials: &garment.materials,
                vertex_material: Some(&garment.vertex_material),
                seams: &garment.seams,
            },
            state,
            s,
            sphere,
        )
    }

    fn step_problem(
        problem: Problem<'_>,
        state: &mut State,
        s: &SolverSettings,
        sphere: Option<&dyn Collider>,
    ) -> Result<Metrics, String> {
        let mesh = problem.mesh;
        let started = Instant::now();
        s.validate()?;
        problem.validate()?;
        if let Some(sp) = sphere {
            sp.validate()?;
        }
        if state.x.len() != mesh.masses.len()
            || state.v.len() != state.x.len()
            || !state.time.is_finite()
            || state.v.iter().any(|v| !v.is_finite())
            || !Self::geometry_valid(mesh, &state.x, sphere)
        {
            return Err("Invalid state, collapsed triangle or vertex at sphere centre".into());
        }
        let mut x = state.x.clone();
        for (i, pin) in mesh.pins.iter().enumerate() {
            if let Some(p) = pin {
                x[i] = *p;
            }
        }
        let mut stats = Metrics {
            dt_s: s.dt,
            vertices: x.len(),
            triangles: mesh.triangles.len(),
            initial_penetrating_vertices: sphere.map_or(0, |sp| {
                x.iter()
                    .enumerate()
                    .filter(|(i, p)| {
                        sp.penetration(**p, problem.vertex_material(*i).thickness) > 0.0
                    })
                    .count()
            }),
            ..Default::default()
        };
        let mut tolerance = 0.0;
        for iteration in 0..=s.newton_iterations {
            let clock = Instant::now();
            let system = Self::assemble(&problem, state, &x, s, sphere);
            stats.assembly_ms += clock.elapsed().as_secs_f64() * 1000.0;
            let residual = rms(&system.gradient, &system.fixed);
            if !residual.is_finite() || !system.energy.is_finite() {
                return Err("Non-finite FEM assembly; step rolled back".into());
            }
            if iteration == 0 {
                stats.objective_initial_j = system.energy;
                tolerance = s.absolute_tolerance.max(s.relative_tolerance * residual);
            }
            let storage = system
                .elements
                .iter()
                .map(|e| {
                    (e.gradient.capacity() + e.hessian.capacity()) * 8
                        + e.dofs.capacity() * std::mem::size_of::<usize>()
                })
                .sum();
            stats.element_storage_bytes = stats.element_storage_bytes.max(storage);
            stats.residual_rms_n = residual;
            stats.tolerance_n = tolerance;
            stats.objective_final_j = system.energy;
            stats.newton_iterations = iteration;
            if residual <= tolerance {
                stats.converged = true;
                break;
            }
            if iteration == s.newton_iterations {
                break;
            }
            let scale = system.diagonal(0.0).into_iter().fold(1.0, f64::max);
            let mut accepted = None;
            let mut shift = 0.0;
            for _ in 0..12 {
                let clock = Instant::now();
                let (solve, iterations) = system.pcg(shift, s.pcg_iterations);
                stats.pcg_ms += clock.elapsed().as_secs_f64() * 1000.0;
                stats.pcg_iterations += iterations;
                if let Some(direction) = solve {
                    let slope = dot(&system.gradient, &direction);
                    if slope.is_finite() && slope < 0.0 {
                        let clock = Instant::now();
                        let mut alpha = 1.0;
                        for _ in 0..24 {
                            let trial: Vec<_> = x
                                .iter()
                                .enumerate()
                                .map(|(i, p)| {
                                    mesh.pins[i].unwrap_or(
                                        *p + alpha
                                            * DVec3::new(
                                                direction[3 * i],
                                                direction[3 * i + 1],
                                                direction[3 * i + 2],
                                            ),
                                    )
                                })
                                .collect();
                            let objective = Self::objective(&problem, state, &trial, s, sphere);
                            if objective.is_finite()
                                && objective <= system.energy + 1e-4 * alpha * slope
                            {
                                accepted = Some(trial);
                                break;
                            }
                            alpha *= 0.5;
                            stats.line_search_backtracks += 1;
                        }
                        stats.line_search_ms += clock.elapsed().as_secs_f64() * 1000.0;
                    }
                }
                if accepted.is_some() {
                    break;
                }
                shift = if shift == 0.0 {
                    scale * 1e-6
                } else {
                    shift * 10.0
                };
            }
            stats.max_regularization = stats.max_regularization.max(shift);
            x = accepted.ok_or("Newton line search failed; state unchanged. Reduce time step.")?;
        }
        let v: Vec<_> = x
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if mesh.pins[i].is_some() {
                    DVec3::ZERO
                } else {
                    (*p - state.x[i]) / s.dt
                }
            })
            .collect();
        stats.membrane_j = mesh
            .triangles
            .iter()
            .map(|t| {
                energy::membrane(
                    t.vertices.map(|i| x[i].to_array()),
                    t,
                    &problem.materials[t.material],
                )
            })
            .sum();
        stats.bending_j = mesh
            .hinges
            .iter()
            .map(|h| {
                energy::bending(
                    h.vertices.map(|i| x[i].to_array()),
                    h,
                    &problem.materials[h.material],
                )
            })
            .sum();
        stats.kinetic_j = v
            .iter()
            .enumerate()
            .map(|(i, v)| 0.5 * mesh.masses[i] * v.length_squared())
            .sum();
        stats.gravity_j = x
            .iter()
            .enumerate()
            .map(|(i, p)| -mesh.masses[i] * s.gravity.dot(*p))
            .sum();
        stats.max_strain = mesh
            .triangles
            .iter()
            .flat_map(|t| energy::stretches(&x, t, &problem.materials[t.material]))
            .map(|s| (s - 1.0).abs())
            .fold(0.0, f64::max);
        stats.min_y_m = x.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
        if let Some(sp) = sphere {
            stats.contact_j = x
                .iter()
                .enumerate()
                .map(|(i, p)| sp.value(*p, problem.vertex_material(i).thickness, mesh.rest_area[i]))
                .sum();
            stats.max_vertex_penetration_m = x
                .iter()
                .enumerate()
                .map(|(i, p)| sp.penetration(*p, problem.vertex_material(i).thickness))
                .fold(0.0, f64::max);
            stats.contacting_vertices = x
                .iter()
                .enumerate()
                .filter(|(i, p)| sp.penetration(**p, problem.vertex_material(*i).thickness) > 0.0)
                .count();
        }
        stats.seam_j = problem.seams.iter().map(|seam| seam.energy(&x)).sum();
        stats.max_seam_gap_m = problem
            .seams
            .iter()
            .map(|seam| seam.max_gap(&x))
            .fold(0.0, f64::max);
        let measure: f64 = problem.seams.iter().map(|seam| seam.measure()).sum();
        if measure > 0.0 {
            stats.rms_seam_gap_m = (problem
                .seams
                .iter()
                .map(|seam| seam.integrated_gap_squared(&x))
                .sum::<f64>()
                / measure)
                .sqrt();
        }
        stats.seam_samples = problem.seams.iter().map(|seam| seam.samples.len()).sum();
        state.x = x;
        state.v = v;
        state.time += s.dt;
        stats.time_s = state.time;
        stats.step_ms = started.elapsed().as_secs_f64() * 1000.0;
        tracing::debug!(
            time = state.time,
            residual = stats.residual_rms_n,
            ms = stats.step_ms,
            converged = stats.converged,
            "CPU FEM step"
        );
        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::garment::GarmentTemplate;

    fn demo_system(settings: &SolverSettings) -> (CompiledGarment, System) {
        let garment = GarmentTemplate::two_panel_demo()
            .compile()
            .expect("demo template compiles");
        let state = garment.initial_state.clone();
        let x = state.x.clone();
        let problem = Problem {
            mesh: &garment.mesh,
            materials: &garment.materials,
            vertex_material: Some(&garment.vertex_material),
            seams: &garment.seams,
        };
        let system = ReferenceSolver::assemble(&problem, &state, &x, settings, None);
        (garment, system)
    }

    /// The assembled mixed block-CSR operator must describe exactly the same
    /// linear operator as the matrix-free element Hessian path it replaces.
    ///
    /// This is the guard that keeps `System::assemble_operator` from drifting
    /// away from `System::apply`, including the Dirichlet convention for pinned
    /// dofs and the mass-plus-shift diagonal.
    #[test]
    fn blocksolve_assembly_matches_the_matrix_free_operator() {
        let settings = SolverSettings::default();
        let (_garment, system) = demo_system(&settings);
        let n = system.gradient.len();
        assert!(n > 0, "demo must produce dofs");
        assert!(
            system.fixed.iter().any(|f| *f),
            "demo must exercise pinned dofs"
        );
        assert!(
            system.fixed.iter().any(|f| !*f),
            "demo must exercise free dofs"
        );

        // Deterministic, non-trivial probe vector.
        let probe: Vec<f64> = (0..n)
            .map(|i| ((i % 7) as f64 - 3.0) * 0.125 + 0.0625)
            .collect();

        for shift in [0.0, 1e-3, 1.0] {
            let matrix = system.assemble_operator(shift);
            let expected = system.apply(&probe, shift);
            let mut assembled = vec![0.0; n];
            matrix.spmv(&probe, &mut assembled);
            for i in 0..n {
                let scale = expected[i].abs().max(1.0);
                assert!(
                    (assembled[i] - expected[i]).abs() <= 1e-9 * scale,
                    "shift {shift}: assembled[{i}] = {} but matrix-free = {}",
                    assembled[i],
                    expected[i]
                );
                if system.fixed[i] {
                    assert_eq!(
                        assembled[i], probe[i],
                        "pinned dof {i} must act as an identity row"
                    );
                }
            }
        }
    }

    /// A real Newton step must run through the block-Jacobi path and leave a
    /// finite state with the pinned vertices exactly where they were placed.
    #[test]
    fn block_jacobi_pcg_drives_a_real_newton_step() {
        let garment = GarmentTemplate::two_panel_demo()
            .compile()
            .expect("demo template compiles");
        let mut state = garment.initial_state.clone();
        let settings = SolverSettings::default();
        let metrics = ReferenceSolver::step_garment(&garment, &mut state, &settings, None)
            .expect("step succeeds");
        assert!(metrics.residual_rms_n.is_finite());
        assert!(
            metrics.pcg_iterations > 0,
            "the linear solve must actually run"
        );
        assert!(state.x.iter().all(|p| p.is_finite()));
        assert!(state.v.iter().all(|v| v.is_finite()));
        for (i, pin) in garment.mesh.pins.iter().enumerate() {
            if let Some(p) = pin {
                assert!(
                    (state.x[i] - *p).length() < 1e-12,
                    "pinned vertex {i} moved off its pin"
                );
            }
        }
    }

    /// The assembled block path must never need more iterations than the
    /// matrix-free point-Jacobi path on the same operator, and it must beat it
    /// where the nodal blocks carry real off-diagonal coupling.
    ///
    /// Measured on `two_panel_demo` (median nodal off-diagonal/diagonal ratio
    /// 0.24) the assembled path is strictly better; on the T-shirt fixtures the
    /// nodal blocks are nearly diagonal (ratio 0.025) and the two agree exactly.
    /// See `docs/clo3d-reverse-engineering.md` for the full sweep.
    #[test]
    fn block_jacobi_bounds_point_jacobi_iterations() {
        for template in [
            GarmentTemplate::two_panel_demo(),
            crate::garment::presets::basic(crate::garment::presets::GarmentKind::Tshirt, 2),
        ] {
            let garment = template.compile().expect("fixture compiles");
            let settings = SolverSettings::default();
            let state = garment.initial_state.clone();
            let x = state.x.clone();
            let problem = Problem {
                mesh: &garment.mesh,
                materials: &garment.materials,
                vertex_material: Some(&garment.vertex_material),
                seams: &garment.seams,
            };
            let system = ReferenceSolver::assemble(&problem, &state, &x, &settings, None);
            let (block, block_iters) = system.pcg(0.0, 250);
            let (point, point_iters) = system.pcg_point_jacobi(0.0, 250);
            assert!(block.is_some() && point.is_some());
            assert!(
                block_iters <= point_iters,
                "assembled block-Jacobi used {block_iters} iterations, \
                 point-Jacobi used {point_iters}"
            );
        }
    }
}

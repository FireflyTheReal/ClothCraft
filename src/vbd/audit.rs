//! Opt-in f64 energy accounting. Never called by a solver step or the UI worker.
//! Freeze contacts, controls, pins and settings across each audited interval.
//! A residual includes incomplete solving AND reactions from movement bounds;
//! it must not be interpreted as an isolated numerical energy source.
use super::*;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Energy {
    pub kinetic_j: f64,
    pub membrane_j: f64,
    pub bending_j: f64,
    pub sewing_j: f64,
    pub gravity_j: f64,
    pub contact_j: f64,
    pub grab_j: f64,
}
impl Energy {
    pub fn potential_j(&self) -> f64 {
        self.membrane_j
            + self.bending_j
            + self.sewing_j
            + self.gravity_j
            + self.contact_j
            + self.grab_j
    }
    pub fn total_j(&self) -> f64 {
        self.kinetic_j + self.potential_j()
    }
    fn potential_change(&self, old: &Self) -> f64 {
        (self.membrane_j - old.membrane_j)
            + (self.bending_j - old.bending_j)
            + (self.sewing_j - old.sewing_j)
            + (self.gravity_j - old.gravity_j)
            + (self.contact_j - old.contact_j)
            + (self.grab_j - old.grab_j)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct StepEnergy {
    pub before: Energy,
    pub after: Energy,
    pub delta_mechanical_j: f64,
    /// h * damping * sum(m * |v_new|²), nonnegative.
    pub drag_loss_j: f64,
    /// 0.5 * sum(m * |v_new-v_old|²), backward Euler kinetic defect.
    pub implicit_kinetic_loss_j: f64,
    /// U_new-U_old - grad(U_new) dot (x_new-x_old). Nonpositive only for convex U.
    pub nonlinear_remainder_j: f64,
    pub residual_work_j: f64,
    /// Recomputed at the complete final configuration, excluding fixed pins.
    pub residual_rms_n: f64,
    pub kinematic_error_m: f64,
    /// Algebraic accounting check, NOT a test of physical convergence.
    pub balance_error_j: f64,
}

impl VbdSolver {
    /// Physical forces at selected vertices, for the supplied contact feature
    /// set. Read-only/offline; excludes inertia and mass-proportional drag.
    /// Fresh end-state contacts need not match the sets used inside a step.
    pub fn audit_vertex_forces(
        &self,
        state: &State,
        settings: &VbdSettings,
        contacts: &ContactSet,
        grab: Option<&Grab>,
        vertices: &[usize],
    ) -> Result<Vec<DVec3>, String> {
        self.validate_audit(state, settings, contacts, grab)?;
        if vertices.iter().any(|&v| v >= state.x.len()) {
            return Err("Invalid force-audit vertex".into());
        }
        let x = self.world.positions(&state.x);
        let forces: Vec<_> = vertices
            .iter()
            .map(|&v| {
                let j = self.local(
                    v,
                    std::array::from_fn(|k| Jet::<3>::variable(x[v][k], k)),
                    &x,
                    contacts,
                    settings,
                    grab,
                );
                self.garment.mesh.masses[v] * settings.gravity - DVec3::from_array(j.g)
            })
            .collect();
        if forces.iter().any(|f| !f.is_finite()) {
            return Err("Non-finite force audit".into());
        }
        Ok(forces)
    }

    fn validate_audit(
        &self,
        state: &State,
        settings: &VbdSettings,
        contacts: &ContactSet,
        grab: Option<&Grab>,
    ) -> Result<(), String> {
        settings.validate()?;
        let n = self.garment.mesh.uv.len();
        let all = n + self.world.fixed.len();
        if state.x.len() != n
            || state.v.len() != n
            || !state.time.is_finite()
            || state.x.iter().chain(&state.v).any(|p| !p.is_finite())
            || contacts
                .contacts
                .iter()
                .any(|c| !(2..=4).contains(&c.count) || c.vertices.iter().any(|&v| v >= all))
            || contacts.incident != crate::ogc::ContactIncidence::new(n, &contacts.contacts)
        {
            return Err("Energy audit dimensions or values are invalid".into());
        }
        if self.garment.mesh.pins.iter().enumerate().any(|(i, pin)| {
            pin.is_some_and(|p| state.x[i].distance(p) > 1e-10 || state.v[i].length() > 1e-10)
        }) {
            return Err("Energy audit requires stationary pins".into());
        }
        if let Some(g) = grab
            && (g.vertices.iter().any(|&i| i >= n)
                || !g.target.is_finite()
                || !g.stiffness.is_finite()
                || g.stiffness <= 0.
                || g.weights.iter().any(|w| !w.is_finite() || *w < 0.)
                || (g.weights.iter().sum::<f64>() - 1.).abs() > 1e-8
                || !g.valid_patch(n))
        {
            return Err("Invalid energy-audit grab".into());
        }
        Ok(())
    }

    /// Sum each physical term once. Contact energy is for the supplied frozen
    /// feature set, not a globally smooth collision potential across detection.
    pub fn audit_energy(
        &self,
        state: &State,
        settings: &VbdSettings,
        contacts: &ContactSet,
        grab: Option<&Grab>,
    ) -> Result<Energy, String> {
        self.validate_audit(state, settings, contacts, grab)?;
        let g = &self.garment;
        let result = Energy {
            kinetic_j: g
                .mesh
                .masses
                .iter()
                .zip(&state.v)
                .map(|(m, v)| 0.5 * m * v.length_squared())
                .sum(),
            membrane_j: g
                .mesh
                .triangles
                .iter()
                .map(|t| {
                    energy::membrane(
                        t.vertices.map(|i| state.x[i].to_array()),
                        t,
                        &g.materials[t.material],
                    )
                })
                .sum(),
            bending_j: g
                .mesh
                .hinges
                .iter()
                .map(|h| {
                    energy::bending(
                        h.vertices.map(|i| state.x[i].to_array()),
                        h,
                        &g.materials[h.material],
                    )
                })
                .sum(),
            sewing_j: g.seams.iter().map(|s| s.energy(&state.x)).sum(),
            gravity_j: g
                .mesh
                .masses
                .iter()
                .zip(&state.x)
                .map(|(m, x)| -m * settings.gravity.dot(*x))
                .sum(),
            contact_j: contacts.value(&self.world.positions(&state.x), &settings.contact),
            grab_j: grab.map_or(0., |g| {
                let p: DVec3 = g
                    .vertices
                    .iter()
                    .zip(g.weights)
                    .map(|(&i, w)| w * state.x[i])
                    .sum();
                0.5 * g.point_stiffness() * p.distance_squared(g.target) + g.patch_energy(&state.x)
            }),
        };
        if !result.total_j().is_finite() {
            return Err("Non-finite energy in audit; contact features may be singular".into());
        }
        Ok(result)
    }

    /// Decompose accepted-step energy change without altering either state.
    /// Moving the grab target or refreshing contacts must be accounted for
    /// separately by evaluating both controls/feature sets at the SAME position.
    pub fn audit_step(
        &self,
        before: &State,
        after: &State,
        settings: &VbdSettings,
        contacts: &ContactSet,
        grab: Option<&Grab>,
    ) -> Result<StepEnergy, String> {
        let old = self.audit_energy(before, settings, contacts, grab)?;
        let new = self.audit_energy(after, settings, contacts, grab)?;
        let dt = settings.dt;
        if (after.time - before.time - dt).abs() > dt * 1e-6 {
            return Err("Energy audit requires consecutive steps at the specified dt".into());
        }
        let x = self.world.positions(&after.x);
        let mut drag = 0.;
        let mut implicit = 0.;
        let mut gradient_work = 0.;
        let mut residual_work = 0.;
        let mut residual2 = 0.;
        let mut free = 0;
        let mut kinematic_error = 0.0_f64;
        for i in 0..after.x.len() {
            let dx = after.x[i] - before.x[i];
            let dv = after.v[i] - before.v[i];
            let mass = self.garment.mesh.masses[i];
            kinematic_error = kinematic_error.max((dx - dt * after.v[i]).length());
            drag += dt * settings.damping * mass * after.v[i].length_squared();
            implicit += 0.5 * mass * dv.length_squared();
            if self.garment.mesh.pins[i].is_some() {
                continue;
            }
            let j = self.local(
                i,
                std::array::from_fn(|k| Jet::<3>::variable(x[i][k], k)),
                &x,
                contacts,
                settings,
                grab,
            );
            let grad = DVec3::from_array(j.g) - mass * settings.gravity;
            let residual = mass * dv / dt + settings.damping * mass * after.v[i] + grad;
            gradient_work += grad.dot(dx);
            residual_work += residual.dot(dx);
            residual2 += residual.length_squared();
            free += 1;
        }
        if kinematic_error > 1e-10 || !residual2.is_finite() {
            return Err(
                "Energy audit requires finite derivatives and x_new-x_old = dt*v_new".into(),
            );
        }
        let du = new.potential_change(&old);
        let delta = new.kinetic_j - old.kinetic_j + du;
        let remainder = du - gradient_work;
        Ok(StepEnergy {
            before: old,
            after: new,
            delta_mechanical_j: delta,
            drag_loss_j: drag,
            implicit_kinetic_loss_j: implicit,
            nonlinear_remainder_j: remainder,
            residual_work_j: residual_work,
            residual_rms_n: (residual2 / (3 * free).max(1) as f64).sqrt(),
            kinematic_error_m: kinematic_error,
            balance_error_j: delta + drag + implicit - remainder - residual_work,
        })
    }

    /// One-shot accepted-state diagnostic for the Simulation inspector.
    ///
    /// Reads `previous` and `accepted` without changing either one and clones
    /// the collision world before the fresh detection, so the next accepted
    /// step cannot inherit a diagnostic cache. The residual is the
    /// backward-Euler stationarity expression for free vertices, using the
    /// accepted velocity and physical forces at the accepted positions with
    /// **final** OGC contacts:
    ///
    /// `r_i = m_i (v_i^n - v_i^{n-1}) / dt + damping m_i v_i^n - F_i`
    ///
    /// The reported value is `sqrt(sum |r_i|^2 / (3 * free))` in newtons.
    /// It includes inertia, damping, gravity, membrane, bending, sewing,
    /// contact and grab terms, but it is not a KKT certificate: movement
    /// bounds, contact activation and the final detection are not solved as a
    /// constrained equilibrium. A `CpuAuditOfGpuState` sample audits the
    /// accepted GPU state on the CPU; it does not claim a GPU residual.
    pub fn sample_final_diagnostics(
        &self,
        previous: &State,
        accepted: &State,
        settings: &VbdSettings,
        grab: Option<&Grab>,
        backend: FinalDiagnosticsBackend,
    ) -> Result<FinalDiagnostics, String> {
        settings.validate()?;
        let n = self.garment.mesh.masses.len();
        if previous.x.len() != n
            || previous.v.len() != n
            || accepted.x.len() != n
            || accepted.v.len() != n
        {
            return Err("Final diagnostics state dimensions do not match the garment".into());
        }
        if !previous.time.is_finite()
            || !accepted.time.is_finite()
            || previous
                .x
                .iter()
                .chain(&previous.v)
                .chain(&accepted.x)
                .chain(&accepted.v)
                .any(|p| !p.is_finite())
        {
            return Err("Final diagnostics requires finite states".into());
        }
        let dt = accepted.time - previous.time;
        if !dt.is_finite() || dt <= 0.0 || (dt - settings.dt).abs() > settings.dt.abs() * 1e-6 {
            return Err(
                "Final diagnostics requires exactly one accepted step at the configured dt".into(),
            );
        }
        // The accepted velocity must be the backward-Euler velocity of this
        // step; otherwise `previous` is not the state that produced `accepted`.
        if (0..n).any(|i| ((accepted.x[i] - previous.x[i]) - accepted.v[i] * dt).length() > 1e-9) {
            return Err("Final diagnostics requires x_new - x_old = dt * v_new".into());
        }

        let start = std::time::Instant::now();
        let accepted_positions = self.world.positions(&accepted.x);
        let mut verifier = self.world.clone();
        let contacts = verifier.detect(
            &accepted_positions,
            2.0 * settings.contact.radius,
            &settings.contact,
        )?;
        let detection_ms = start.elapsed().as_secs_f64() * 1000.0;

        let free: Vec<usize> = (0..n)
            .filter(|&i| self.garment.mesh.pins[i].is_none())
            .collect();
        let forces = self.audit_vertex_forces(accepted, settings, &contacts, grab, &free)?;
        let mut residual2 = 0.0;
        for (i, force) in free.iter().zip(&forces) {
            let mass = self.garment.mesh.masses[*i];
            let residual = mass * (accepted.v[*i] - previous.v[*i]) / dt
                + settings.damping * mass * accepted.v[*i]
                - *force;
            residual2 += residual.length_squared();
        }
        let sample_ms = start.elapsed().as_secs_f64() * 1000.0;
        let minimum_distance_m = if contacts.contacts.is_empty() {
            None
        } else {
            Some(contacts.minimum_distance)
        };
        if !residual2.is_finite() || minimum_distance_m.is_some_and(|d| !d.is_finite()) {
            return Err("Final diagnostics produced a non-finite measurement".into());
        }
        Ok(FinalDiagnostics {
            state_time_s: accepted.time,
            step_dt_s: dt,
            sample_ms,
            detection_ms,
            residual_ms: (sample_ms - detection_ms).max(0.0),
            backend,
            contacts: contacts.active_contacts,
            contact_terms: contacts.contacts.len(),
            minimum_distance_m,
            free_vertices: free.len(),
            residual_rms_n: if free.is_empty() {
                None
            } else {
                Some((residual2 / (3 * free.len()) as f64).sqrt())
            },
            residual_kind: "free_dof_backward_euler_accepted_state_final_contacts",
        })
    }
}

//! CPU f64 Vertex Block Descent with OGC trust regions. Exact local gradients
//! and Hessians, positive local solve + objective-checked steps. No XPBD.
use crate::{
    avatar::SurfaceMesh,
    energy,
    garment::CompiledGarment,
    math::{Jet, Scalar},
    model::State,
    ogc::{ContactSet, OgcSettings, OgcWorld},
};
use glam::{DMat3, DVec3};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, sync::Arc, time::Instant};
pub mod audit;
mod coloring;
pub mod grab;
mod local_step;
mod prediction;
mod seam_blocks;
pub use seam_blocks::SeamBlockMetrics;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VbdSettings {
    pub dt: f64,
    pub iterations: usize,
    pub damping: f64,
    pub gravity: DVec3,
    pub contact: OgcSettings,
}
impl Default for VbdSettings {
    fn default() -> Self {
        Self {
            dt: 1.0 / 240.0,
            iterations: 16,
            damping: 1.5,
            gravity: DVec3::new(0.0, -9.81, 0.0),
            contact: Default::default(),
        }
    }
}
impl VbdSettings {
    pub fn validate(&self) -> Result<(), String> {
        self.contact.validate()?;
        if !self.dt.is_finite()
            || !(1e-6..=0.025).contains(&self.dt)
            || self.iterations == 0
            || self.iterations > 200
            || !self.damping.is_finite()
            || self.damping < 0.0
            || !self.gravity.is_finite()
        {
            return Err("Invalid VBD settings".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Grab {
    pub vertices: [usize; 3],
    pub weights: [f64; 3],
    pub target: DVec3,
    pub stiffness: f64,
    /// Independent surface attachments, sorted by vertex. Empty retains the
    /// historical barycentric point grab exactly.
    pub patch: Vec<grab::PatchAttachment>,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct GpuStepProfile {
    pub coloring_ms: f64,
    pub colors: usize,
    pub packing_ms: f64,
    pub upload_ms: f64,
    pub encode_ms: f64,
    /// Submission, fence wait and mapping, including compute (not additional to it).
    pub submit_wait_ms: f64,
    /// Readback processing, precision refinement, state update and diagnostics.
    pub post_ms: f64,
    /// Scope cleanup after the internal step timer, included in full step_ms.
    pub cleanup_ms: f64,
}

/// One-shot, read-only measurement of an accepted VBD step. The sample is
/// produced after the step and never changes the accepted trajectory.
#[derive(Clone, Debug, Serialize)]
pub struct FinalDiagnostics {
    /// Physical time of the accepted state.
    pub state_time_s: f64,
    /// The accepted step duration used by the implicit-Euler residual.
    pub step_dt_s: f64,
    /// Total wall time of the sample, including detection and force audit.
    pub sample_ms: f64,
    /// Final accepted-position OGC detection time inside `sample_ms`.
    pub detection_ms: f64,
    /// Free-degree force-audit time inside `sample_ms`.
    pub residual_ms: f64,
    pub backend: FinalDiagnosticsBackend,
    pub contacts: usize,
    /// Retained nearby interactions, including currently inactive potentials.
    pub contact_terms: usize,
    /// Minimum signed feature separation when at least one retained term
    /// exists; `None` means no contact term, never a fabricated distance.
    pub minimum_distance_m: Option<f64>,
    pub free_vertices: usize,
    /// RMS over free translational degrees of freedom. `None` when every
    /// vertex is pinned; an unavailable value is never reported as zero.
    pub residual_rms_n: Option<f64>,
    pub residual_kind: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalDiagnosticsBackend {
    /// The accepted step and the final-state audit both used the CPU solver.
    Cpu,
    /// The accepted state came from the GPU backend; the audit itself is the
    /// CPU force path, so no GPU timing or GPU residual is claimed.
    CpuAuditOfGpuState,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct VbdMetrics {
    pub seam_blocks: SeamBlockMetrics,
    pub motion: Option<crate::motion::MotionMetrics>,
    pub time_s: f64,
    pub dt_s: f64,
    pub step_ms: f64,
    pub detection_ms: f64,
    pub contact_profile: crate::ogc::DetectionProfile,
    pub solve_ms: f64,
    /// Hardware timestamps; absent for CPU or unsupported timestamp queries.
    pub gpu_compute_ms: Option<f64>,
    pub gpu_profile: Option<GpuStepProfile>,
    /// Native 32-lane subgroup reduction; portable workgroup fallback otherwise.
    pub gpu_subgroups: bool,
    /// Vertices advanced in f64 for roundoff or sparse directional restrictions.
    pub precision_vertices: usize,
    pub precision_ms: f64,
    pub directional_vertices: usize,
    pub iterations: usize,
    pub contacts: usize,
    /// Retained nearby interactions, including currently inactive potentials.
    pub contact_terms: usize,
    pub detection_updates: usize,
    pub broad_phase_rebuilds: usize,
    pub truncated_vertices: usize,
    pub minimum_distance_m: f64,
    pub max_seam_gap_m: f64,
    pub extension_p95_percent: f64,
    pub extension_max_percent: f64,
    pub residual_rms_n: Option<f64>,
}
#[derive(Clone)]
struct Stitch {
    terms: Vec<(usize, f64)>,
    stiffness: f64,
}
#[derive(Clone)]
pub struct VbdSolver {
    /// Optional CPU diagnostic: energy changes at identical geometry when the
    /// contact set refreshes inside the most recent step. Disabled by default.
    pub contact_refresh_audit: Option<Vec<f64>>,
    /// Highest signed refresh delta in the latest CPU step, when audit is on.
    /// Capture only; never read by the numerical algorithm.
    pub contact_refresh_capture: Option<crate::ogc::diagnostics::RefreshEvent>,
    local_probe: local_step::LocalSolveProbe,
    pub(crate) seam_block_sweeps: usize,
    seam_components: Vec<seam_blocks::Block>,
    pub(crate) motion: crate::motion::MotionMonitor,
    pub(crate) prediction: prediction::Prediction,
    pub garment: Arc<CompiledGarment>,
    pub world: OgcWorld,
    triangles: Vec<Vec<usize>>,
    hinges: Vec<Vec<usize>>,
    stitches: Vec<Stitch>,
    vertex_stitches: Vec<Vec<usize>>,
    graph: Vec<Vec<usize>>,
}
impl VbdSolver {
    pub fn new(
        garment: Arc<CompiledGarment>,
        avatar: Option<&SurfaceMesh>,
        state: &State,
    ) -> Result<Self, String> {
        let n = garment.mesh.uv.len();
        if state.x.len() != n || state.v.len() != n {
            return Err("VBD state dimensions do not match garment".into());
        }
        if garment
            .mesh
            .pins
            .iter()
            .enumerate()
            .any(|(i, p)| p.is_some_and(|p| state.x[i].distance(p) > 1e-10))
        {
            return Err(
                "VBD pinned positions must match their constraints before initialization".into(),
            );
        }
        let mut x = state.x.clone();
        let mut faces: Vec<_> = garment.mesh.triangles.iter().map(|t| t.vertices).collect();
        if let Some(a) = avatar {
            a.validate()?;
            x.extend_from_slice(&a.vertices);
            faces.extend(a.triangles.iter().map(|t| t.map(|i| i + n)));
        }
        let mut excluded = BTreeSet::new();
        let mut stitches = Vec::new();
        for seam in &garment.seams {
            for sample in &seam.samples {
                for (a, wa) in &sample.point.terms {
                    for (b, wb) in &sample.point.terms {
                        if wa * wb < 0.0 {
                            excluded.insert([(*a).min(*b), (*a).max(*b)]);
                        }
                    }
                }
                stitches.push(Stitch {
                    terms: sample.point.terms.clone(),
                    stiffness: seam.stiffness * sample.weight_m,
                });
            }
        }
        let mut world = OgcWorld::new(&x, faces, n, excluded)?;
        world.validate_start(&x)?;
        world.directional_bounds = true;
        let mut triangles = vec![vec![]; n];
        let mut hinges = vec![vec![]; n];
        let mut vertex_stitches = vec![vec![]; n];
        let mut graph = vec![BTreeSet::new(); n];
        let mut connect = |ids: &[usize]| {
            for a in ids {
                for b in ids {
                    if a != b {
                        graph[*a].insert(*b);
                    }
                }
            }
        };
        for (i, t) in garment.mesh.triangles.iter().enumerate() {
            for v in t.vertices {
                triangles[v].push(i);
            }
            connect(&t.vertices);
        }
        for (i, h) in garment.mesh.hinges.iter().enumerate() {
            for v in h.vertices {
                hinges[v].push(i);
            }
            connect(&h.vertices);
        }
        for (i, s) in stitches.iter().enumerate() {
            let ids: Vec<_> = s.terms.iter().map(|(v, _)| *v).collect();
            for v in &ids {
                vertex_stitches[*v].push(i);
            }
            connect(&ids);
        }
        let solver = Self {
            contact_refresh_audit: None,
            contact_refresh_capture: None,
            local_probe: Default::default(),
            seam_block_sweeps: 0,
            seam_components: Vec::new(),
            motion: Default::default(),
            prediction: Default::default(),
            garment,
            world,
            triangles,
            hinges,
            stitches,
            vertex_stitches,
            graph: graph
                .into_iter()
                .map(|neighbors| neighbors.into_iter().collect())
                .collect(),
        };
        Ok(solver)
    }
    pub(crate) fn colors(&self, contact: &ContactSet, grab: Option<&Grab>) -> Vec<Vec<usize>> {
        coloring::colors(&self.graph, contact, grab)
    }
    pub(crate) fn local<S: Scalar>(
        &self,
        v: usize,
        p: [S; 3],
        x: &[DVec3],
        contact: &ContactSet,
        settings: &VbdSettings,
        grab: Option<&Grab>,
    ) -> S {
        let get = |i: usize| if i == v { p } else { x[i].to_array().map(S::c) };
        let mut value = S::c(0.0);
        for i in &self.triangles[v] {
            let t = &self.garment.mesh.triangles[*i];
            value = value
                + energy::membrane(t.vertices.map(get), t, &self.garment.materials[t.material]);
        }
        for i in &self.hinges[v] {
            let h = &self.garment.mesh.hinges[*i];
            value = value
                + energy::bending(h.vertices.map(get), h, &self.garment.materials[h.material]);
        }
        for i in &self.vertex_stitches[v] {
            let s = &self.stitches[*i];
            let mut d = [S::c(0.0); 3];
            for (j, w) in &s.terms {
                let q = get(*j);
                for k in 0..3 {
                    d[k] = d[k] + S::c(*w) * q[k];
                }
            }
            value = value + S::c(0.5 * s.stiffness) * crate::math::dot(d, d);
        }
        for i in &contact.incident[v] {
            let c = &contact.contacts[*i];
            value = value
                + c.energy(
                    c.vertices.map(get),
                    settings.contact.radius,
                    settings.contact.stiffness,
                );
        }
        if let Some(g) = grab.filter(|g| g.vertices.contains(&v)) {
            let mut d = g.target.to_array().map(|a| S::c(-a));
            for (j, w) in g.vertices.iter().zip(g.weights) {
                let q = get(*j);
                for k in 0..3 {
                    d[k] = d[k] + S::c(w) * q[k];
                }
            }
            value = value + S::c(0.5 * g.point_stiffness()) * crate::math::dot(d, d);
        }
        if let Some(g) = grab
            && let Some(a) = g.attachment(v)
        {
            let target = g.target + a.offset;
            let d = crate::math::sub(get(v), target.to_array().map(S::c));
            value = value + S::c(0.5 * g.patch_stiffness() * a.weight) * crate::math::dot(d, d);
        }
        value
    }
    /// State is committed only after a finite step. Fixed geometry never moves.
    pub fn step(
        &mut self,
        state: &mut State,
        settings: &VbdSettings,
        grab: Option<&Grab>,
    ) -> Result<VbdMetrics, String> {
        settings.validate()?;
        let n = self.garment.mesh.uv.len();
        if state.x.len() != n
            || state.v.len() != n
            || !state.time.is_finite()
            || state.x.iter().chain(&state.v).any(|v| !v.is_finite())
        {
            return Err("Invalid VBD state".into());
        }
        if let Some(g) = grab
            && (g.vertices.iter().any(|i| *i >= n)
                || !g.target.is_finite()
                || !g.stiffness.is_finite()
                || g.stiffness <= 0.0
                || g.weights.iter().any(|w| !w.is_finite() || *w < 0.0)
                || (g.weights.iter().sum::<f64>() - 1.0).abs() > 1e-8
                || !g.valid_patch(n))
        {
            return Err("Invalid mouse attachment".into());
        }
        let start = Instant::now();
        let mut metrics = VbdMetrics {
            dt_s: settings.dt,
            iterations: settings.iterations,
            ..Default::default()
        };
        let dt = settings.dt;
        let old_velocity = state.v.clone();
        let guess = self.prediction.guess(state, settings);
        let damp = 1.0 + settings.damping * dt;
        let target: Vec<_> = state
            .x
            .iter()
            .zip(&state.v)
            .map(|(x, v)| *x + (*v * dt + settings.gravity * dt * dt) / damp)
            .collect();
        let inertia: Vec<_> = self
            .garment
            .mesh
            .masses
            .iter()
            .map(|m| m * damp / (dt * dt))
            .collect();
        let predicted_motion = target
            .iter()
            .zip(&state.x)
            .map(|(a, b)| a.distance(*b))
            .fold(0.0, f64::max);
        let rq = (settings.contact.radius + predicted_motion)
            .clamp(2.0 * settings.contact.radius, 4.0 * settings.contact.radius);
        let mut x = self.world.positions(&state.x);
        self.contact_refresh_capture = None;
        if let Some(audit) = &mut self.contact_refresh_audit {
            audit.clear();
        }
        let detect = Instant::now();
        let mut contacts = self.world.detect(&x, rq, &settings.contact)?;
        if contacts.minimum_distance <= 1e-12 {
            return Err("OGC zero separation; step rejected, reset the arrangement".into());
        }
        metrics.detection_ms += detect.elapsed().as_secs_f64() * 1000.0;
        metrics.detection_updates += 1;
        metrics.broad_phase_rebuilds += usize::from(contacts.broad_phase_rebuilt);
        metrics.contact_profile += contacts.profile;
        let mut colors = self.colors(&contacts, grab);
        // Conservative warm start; no unbounded inertial prediction crosses a face.
        let mut clipped = 0;
        for i in 0..n {
            if self.garment.mesh.pins[i].is_none() {
                let (p, b) = contacts.truncate(i, guess[i]);
                x[i] = p;
                clipped += usize::from(b);
            }
        }
        for sweep in 0..settings.iterations {
            if clipped > 0 && clipped as f64 >= settings.contact.refresh_fraction * n as f64 {
                let detect = Instant::now();
                let previous_energy = self
                    .contact_refresh_audit
                    .as_ref()
                    .map(|_| contacts.value(&x, &settings.contact));
                let refreshed = self.world.detect(&x, rq, &settings.contact)?;
                if let (Some(audit), Some(previous)) =
                    (&mut self.contact_refresh_audit, previous_energy)
                {
                    let delta = refreshed.value(&x, &settings.contact) - previous;
                    audit.push(delta);
                    if self
                        .contact_refresh_capture
                        .as_ref()
                        .is_none_or(|e| delta > e.delta_j)
                    {
                        self.contact_refresh_capture =
                            Some(crate::ogc::diagnostics::refresh_event(
                                &contacts,
                                &refreshed,
                                &x,
                                n,
                                &settings.contact,
                                sweep + 1,
                                delta,
                            ));
                    }
                }
                contacts = refreshed;
                colors = self.colors(&contacts, grab);
                metrics.detection_ms += detect.elapsed().as_secs_f64() * 1000.0;
                metrics.detection_updates += 1;
                metrics.broad_phase_rebuilds += usize::from(contacts.broad_phase_rebuilt);
                metrics.contact_profile += contacts.profile;
            }
            clipped = 0;
            let mut residual = 0.0;
            for color in &colors {
                let updates: Vec<_> = color
                    .par_iter()
                    .map(|v| {
                        let v = *v;
                        if self.garment.mesh.pins[v].is_some() {
                            return (v, x[v], false, 0.0);
                        }
                        let (accepted, truncated, residual) = self.local_step(
                            v,
                            &x,
                            &contacts,
                            settings,
                            grab,
                            (target[v], inertia[v]),
                        );
                        (v, accepted, truncated, residual)
                    })
                    .collect();
                for (v, p, c, r) in updates {
                    x[v] = p;
                    clipped += usize::from(c);
                    residual += r;
                }
            }
            metrics.truncated_vertices += clipped;
            metrics.residual_rms_n = Some((residual / (3 * n) as f64).sqrt());
        }
        metrics.seam_blocks = self.refine_seams(&mut x, state, &contacts, settings, grab);
        if x.iter().any(|v| !v.is_finite()) {
            return Err("Non-finite VBD result; state unchanged".into());
        }
        // Final bound feasibility is an invariant, not a post-Newton correction.
        if (0..n).any(|i| !contacts.contains(i, x[i], 1e-10)) {
            return Err("OGC trust-region invariant failed".into());
        }
        for (i, position) in x.iter().take(n).enumerate() {
            state.v[i] = (*position - state.x[i]) / dt;
            state.x[i] = *position;
        }
        state.time += dt;
        self.prediction.record(&old_velocity, state, settings);
        metrics.motion = Some(self.motion.record(&old_velocity, state, dt, &self.garment));
        metrics.time_s = state.time;
        metrics.contacts = contacts.active_contacts;
        metrics.contact_terms = contacts.contacts.len();
        metrics.minimum_distance_m = contacts.minimum_distance;
        metrics.directional_vertices = contacts.directional.vertex_count();
        metrics.max_seam_gap_m = self.garment.max_seam_gap(&state.x);
        [metrics.extension_p95_percent, metrics.extension_max_percent] =
            crate::garment::diagnostics::extension_percent(&self.garment, state);
        metrics.step_ms = start.elapsed().as_secs_f64() * 1000.0;
        metrics.solve_ms = metrics.step_ms - metrics.detection_ms;
        Ok(metrics)
    }
}

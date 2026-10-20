//! Hybrid backend: conservative GPU contact search, f64 CPU geometry/bounds,
//! f32 GPU VBD local solves. Timings include transfers and readback.
use crate::{
    model::State,
    ogc::potential::Kind,
    vbd::{GpuStepProfile, Grab, VbdMetrics, VbdSettings, VbdSolver},
};
use eframe::wgpu;
mod cache;
pub mod contact_filter;
use crate::garment::CompiledGarment;
use cache::Resources;
use glam::DVec3;
use std::cell::RefCell;
use std::{sync::Arc, time::Instant};
pub struct GpuVbd {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    predict: wgpu::ComputePipeline,
    probe: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    pub check_gradients: bool,
    pub check_contacts: bool,
    subgroups: bool,
    resources: RefCell<Resources>,
    prepared: RefCell<Option<Prepared>>,
    packing: RefCell<Packing>,
    contact_filter: RefCell<contact_filter::GpuContactFilter>,
    pub filter_contacts: bool,
}
#[derive(Clone, Copy)]
#[repr(C)]
struct Term {
    ids: [u32; 4],
    info: [u32; 4],
    a: [f32; 4],
    b: [f32; 4],
    c: [f32; 4],
}
// All fields are 4-lane arrays; repr(C) has no padding and the WGSL stride is 80.
unsafe impl bytemuck::Zeroable for Term {}
unsafe impl bytemuck::Pod for Term {}

/// Per-step upload staging on the CPU. Lengths reset every step, capacity stays
/// reusable across steps and grows/shrinks independently of the active topology.
#[derive(Default)]
struct Packing {
    terms: Vec<Term>,
    ranges: Vec<[u32; 4]>,
    incident: Vec<u32>,
    positions: Vec<[f32; 4]>,
    targets: Vec<[f32; 4]>,
    balls: Vec<[f32; 8]>,
    order: Vec<u32>,
    configs: Vec<[u32; 4]>,
    config_data: Vec<u8>,
}
impl Term {
    fn new(ids: &[usize], kind: u32) -> Self {
        let mut t = Self {
            ids: [0; 4],
            info: [kind, ids.len() as u32, 0, 0],
            a: [0.; 4],
            b: [0.; 4],
            c: [0.; 4],
        };
        for (i, v) in ids.iter().enumerate() {
            t.ids[i] = *v as u32;
        }
        t
    }
}
impl GpuVbd {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let entries: Vec<_> = (0..9)
            .map(|i| wgpu::BindGroupLayoutEntry {
                binding: i,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: if i == 7 {
                        wgpu::BufferBindingType::Uniform
                    } else {
                        wgpu::BufferBindingType::Storage {
                            read_only: i != 0 && i != 8,
                        }
                    },
                    has_dynamic_offset: i == 7,
                    min_binding_size: None,
                },
                count: None,
            })
            .collect();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("VBD layout"),
            entries: &entries,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("VBD pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let info = device.adapter_info();
        let subgroups = device.features().contains(wgpu::Features::SUBGROUP)
            && info.subgroup_min_size == 32
            && info.subgroup_max_size == 32;
        let source = format!(
            "{}\n{}",
            include_str!("vbd.wgsl").replace(
                "@builtin(local_invocation_index)",
                if subgroups {
                    "@builtin(subgroup_invocation_id)"
                } else {
                    "@builtin(local_invocation_index)"
                },
            ),
            if subgroups {
                include_str!("gpu_vbd/reduce_subgroup.wgsl")
            } else {
                include_str!("gpu_vbd/reduce_portable.wgsl")
            }
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("VBD + OGC compute"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("VBD local solve"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("iterate"),
            compilation_options: Default::default(),
            cache: None,
        });
        let probe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("VBD force validation"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("probe"),
            compilation_options: Default::default(),
            cache: None,
        });
        let predict = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("VBD conservative inertial prediction"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("predict"),
            compilation_options: Default::default(),
            cache: None,
        });
        let resources = RefCell::new(Resources::new(&device));
        Self {
            contact_filter: RefCell::new(contact_filter::GpuContactFilter::new(
                device.clone(),
                queue.clone(),
            )),
            filter_contacts: true,
            subgroups,
            resources,
            prepared: RefCell::new(None),
            packing: RefCell::new(Packing::default()),
            device,
            queue,
            pipeline,
            predict,
            probe,
            layout,
            check_gradients: false,
            check_contacts: false,
        }
    }
    pub fn step(
        &self,
        solver: &mut VbdSolver,
        state: &mut State,
        settings: &VbdSettings,
        grab: Option<&Grab>,
    ) -> Result<VbdMetrics, String> {
        if solver.world.experimental_interior_edges || solver.world.experimental_primitive_distances
        {
            return Err("Experimental contact laws are CPU-only".into());
        }
        let start = Instant::now();
        let mut metrics = self.step_inner(solver, state, settings, grab)?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.;
        if let Some(profile) = &mut metrics.gpu_profile {
            profile.cleanup_ms = (elapsed - metrics.step_ms).max(0.);
        }
        metrics.step_ms = elapsed;
        metrics.solve_ms = elapsed - metrics.detection_ms;
        Ok(metrics)
    }
    fn step_inner(
        &self,
        solver: &mut VbdSolver,
        state: &mut State,
        settings: &VbdSettings,
        grab: Option<&Grab>,
    ) -> Result<VbdMetrics, String> {
        settings.validate()?;
        let start = Instant::now();
        let n = state.x.len();
        let x = solver.world.positions(&state.x);
        if n != solver.world.moving
            || !state.time.is_finite()
            || state.v.len() != n
            || x.iter().chain(&state.v).any(|p| !p.is_finite())
        {
            return Err("Invalid GPU VBD state".into());
        }
        if let Some(g) = grab
            && (g.vertices.iter().any(|i| *i >= n)
                || !g.target.is_finite()
                || !g.stiffness.is_finite()
                || g.stiffness <= 0.
                || g.weights.iter().any(|w| !w.is_finite() || *w < 0.)
                || (g.weights.iter().sum::<f64>() - 1.).abs() > 1e-8
                || !g.valid_patch(n))
        {
            return Err("Invalid GPU mouse attachment".into());
        }
        let dt = settings.dt;
        // Work near the cloth, not the world origin: f32 contact gaps must not
        // lose precision merely because the garment sits at shoulder height.
        let origin = state.x.iter().copied().sum::<DVec3>() / n as f64;
        let old_velocity = state.v.clone();
        let guess = solver.prediction.guess(state, settings);
        let damp = 1. + settings.damping * dt;
        let motion = state
            .v
            .iter()
            .map(|v| ((*v * dt + settings.gravity * dt * dt) / damp).length())
            .fold(0., f64::max);
        // A bounded query controls broad-phase work. This can slow convergence,
        // never enlarge the allowable movement beyond a conservative ball.
        let rq = (settings.contact.radius + motion)
            .clamp(settings.contact.radius * 2., settings.contact.radius * 4.);
        let contacts = if self.filter_contacts {
            solver.world.detect_with_filter(
                &x,
                rq,
                &settings.contact,
                &mut *self.contact_filter.borrow_mut(),
            )?
        } else {
            solver.world.detect(&x, rq, &settings.contact)?
        };
        if self.check_contacts {
            let reference = solver.world.detect(&x, rq, &settings.contact)?;
            let keys = |c: &crate::ogc::ContactSet| {
                c.contacts
                    .iter()
                    .map(|c| (c.kind, c.vertices, c.count, c.mollifier_epsilon.to_bits()))
                    .collect::<Vec<_>>()
            };
            if contacts.radii != reference.radii
                || contacts.directional != reference.directional
                || contacts.incident != reference.incident
                || contacts.minimum_distance != reference.minimum_distance
                || contacts.active_contacts != reference.active_contacts
                || contacts.vertex_face_candidates != reference.vertex_face_candidates
                || contacts.edge_edge_candidates != reference.edge_edge_candidates
                || keys(&contacts) != keys(&reference)
            {
                return Err(
                    "GPU contact search differs from the complete f64 detector; state unchanged"
                        .into(),
                );
            }
        }
        if contacts.minimum_distance <= 1e-10 {
            return Err("OGC separation is numerically unresolved; state unchanged. Inspect or rearrange the touching region".into());
        }
        let color_start = Instant::now();
        let colors = solver.colors(&contacts, grab);
        let mut profile = GpuStepProfile {
            coloring_ms: color_start.elapsed().as_secs_f64() * 1000.,
            colors: colors.len(),
            ..Default::default()
        };
        let detection_ms = start.elapsed().as_secs_f64() * 1000.;
        let packing_start = Instant::now();
        let g = &solver.garment;
        // Keep tiny isotropic balls and sparse separating-plane constraints
        // exact in f64. Other vertices retain the original GPU path. A larger
        // directional radius is valid only together with all its half-spaces.
        let precision: Vec<_> = (0..n)
            .filter(|&i| {
                g.mesh.pins[i].is_none()
                    && (contacts.radii[i] < 4. * roundoff_margin(contacts.centers[i] - origin)
                        || !contacts.directional.at(i).is_empty())
            })
            .collect();
        let mut on_cpu = vec![false; n];
        for &i in &precision {
            on_cpu[i] = true;
        }
        let mut prepared = self.prepared.borrow_mut();
        let rest_changed = prepared
            .as_ref()
            .is_none_or(|p| !Arc::ptr_eq(&p.garment, g));
        if rest_changed {
            *prepared = Some(Prepared::new(g.clone()));
        }
        let prepared = prepared.as_ref().unwrap();
        let mut packing = self.packing.borrow_mut();
        let Packing {
            terms,
            ranges,
            incident,
            positions,
            targets,
            balls,
            order,
            configs,
            config_data,
        } = &mut *packing;
        terms.clear();
        ranges.clear();
        incident.clear();
        positions.clear();
        targets.clear();
        balls.clear();
        order.clear();
        configs.clear();
        terms.reserve(contacts.contacts.len() + usize::from(grab.is_some()));
        for c in &contacts.contacts {
            let kind = match c.kind {
                Kind::PointPoint => 10,
                Kind::PointEdge => 11,
                Kind::PointFace => 12,
                Kind::EdgeEdge => 13,
                Kind::EdgeEdgeInterior | Kind::PointTriangle | Kind::SegmentSegment => {
                    return Err("Experimental contact laws are CPU-only".into());
                }
            };
            let mut t = Term::new(&c.vertices[..c.count], kind);
            t.a = [
                settings.contact.radius as f32,
                settings.contact.stiffness as f32,
                c.mollifier_epsilon as f32,
                0.,
            ];
            terms.push(t);
        }
        if let Some(grab) = grab {
            let mut t = Term::new(&grab.vertices, 2);
            for i in 0..3 {
                t.a[i] = grab.weights[i] as f32;
            }
            t.b[0] = grab.point_stiffness() as f32;
            let target = grab.target - origin;
            t.c = [target.x as f32, target.y as f32, target.z as f32, 1.];
            terms.push(t);
        }
        // Independent brush springs reuse the existing implicit grab kernel.
        // Single-vertex terms add no coloring dependencies.
        if let Some(grab) = grab {
            for a in &grab.patch {
                let mut t = Term::new(&[a.vertex], 2);
                t.a[0] = 1.;
                t.b[0] = (grab.patch_stiffness() * a.weight) as f32;
                let target = grab.target + a.offset - origin;
                t.c = [target.x as f32, target.y as f32, target.z as f32, 1.];
                terms.push(t);
            }
        }
        ranges.reserve(n);
        incident.reserve(contacts.contacts.len() * 4 + 12);
        // Detection already built this incidence in canonical contact order.
        // Reuse it instead of allocating and filling another vector per vertex.
        for (v, contacts_at_vertex) in contacts.incident.iter().enumerate() {
            let start = (prepared.incident.len() + incident.len()) as u32;
            incident.extend(
                contacts_at_vertex
                    .iter()
                    .map(|i| (prepared.terms.len() + i) as u32),
            );
            if let Some(grab) = grab {
                for &vertex in &grab.vertices {
                    if vertex == v {
                        incident.push((prepared.terms.len() + contacts.contacts.len()) as u32);
                    }
                }
                if let Ok(index) = grab.patch.binary_search_by_key(&v, |a| a.vertex) {
                    incident
                        .push((prepared.terms.len() + contacts.contacts.len() + 1 + index) as u32);
                }
            }
            ranges.push([
                prepared.ranges[v][0] as u32,
                prepared.ranges[v][1] as u32,
                start,
                (prepared.incident.len() + incident.len()) as u32,
            ]);
        }
        positions.extend(x.iter().enumerate().map(|(i, p)| {
            let p = *p - origin;
            [
                p.x as f32,
                p.y as f32,
                p.z as f32,
                if i >= n || g.mesh.pins[i].is_some() || on_cpu[i] {
                    1.
                } else {
                    0.
                },
            ]
        }));
        targets.extend((0..n).map(|i| {
            let t = state.x[i] - origin + (state.v[i] * dt + settings.gravity * dt * dt) / damp;
            [
                t.x as f32,
                t.y as f32,
                t.z as f32,
                (g.mesh.masses[i] * damp / (dt * dt)) as f32,
            ]
        }));
        balls.extend((0..n).map(|i| {
            let p = contacts.centers[i] - origin;
            let roundoff = roundoff_margin(p);
            let guess = guess[i] - origin;
            [
                p.x as f32,
                p.y as f32,
                p.z as f32,
                (contacts.radii[i] - roundoff).max(0.) as f32,
                guess.x as f32,
                guess.y as f32,
                guess.z as f32,
                0.,
            ]
        }));
        for color in &colors {
            let offset = order.len();
            // A tiny contact may round to zero in f32. All its moving vertices
            // have tiny original balls and are handled exactly on the CPU.
            // Do not evaluate their singular rounded objective on the GPU.
            order.extend(
                color
                    .iter()
                    .filter(|&&i| !on_cpu[i] && g.mesh.pins[i].is_none())
                    .map(|i| *i as u32),
            );
            if order.len() > offset {
                configs.push([offset as u32, (order.len() - offset) as u32, n as u32, 0]);
            }
        }
        if configs.is_empty() {
            configs.push([0, 0, n as u32, 0]);
        }
        let alignment = self.device.limits().min_uniform_buffer_offset_alignment as usize;
        config_data.resize(alignment * configs.len(), 0);
        for (i, c) in configs.iter().enumerate() {
            config_data[i * alignment..i * alignment + 16].copy_from_slice(bytemuck::cast_slice(c));
        }
        profile.packing_ms = packing_start.elapsed().as_secs_f64() * 1000.;
        let upload_start = Instant::now();
        let mut resources = self.resources.borrow_mut();
        resources.prepare(
            &self.device,
            &self.queue,
            &self.layout,
            [
                bytemuck::cast_slice(positions),
                bytemuck::cast_slice(targets),
                bytemuck::cast_slice(ranges),
                bytemuck::cast_slice(terms),
                bytemuck::cast_slice(incident),
                bytemuck::cast_slice(balls),
                bytemuck::cast_slice(order),
                config_data,
            ],
            (
                n,
                bytemuck::cast_slice(&prepared.terms),
                bytemuck::cast_slice(&prepared.incident),
                rest_changed,
            ),
        );
        let buffers = &resources.buffers;
        let bind = resources.bind.as_ref().unwrap();
        profile.upload_ms = upload_start.elapsed().as_secs_f64() * 1000.;
        if self.check_gradients {
            use crate::math::Jet;
            let read = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("force checks"),
                size: (n * 16) as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let mut e = self.device.create_command_encoder(&Default::default());
            {
                let mut p = e.begin_compute_pass(&Default::default());
                p.set_pipeline(&self.probe);
                p.set_bind_group(0, bind, &[0]);
                p.dispatch_workgroups(n as u32, 1, 1);
            }
            e.copy_buffer_to_buffer(&buffers[8], 0, &read, 0, (n * 16) as u64);
            let submission = self.queue.submit([e.finish()]);
            let (tx, rx) = std::sync::mpsc::channel();
            read.map_async(wgpu::MapMode::Read, .., move |r| {
                let _ = tx.send(r);
            });
            self.device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(submission),
                    timeout: None,
                })
                .map_err(|e| e.to_string())?;
            rx.recv()
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            let data = read.get_mapped_range(..).map_err(|e| e.to_string())?;
            let probes: &[[f32; 4]] = bytemuck::cast_slice(&data);
            // Compare at IDENTICAL rounded geometry/targets, so this checks the
            // GPU formulas, not the f32 quantization of a stiff rest state.
            let rounded: Vec<_> = positions
                .iter()
                .map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64) + origin)
                .collect();
            let mut max_error = 0.0_f64;
            for i in 0..n {
                let j = solver.local(
                    i,
                    std::array::from_fn(|k| Jet::<3>::variable(rounded[i][k], k)),
                    &rounded,
                    &contacts,
                    settings,
                    grab,
                );
                let target = DVec3::new(
                    targets[i][0] as f64,
                    targets[i][1] as f64,
                    targets[i][2] as f64,
                ) + origin;
                let g = DVec3::from_array(j.g) + (rounded[i] - target) * targets[i][3] as f64;
                let actual = DVec3::new(
                    probes[i][0] as f64,
                    probes[i][1] as f64,
                    probes[i][2] as f64,
                );
                let error = (actual - g).length();
                if !error.is_finite() || error > 0.003 + 0.003 * g.length() {
                    return Err(format!(
                        "GPU gradient mismatch at {i}: {actual:?} vs {g:?}, error {error}"
                    ));
                }
                max_error = max_error.max(error);
                let energy =
                    j.v + 0.5 * targets[i][3] as f64 * (rounded[i] - target).length_squared();
                if (energy - probes[i][3] as f64).abs() > 1e-5 + 0.001 * energy.abs() {
                    return Err(format!("GPU energy mismatch at {i}"));
                }
            }
            eprintln!(
                "GPU/CPU local gradients verified: {n} vertices, maximum absolute error {max_error:.3e} N"
            );
            drop(data);
            read.unmap();
        }
        let query = &resources.query;
        let resolve = &resources.resolve;
        let read = &resources.read;
        let encode_start = Instant::now();
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("VBD iterations"),
                timestamp_writes: query.as_ref().map(|q| wgpu::ComputePassTimestampWrites {
                    query_set: q,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                }),
            });
            // Same warm start as CPU VBD, clipped to the original detection
            // balls (including the f32 safety margin). Never reset their centers.
            pass.set_pipeline(&self.predict);
            pass.set_bind_group(0, bind, &[0]);
            pass.dispatch_workgroups((n as u32).div_ceil(64), 1, 1);
            pass.set_pipeline(&self.pipeline);
            for _ in 0..settings.iterations {
                for (i, c) in configs.iter().enumerate() {
                    pass.set_bind_group(0, bind, &[(i * alignment) as u32]);
                    pass.dispatch_workgroups(c[1], 1, 1);
                }
            }
        }
        encoder.copy_buffer_to_buffer(&buffers[0], 0, read, 0, (n * 16) as u64);
        if let (Some(q), Some(resolve)) = (query, resolve) {
            encoder.resolve_query_set(q, 0..2, resolve, 0);
            encoder.copy_buffer_to_buffer(resolve, 0, read, (n * 16) as u64, 16);
        }
        let commands = encoder.finish();
        profile.encode_ms = encode_start.elapsed().as_secs_f64() * 1000.;
        let wait_start = Instant::now();
        let submission = self.queue.submit([commands]);
        let (tx, rx) = std::sync::mpsc::channel();
        read.map_async(wgpu::MapMode::Read, .., move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: None,
            })
            .map_err(|e| e.to_string())?;
        rx.recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        profile.submit_wait_ms = wait_start.elapsed().as_secs_f64() * 1000.;
        let post_start = Instant::now();
        let mapped = read.get_mapped_range(..).map_err(|e| e.to_string())?;
        let gpu_compute_ms = query.as_ref().map(|_| {
            let a = u64::from_le_bytes(mapped[n * 16..n * 16 + 8].try_into().unwrap());
            let b = u64::from_le_bytes(mapped[n * 16 + 8..n * 16 + 16].try_into().unwrap());
            b.wrapping_sub(a) as f64 * self.queue.get_timestamp_period() as f64 / 1e6
        });
        let values: &[[f32; 4]] = bytemuck::cast_slice(&mapped[..n * 16]);
        let mut new: Vec<_> = values
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if on_cpu[i] || g.mesh.pins[i].is_some() {
                    state.x[i]
                } else {
                    DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64) + origin
                }
            })
            .collect();
        drop(mapped);
        read.unmap();
        for (i, p) in new.iter().enumerate() {
            if !contacts.contains(i, *p, 0.) {
                return Err("GPU OGC trust-region check failed; state unchanged".into());
            }
        }
        let precision_start = Instant::now();
        if !precision.is_empty() {
            let mut world_x = solver.world.positions(&new);
            // The same adaptive warm start as the GPU, restricted to the
            // original convex region. Without it, a whole sparse patch can
            // lose its common translational mode in finite local iterations.
            for &i in &precision {
                if !contacts.directional.at(i).is_empty() {
                    world_x[i] = contacts.truncate(i, guess[i]).0;
                }
            }
            solver.refine_precision_vertices(
                &mut world_x,
                state,
                &contacts,
                settings,
                grab,
                &precision,
            );
            for &i in &precision {
                let p = world_x[i];
                if !contacts.contains(i, p, 1e-12) {
                    return Err("f64 OGC trust-region check failed; state unchanged".into());
                }
                new[i] = p;
            }
        }
        let precision_ms = precision_start.elapsed().as_secs_f64() * 1000.;
        let seam_blocks = if solver.seam_block_sweeps > 0 {
            let mut world_x = solver.world.positions(&new);
            let metrics = solver.refine_seams(&mut world_x, state, &contacts, settings, grab);
            new.copy_from_slice(&world_x[..n]);
            if new
                .iter()
                .enumerate()
                .any(|(i, p)| !contacts.contains(i, *p, 0.))
            {
                return Err("Seam block OGC trust-region check failed; state unchanged".into());
            }
            metrics
        } else {
            Default::default()
        };
        for (i, position) in new.iter().enumerate() {
            if g.mesh.pins[i].is_none() {
                state.v[i] = (*position - state.x[i]) / dt;
                state.x[i] = *position;
            } else {
                state.v[i] = DVec3::ZERO;
            }
        }
        state.time += dt;
        solver.prediction.record(&old_velocity, state, settings);
        let motion_metrics = solver.motion.record(&old_velocity, state, dt, g);
        let [extension_p95_percent, extension_max_percent] =
            crate::garment::diagnostics::extension_percent(g, state);
        let max_seam_gap_m = g.max_seam_gap(&state.x);
        profile.post_ms = post_start.elapsed().as_secs_f64() * 1000.;
        let step_ms = start.elapsed().as_secs_f64() * 1000.;
        Ok(VbdMetrics {
            seam_blocks,
            motion: Some(motion_metrics),
            time_s: state.time,
            dt_s: dt,
            step_ms,
            detection_ms,
            contact_profile: contacts.profile,
            solve_ms: step_ms - detection_ms,
            gpu_compute_ms,
            gpu_profile: Some(profile),
            gpu_subgroups: self.subgroups,
            precision_vertices: precision.len(),
            precision_ms,
            directional_vertices: contacts.directional.vertex_count(),
            iterations: settings.iterations,
            contacts: contacts.active_contacts,
            contact_terms: contacts.contacts.len(),
            detection_updates: 1,
            broad_phase_rebuilds: usize::from(contacts.broad_phase_rebuilt),
            minimum_distance_m: contacts.minimum_distance,
            max_seam_gap_m,
            extension_p95_percent,
            extension_max_percent,
            ..Default::default()
        })
    }
}

fn roundoff_margin(local_center: DVec3) -> f64 {
    16. * f32::EPSILON as f64 * local_center.abs().max_element().max(1.)
}

/// Rest geometry/material packing and static incidence survive time steps. The
/// retained Arc prevents in-place mutation and pointer reuse; remeshing or a new
/// garment automatically invalidates the cache.
struct Prepared {
    garment: Arc<CompiledGarment>,
    terms: Vec<Term>,
    ranges: Vec<[usize; 2]>,
    incident: Vec<u32>,
}
impl Prepared {
    fn new(garment: Arc<CompiledGarment>) -> Self {
        let g = &garment;
        let mut terms = Vec::new();
        for tri in &g.mesh.triangles {
            let m = &g.materials[tri.material];
            let (s, c) = m.grain_degrees.to_radians().sin_cos();
            let u = tri.inv_dm.x_axis * c + tri.inv_dm.y_axis * s;
            let v = -tri.inv_dm.x_axis * s + tri.inv_dm.y_axis * c;
            let mut t = Term::new(&tri.vertices, 0);
            t.a = [(-u.x - u.y) as f32, u.x as f32, u.y as f32, tri.area as f32];
            t.b = [(-v.x - v.y) as f32, v.x as f32, v.y as f32, m.warp as f32];
            t.c = [
                m.weft as f32,
                m.shear as f32,
                m.compression_ratio as f32,
                m.compression_transition as f32,
            ];
            terms.push(t);
        }
        for h in &g.mesh.hinges {
            let m = &g.materials[h.material];
            let (s, c) = m.grain_degrees.to_radians().sin_cos();
            let aw = h.across.x * c + h.across.y * s;
            let af = -h.across.x * s + h.across.y * c;
            let mut t = Term::new(&h.vertices, 1);
            t.a[0] = (h.weight * (m.bend_warp * aw * aw + m.bend_weft * af * af)) as f32;
            terms.push(t);
        }
        for seam in &g.seams {
            for s in &seam.samples {
                let mut t = Term::new(
                    &s.point.terms.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
                    2,
                );
                for (i, (_, w)) in s.point.terms.iter().enumerate() {
                    t.a[i] = *w as f32;
                }
                t.b[0] = (s.weight_m * seam.stiffness) as f32;
                terms.push(t);
            }
        }

        let mut adjacency = vec![Vec::new(); g.mesh.uv.len()];
        for (i, t) in terms.iter().enumerate() {
            for v in &t.ids[..t.info[1] as usize] {
                adjacency[*v as usize].push(i as u32);
            }
        }
        let mut incident = Vec::new();
        let mut ranges = Vec::new();
        for row in adjacency {
            let start = incident.len();
            incident.extend(row);
            ranges.push([start, incident.len()]);
        }
        Self {
            garment,
            terms,
            ranges,
            incident,
        }
    }
}

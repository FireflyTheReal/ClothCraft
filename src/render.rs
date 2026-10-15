//! A depth-buffered wgpu viewport. CPU FEM is entirely independent of this module.
use crate::{
    avatar::SurfaceMesh,
    contact::{Collider, Sphere},
    energy,
    garment::seam::CompiledSeam,
    model::{Material, Mesh, State},
    project::Project,
};
use eframe::{egui, egui_wgpu::RenderState, wgpu};
use glam::{Mat4, Vec3};

pub struct DrawModel<'a> {
    pub mesh: &'a Mesh,
    pub state: &'a State,
    pub materials: &'a [Material],
    pub sphere: Option<&'a Sphere>,
    pub avatar: Option<&'a SurfaceMesh>,
    pub collider: Option<&'a dyn Collider>,
    pub seams: &'a [CompiledSeam],
}
struct Timers {
    query: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
}

pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: 0.55,
            pitch: 0.4,
            distance: 2.5,
            target: Vec3::new(0.0, 0.4, 0.0),
        }
    }
}
impl Camera {
    pub fn matrix(&self, aspect: f32) -> Mat4 {
        let eye = self.target
            + self.distance
                * Vec3::new(
                    self.yaw.sin() * self.pitch.cos(),
                    self.pitch.sin(),
                    self.yaw.cos() * self.pitch.cos(),
                );
        // WebGPU uses a [0,1] depth range and Y-up NDC on every backend.
        glam::camera::rh::proj::directx::perspective(45.0_f32.to_radians(), aspect, 0.01, 100.0)
            * glam::camera::rh::view::look_at_mat4(eye, self.target, Vec3::Y)
    }
    pub fn interact(&mut self, response: &egui::Response) {
        let delta = response.ctx.input(|i| i.pointer.delta());
        if response.dragged_by(egui::PointerButton::Primary) {
            self.yaw -= delta.x * 0.008;
            self.pitch = (self.pitch + delta.y * 0.008).clamp(-1.45, 1.45);
        }
        if response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Middle)
        {
            let right = Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin());
            let up = Vec3::new(
                -self.yaw.sin() * self.pitch.sin(),
                self.pitch.cos(),
                -self.yaw.cos() * self.pitch.sin(),
            );
            self.target += (-delta.x * right + delta.y * up) * self.distance * 0.0015;
        }
        if response.hovered() {
            let scroll = response.ctx.input(|i| i.smooth_scroll_delta.y);
            self.distance = (self.distance * (-scroll * 0.002).exp()).clamp(0.15, 30.0);
        }
        if response.double_clicked() {
            *self = Self::default();
        }
    }
    pub fn garment_interact(&mut self, response: &egui::Response) {
        let delta = response.ctx.input(|i| i.pointer.delta());
        if response.dragged_by(egui::PointerButton::Secondary) {
            self.yaw -= delta.x * 0.008;
            self.pitch = (self.pitch + delta.y * 0.008).clamp(-1.45, 1.45);
        }
        if response.dragged_by(egui::PointerButton::Middle) {
            let right = Vec3::new(self.yaw.cos(), 0., -self.yaw.sin());
            let up = Vec3::new(
                -self.yaw.sin() * self.pitch.sin(),
                self.pitch.cos(),
                -self.yaw.cos() * self.pitch.sin(),
            );
            self.target += (-delta.x * right + delta.y * up) * self.distance * 0.0015;
        }
        if response.hovered() {
            let scroll = response.ctx.input(|i| i.smooth_scroll_delta.y);
            self.distance = (self.distance * (-scroll * 0.002).exp()).clamp(0.15, 30.);
        }
    }
    pub fn ray(&self, position: egui::Pos2, rect: egui::Rect) -> (glam::DVec3, glam::DVec3) {
        let inv = self.matrix(rect.width() / rect.height()).inverse();
        let u = 2. * (position.x - rect.left()) / rect.width() - 1.;
        let v = 1. - 2. * (position.y - rect.top()) / rect.height();
        let a = inv * glam::Vec4::new(u, v, 0., 1.);
        let b = inv * glam::Vec4::new(u, v, 1., 1.);
        let origin = (a.truncate() / a.w).as_dvec3();
        let far = (b.truncate() / b.w).as_dvec3();
        (origin, (far - origin).normalize())
    }
}

pub struct Viewport {
    render_state: RenderState,
    pipeline: wgpu::RenderPipeline,
    color: wgpu::TextureView,
    color_texture: wgpu::Texture,
    depth: wgpu::TextureView,
    size: [u32; 2],
    texture_id: egui::TextureId,
    vertices: wgpu::Buffer,
    capacity: u64,
    timers: Option<Timers>,
    view_uniform: wgpu::Buffer,
    view_bind: wgpu::BindGroup,
    avatar_buffer: Option<wgpu::Buffer>,
    avatar_key: (usize, usize),
    avatar_count: u32,
}
impl Viewport {
    fn target(device: &wgpu::Device, size: [u32; 2], depth: bool) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(if depth { "cloth-depth" } else { "cloth-color" }),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: if depth {
                wgpu::TextureFormat::Depth32Float
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            },
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }
    pub fn new(render_state: RenderState) -> Self {
        let device = &render_state.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cloth-lit-wireframe"),
            source: wgpu::ShaderSource::Wgsl(include_str!("cloth.wgsl").into()),
        });
        let attrs = wgpu::vertex_attr_array![0=>Float32x4,1=>Float32x3,2=>Float32x4,3=>Float32x3];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cloth-viewport"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 56,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attrs,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let color_texture = Self::target(device, [1, 1], false);
        let view_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera matrix"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let view_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: view_uniform.as_entire_binding(),
            }],
        });
        let color = color_texture.create_view(&Default::default());
        let depth = Self::target(device, [1, 1], true).create_view(&Default::default());
        let texture_id = render_state.renderer.write().register_native_texture(
            device,
            &color,
            wgpu::FilterMode::Linear,
        );
        let capacity = 1024;
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viewport-vertices"),
            size: capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            render_state,
            pipeline,
            color,
            color_texture,
            depth,
            size: [1, 1],
            texture_id,
            vertices,
            capacity,
            timers: None,
            view_uniform,
            view_bind,
            avatar_buffer: None,
            avatar_key: (0, 0),
            avatar_count: 0,
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        size: [u32; 2],
        camera: &Camera,
        mesh: &Mesh,
        state: &State,
        p: &Project,
        wire: bool,
        heat: bool,
        contacts: bool,
    ) -> egui::TextureId {
        self.draw_scene(
            size,
            camera,
            DrawModel {
                mesh,
                state,
                materials: std::slice::from_ref(&p.material),
                sphere: p.sphere.as_ref(),
                avatar: None,
                collider: p.sphere.as_ref().map(|s| s as &dyn Collider),
                seams: &[],
            },
            wire,
            heat,
            contacts,
        )
    }
    pub fn draw_scene(
        &mut self,
        size: [u32; 2],
        camera: &Camera,
        model: DrawModel<'_>,
        wire: bool,
        heat: bool,
        contacts: bool,
    ) -> egui::TextureId {
        let DrawModel {
            mesh,
            state,
            materials,
            sphere,
            avatar,
            collider,
            seams,
        } = model;
        let device = &self.render_state.device;
        let max = device.limits().max_texture_dimension_2d.min(4096);
        let size = size.map(|s| s.clamp(1, max));
        if size != self.size {
            self.color_texture = Self::target(device, size, false);
            self.color = self.color_texture.create_view(&Default::default());
            self.depth = Self::target(device, size, true).create_view(&Default::default());
            self.size = size;
            self.render_state
                .renderer
                .write()
                .update_egui_texture_from_wgpu_texture(
                    device,
                    &self.color,
                    wgpu::FilterMode::Linear,
                    self.texture_id,
                );
        }
        let matrix = camera.matrix(size[0] as f32 / size[1] as f32);
        self.render_state.queue.write_buffer(
            &self.view_uniform,
            0,
            bytemuck::cast_slice(&matrix.to_cols_array()),
        );
        let mut data = Vec::<f32>::new();
        let mut smooth = vec![Vec3::ZERO; state.x.len()];
        for t in &mesh.triangles {
            let p = t.vertices.map(|i| state.x[i].as_vec3());
            let n = (p[1] - p[0]).cross(p[2] - p[0]);
            for i in t.vertices {
                smooth[i] += n;
            }
        }
        for n in &mut smooth {
            *n = n.normalize_or_zero();
        }
        let mut triangle =
            |points: [Vec3; 3], color: [f32; 3], wire: bool, normals: Option<[Vec3; 3]>| {
                let normal = (points[1] - points[0])
                    .cross(points[2] - points[0])
                    .normalize_or_zero();
                if normal.length_squared() < 0.5 {
                    return;
                }
                for (i, p) in points.into_iter().enumerate() {
                    data.extend_from_slice(&p.extend(1.0).to_array());
                    data.extend_from_slice(&normals.map_or(normal, |n| n[i]).to_array());
                    data.extend_from_slice(&color);
                    data.push(if wire { 1.0 } else { 0.0 });
                    data.extend_from_slice(&[
                        if i == 0 { 1.0 } else { 0.0 },
                        if i == 1 { 1.0 } else { 0.0 },
                        if i == 2 { 1.0 } else { 0.0 },
                    ]);
                }
            };
        for t in &mesh.triangles {
            let material = &materials[t.material];
            let strain = energy::stretches(&state.x, t, material)
                .into_iter()
                .map(|s| (s - 1.0).abs())
                .fold(0.0, f64::max);
            let v = (strain / 0.1).clamp(0.0, 1.0) as f32;
            let contact = contacts
                && collider.is_some_and(|s| {
                    t.vertices
                        .iter()
                        .any(|i| s.penetration(state.x[*i], material.thickness) > 0.0)
                });
            let color = if contact {
                [1.0, 0.2, 0.25]
            } else if heat {
                [0.15 + 0.8 * v, 0.75 * (1.0 - v) + 0.15, 0.95 * (1.0 - v)]
            } else {
                [0.91, 0.92, 0.94]
            };
            triangle(
                t.vertices.map(|i| state.x[i].as_vec3()),
                color,
                wire,
                Some(t.vertices.map(|i| smooth[i])),
            );
        }
        for seam in seams {
            for side in [1.0, -1.0] {
                let points: Vec<_> = seam
                    .probes
                    .iter()
                    .map(|p| {
                        p.terms
                            .iter()
                            .filter(|(_, w)| w.signum() == side)
                            .map(|(i, w)| state.x[*i] * w.abs())
                            .sum::<glam::DVec3>()
                            .as_vec3()
                    })
                    .collect();
                for pair in points.windows(2) {
                    let [a, b] = [pair[0], pair[1]];
                    let d = (b - a).cross(Vec3::Z).normalize_or_zero() * 0.0013;
                    triangle([a - d, a + d, b + d], [1.0, 0.72, 0.22], false, None);
                    triangle([a - d, b + d, b - d], [1.0, 0.72, 0.22], false, None);
                }
            }
        }
        if let Some(s) = sphere {
            let point = |i: usize, j: usize| {
                let theta = std::f32::consts::TAU * i as f32 / 40.0;
                let phi = std::f32::consts::PI * j as f32 / 20.0;
                s.center.as_vec3()
                    + s.radius as f32
                        * Vec3::new(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin())
            };
            for j in 0..20 {
                for i in 0..40 {
                    let [a, b, c, d] = [
                        point(i, j),
                        point(i + 1, j),
                        point(i, j + 1),
                        point(i + 1, j + 1),
                    ];
                    triangle([a, c, b], [0.38, 0.4, 0.46], false, None);
                    triangle([b, c, d], [0.38, 0.4, 0.46], false, None);
                }
            }
        }
        // Visual ground grid, deliberately not represented as a physical collider.
        let y = if avatar.is_some() { -0.005 } else { -0.4 };
        for i in -10..=10 {
            let k = i as f32 * 0.2;
            let d = 0.001;
            for points in [
                [
                    Vec3::new(k - d, y, -2.0),
                    Vec3::new(k + d, y, -2.0),
                    Vec3::new(k + d, y, 2.0),
                ],
                [
                    Vec3::new(k - d, y, -2.0),
                    Vec3::new(k + d, y, 2.0),
                    Vec3::new(k - d, y, 2.0),
                ],
                [
                    Vec3::new(-2.0, y, k - d),
                    Vec3::new(2.0, y, k - d),
                    Vec3::new(2.0, y, k + d),
                ],
                [
                    Vec3::new(-2.0, y, k - d),
                    Vec3::new(2.0, y, k + d),
                    Vec3::new(-2.0, y, k + d),
                ],
            ] {
                triangle(points, [0.18, 0.21, 0.27], false, None);
            }
        }
        // Static avatar stays resident; camera transforms now happen in WGSL.
        if let Some(avatar) = avatar {
            let key = (
                avatar.vertices.as_ptr() as usize,
                avatar.triangles.as_ptr() as usize,
            );
            if key != self.avatar_key {
                use wgpu::util::DeviceExt;
                let mut body = Vec::<f32>::new();
                let mut normals = vec![Vec3::ZERO; avatar.vertices.len()];
                for t in &avatar.triangles {
                    let p = t.map(|i| avatar.vertices[i].as_vec3());
                    let n = (p[1] - p[0]).cross(p[2] - p[0]);
                    for i in t {
                        normals[*i] += n;
                    }
                }
                for n in &mut normals {
                    *n = n.normalize_or_zero();
                }
                for t in &avatar.triangles {
                    let p = t.map(|i| avatar.vertices[i].as_vec3());
                    for (i, v) in p.into_iter().enumerate() {
                        body.extend_from_slice(&v.extend(1.).to_array());
                        body.extend_from_slice(&normals[t[i]].to_array());
                        body.extend_from_slice(&[0.46, 0.47, 0.49, 0., 0., 0., 0.]);
                    }
                }
                self.avatar_count = (body.len() / 14) as u32;
                self.avatar_key = key;
                self.avatar_buffer = Some(device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("static avatar"),
                        contents: bytemuck::cast_slice(&body),
                        usage: wgpu::BufferUsages::VERTEX,
                    },
                ));
            }
        } else {
            self.avatar_count = 0;
            self.avatar_key = (0, 0);
            self.avatar_buffer = None;
        }
        let bytes = bytemuck::cast_slice(&data);
        if bytes.len() as u64 > self.capacity {
            self.capacity = (bytes.len() as u64).next_power_of_two();
            self.vertices = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("viewport-vertices"),
                size: self.capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.render_state
            .queue
            .write_buffer(&self.vertices, 0, bytes);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("cloth-viewport"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cloth-viewport"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Studio backdrop for the workspace viewport: a light
                        // neutral grey that still separates the near-white cloth
                        // and the grey mannequin.
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.70,
                            g: 0.73,
                            b: 0.77,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: self
                    .timers
                    .as_ref()
                    .map(|t| wgpu::RenderPassTimestampWrites {
                        query_set: &t.query,
                        beginning_of_pass_write_index: Some(0),
                        end_of_pass_write_index: Some(1),
                    }),
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.view_bind, &[]);
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.draw(0..(data.len() / 14) as u32, 0..1);
            if let Some(body) = &self.avatar_buffer {
                pass.set_vertex_buffer(0, body.slice(..));
                pass.draw(0..self.avatar_count, 0..1);
            }
        }
        if let Some(t) = &self.timers {
            encoder.resolve_query_set(&t.query, 0..2, &t.resolve, 0);
            encoder.copy_buffer_to_buffer(&t.resolve, 0, &t.read, 0, 16);
        }
        self.render_state.queue.submit([encoder.finish()]);
        self.texture_id
    }
    pub fn enable_timing(&mut self) -> bool {
        let d = &self.render_state.device;
        if !d.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return false;
        }
        self.timers = Some(Timers {
            query: d.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("viewport-timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            }),
            resolve: d.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 16,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            read: d.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 16,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        });
        true
    }
    pub fn gpu_time_ms(&self) -> Result<Option<f64>, String> {
        let Some(t) = &self.timers else {
            return Ok(None);
        };
        let bytes = read_buffer(&self.render_state.device, &t.read)?;
        let start = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let end = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        Ok(Some(
            end.wrapping_sub(start) as f64 * self.render_state.queue.get_timestamp_period() as f64
                / 1e6,
        ))
    }
    pub fn capture(&self, path: &std::path::Path) -> Result<(), String> {
        let row = (self.size[0] * 4).div_ceil(256) * 256;
        let d = &self.render_state.device;
        let buffer = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viewport-image"),
            size: row as u64 * self.size[1] as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = d.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.color_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: self.size[0],
                height: self.size[1],
                depth_or_array_layers: 1,
            },
        );
        self.render_state.queue.submit([encoder.finish()]);
        let data = read_buffer(d, &buffer)?;
        let pixels: Vec<_> = data
            .chunks_exact(row as usize)
            .flat_map(|r| r[..(self.size[0] * 4) as usize].iter().copied())
            .collect();
        image::save_buffer(
            path,
            &pixels,
            self.size[0],
            self.size[1],
            image::ColorType::Rgba8,
        )
        .map_err(|e| e.to_string())
    }
}
fn read_buffer(device: &wgpu::Device, buffer: &wgpu::Buffer) -> Result<Vec<u8>, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.map_async(wgpu::MapMode::Read, .., move |r| {
        let _ = tx.send(r);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| e.to_string())?;
    rx.recv()
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let data = buffer
        .get_mapped_range(..)
        .map_err(|e| e.to_string())?
        .to_vec();
    buffer.unmap();
    Ok(data)
}
impl Drop for Viewport {
    fn drop(&mut self) {
        self.render_state
            .renderer
            .write()
            .free_texture(&self.texture_id);
    }
}

//! Cheap studio background and ground preview, with the same depth/sample count as the mesh.
use super::*;

pub(super) struct Environment {
    pub pipeline: wgpu::RenderPipeline,
    pub bind: wgpu::BindGroup,
    uniform: wgpu::Buffer,
}
impl Environment {
    pub fn new(device: &wgpu::Device, samples: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("studio environment"),
            source: wgpu::ShaderSource::Wgsl(include_str!("environment.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("studio environment"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
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
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: samples,
                ..Default::default()
            },
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("environment settings"),
            size: 208,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("environment"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        Self {
            pipeline,
            bind,
            uniform,
        }
    }
    pub fn update(
        &self,
        queue: &wgpu::Queue,
        matrix: Mat4,
        look: &appearance::Appearance,
        avatar: Option<&SurfaceMesh>,
        state: &State,
    ) {
        let mut data = Vec::with_capacity(52);
        data.extend(matrix.inverse().to_cols_array());
        data.extend(matrix.to_cols_array());
        for color in [
            look.top,
            if look.gradient { look.bottom } else { look.top },
            look.floor_color,
        ] {
            data.extend(color.map(|v| f32::from(v) / 255.));
            data.push(1.);
        }
        let ground_y = if avatar.is_some() { -0.005 } else { -0.4 };
        data.extend([
            ground_y,
            f32::from(look.floor),
            f32::from(look.grid),
            if look.shadow { look.shadow_opacity } else { 0. },
        ]);
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for p in state
            .x
            .iter()
            .chain(avatar.into_iter().flat_map(|a| &a.vertices))
        {
            lo = lo.min(p.as_vec3());
            hi = hi.max(p.as_vec3());
        }
        if !lo.is_finite() || !hi.is_finite() {
            lo = Vec3::splat(-0.2);
            hi = Vec3::splat(0.2);
        }
        let center = (lo + hi) * 0.5;
        let radii = ((hi - lo) * 0.6).max(Vec3::splat(0.15));
        data.extend([center.x, center.z, radii.x, radii.z]);
        queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&data));
    }
}

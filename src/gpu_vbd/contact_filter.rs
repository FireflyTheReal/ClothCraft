//! GPU BVH traversal and separating-axis rejection, with conservative f32 padding. No distances,
//! contact features or OGC movement bounds are accepted from this kernel.
use crate::ogc::filter::{CandidateFilter, PrimitiveCandidates, SpatialQuery};
use eframe::wgpu;
mod broad_phase;

pub struct GpuContactFilter {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    refit_pipeline: wgpu::ComputePipeline,
    broad_capacity: usize,
    spatial_layout_id: u64,
    spatial_origin: glam::DVec3,
    layout: wgpu::BindGroupLayout,
    buffers: Vec<wgpu::Buffer>,
    capacities: Vec<usize>,
    bind: Option<wgpu::BindGroup>,
    read: Option<wgpu::Buffer>,
    read_capacity: usize,
}
impl GpuContactFilter {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let entries: Vec<_> = (0..7)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: if binding == 4 {
                        wgpu::BufferBindingType::Uniform
                    } else {
                        wgpu::BufferBindingType::Storage {
                            read_only: binding != 3 && binding != 5,
                        }
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            })
            .collect();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("OGC candidate filter layout"),
            entries: &entries,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("OGC candidate filter"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("OGC conservative GPU BVH"),
            source: wgpu::ShaderSource::Wgsl(
                format!(
                    "{}\n{}",
                    include_str!("contact_bvh.wgsl"),
                    include_str!("contact_geometry.wgsl")
                )
                .into(),
            ),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("OGC GPU BVH query"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("query_bvh"),
            compilation_options: Default::default(),
            cache: None,
        });
        let refit_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("OGC independent node refit"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("refit_nodes"),
            compilation_options: Default::default(),
            cache: None,
        });
        Self {
            device,
            queue,
            pipeline,
            refit_pipeline,
            broad_capacity: 4096,
            spatial_layout_id: 0,
            spatial_origin: glam::DVec3::ZERO,
            layout,
            buffers: Vec::new(),
            capacities: Vec::new(),
            bind: None,
            read: None,
            read_capacity: 0,
        }
    }
    fn prepare(&mut self, data: [&[u8]; 6], output_size: usize) {
        let mut changed = false;
        for i in 0..7 {
            let bytes = if i == 5 {
                &[][..]
            } else {
                data[if i == 6 { 5 } else { i }]
            };
            let size = if i == 5 { output_size } else { bytes.len() }.max(16);
            if self.capacities.get(i).is_none_or(|&c| c < size) {
                let capacity = size.next_power_of_two();
                let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("OGC persistent filter storage"),
                    size: capacity as u64,
                    usage: (if i == 4 {
                        wgpu::BufferUsages::UNIFORM
                    } else {
                        wgpu::BufferUsages::STORAGE
                    }) | wgpu::BufferUsages::COPY_DST
                        | if i == 5 {
                            wgpu::BufferUsages::COPY_SRC
                        } else {
                            wgpu::BufferUsages::empty()
                        },
                    mapped_at_creation: false,
                });
                if i == self.buffers.len() {
                    self.buffers.push(buffer);
                    self.capacities.push(capacity);
                } else {
                    self.buffers[i] = buffer;
                    self.capacities[i] = capacity;
                }
                changed = true;
            }
            if !bytes.is_empty() {
                self.queue.write_buffer(&self.buffers[i], 0, bytes);
            }
        }
        if changed {
            let entries: Vec<_> = self
                .buffers
                .iter()
                .enumerate()
                .map(|(i, b)| wgpu::BindGroupEntry {
                    binding: i as u32,
                    resource: b.as_entire_binding(),
                })
                .collect();
            self.bind = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("OGC persistent filter bindings"),
                layout: &self.layout,
                entries: &entries,
            }));
        }
        if self.read_capacity < output_size {
            self.read_capacity = output_size.next_power_of_two();
            self.read = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("OGC candidate readback"),
                size: self.read_capacity as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }));
        }
    }
}
impl CandidateFilter for GpuContactFilter {
    fn filter(&mut self, query: SpatialQuery<'_>) -> Result<Option<PrimitiveCandidates>, String> {
        self.spatial_filter(query)
    }
}

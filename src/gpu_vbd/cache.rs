//! Reusable allocations. Capacity grows, but stale tail bytes are never indexed:
//! every dispatch, incidence range and readback is bounded by the current scene.
use eframe::wgpu;

pub(super) struct Resources {
    pub buffers: Vec<wgpu::Buffer>,
    capacities: Vec<usize>,
    pub bind: Option<wgpu::BindGroup>,
    pub read: wgpu::Buffer,
    read_capacity: usize,
    pub query: Option<wgpu::QuerySet>,
    pub resolve: Option<wgpu::Buffer>,
}
impl Resources {
    pub fn new(device: &wgpu::Device) -> Self {
        let query = device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
            .then(|| {
                device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("VBD hardware time"),
                    ty: wgpu::QueryType::Timestamp,
                    count: 2,
                })
            });
        let resolve = query.as_ref().map(|_| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("VBD timestamps"),
                size: 16,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        });
        Self {
            buffers: Vec::new(),
            capacities: Vec::new(),
            bind: None,
            read: Self::read_buffer(device, 16),
            read_capacity: 16,
            query,
            resolve,
        }
    }
    fn read_buffer(device: &wgpu::Device, bytes: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VBD reusable readback"),
            size: bytes as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        data: [&[u8]; 8],
        moving: usize,
    ) {
        let mut changed = false;
        for (i, bytes) in data
            .iter()
            .copied()
            .chain(std::iter::once(&[][..]))
            .enumerate()
        {
            let size = if i == 8 { moving * 16 } else { bytes.len() }.max(16);
            if self.capacities.get(i).is_none_or(|c| *c < size) {
                let capacity = size.next_power_of_two();
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("VBD persistent storage"),
                    size: capacity as u64,
                    usage: (if i == 7 {
                        wgpu::BufferUsages::UNIFORM
                    } else {
                        wgpu::BufferUsages::STORAGE
                    }) | wgpu::BufferUsages::COPY_DST
                        | (if i == 0 || i == 8 {
                            wgpu::BufferUsages::COPY_SRC
                        } else {
                            wgpu::BufferUsages::empty()
                        }),
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
                queue.write_buffer(&self.buffers[i], 0, bytes);
            }
        }
        if changed {
            let entries: Vec<_> = self
                .buffers
                .iter()
                .enumerate()
                .map(|(i, buffer)| wgpu::BindGroupEntry {
                    binding: i as u32,
                    resource: if i == 7 {
                        wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer,
                            offset: 0,
                            size: std::num::NonZeroU64::new(16),
                        })
                    } else {
                        buffer.as_entire_binding()
                    },
                })
                .collect();
            self.bind = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("VBD persistent energies"),
                layout,
                entries: &entries,
            }));
        }
        let size = moving * 16 + 16;
        if size > self.read_capacity {
            self.read_capacity = size.next_power_of_two();
            self.read = Self::read_buffer(device, self.read_capacity);
        }
    }
}

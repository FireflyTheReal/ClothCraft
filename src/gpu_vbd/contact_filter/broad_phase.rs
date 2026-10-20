use super::*;

#[derive(Clone, Copy)]
#[repr(C)]
struct Node {
    lo: [f32; 4],
    hi: [f32; 4],
    info: [u32; 4],
}
unsafe impl bytemuck::Zeroable for Node {}
unsafe impl bytemuck::Pod for Node {}

impl GpuContactFilter {
    pub(super) fn spatial_filter(
        &mut self,
        q: SpatialQuery<'_>,
    ) -> Result<Option<PrimitiveCandidates>, String> {
        let started = std::time::Instant::now();
        let changed = self.spatial_layout_id != q.id;
        let n = q.x.len();
        let nt = q.topology.triangles.len();
        let ne = q.topology.edges.len();
        if n == 0 {
            return Ok(Some(PrimitiveCandidates::default()));
        }
        let limits = self.device.limits();
        let size_limit =
            (limits.max_storage_buffer_binding_size as usize).min(limits.max_buffer_size as usize);
        let query_count = n + ne;
        let groups = query_count.div_ceil(64);
        let width = groups.min(limits.max_compute_workgroups_per_dimension as usize);
        let node_count = q.triangles.nodes().len() + q.edges.nodes().len();
        if n + nt + ne >= 0x8000_0000
            || node_count > u32::MAX as usize
            || groups.div_ceil(width) > limits.max_compute_workgroups_per_dimension as usize
            || [(n + nt + ne) * 16, node_count * 48]
                .iter()
                .any(|s| s.next_power_of_two() > size_limit)
        {
            return Ok(None);
        }
        let origin = if changed { q.x[0] } else { self.spatial_origin };
        let scale =
            q.x.iter()
                .map(|p| (*p - origin).abs().max_element())
                .fold(0., f64::max);
        let world_scale = q.x.iter().map(|p| p.abs().max_element()).fold(0., f64::max);
        let radius = q.padded_radius2.sqrt();
        if !scale.is_finite() || scale > 1e6 || !(1e-12..1e6).contains(&radius) {
            return Ok(None);
        }
        let padding =
            64. * f32::EPSILON as f64 * scale.max(radius) + 64. * f64::EPSILON * world_scale;
        let point = |p: glam::DVec3| {
            let p = (p - origin).as_vec3();
            [p.x, p.y, p.z, 0.]
        };
        let positions: Vec<_> = q.x[..if changed { n } else { q.moving }]
            .iter()
            .map(|p| point(*p))
            .collect();
        let mut primitives = Vec::new();
        if changed {
            primitives.reserve(n + nt + ne);
            primitives.extend((0..n).map(|i| [i as u32, i as u32, i as u32, 1]));
            primitives.extend(
                q.topology
                    .triangles
                    .iter()
                    .map(|t| [t[0] as u32, t[1] as u32, t[2] as u32, 3]),
            );
            primitives.extend(
                q.topology
                    .edges
                    .iter()
                    .map(|e| [e[0] as u32, e[1] as u32, e[1] as u32, 2]),
            );
        }
        let mut order = Vec::<u32>::new();
        let mut exclusions = Vec::<u32>::new();
        if changed {
            let entries = n + 1 + q.excluded_neighbors.iter().map(Vec::len).sum::<usize>();
            if entries > u32::MAX as usize || (entries * 4).next_power_of_two() > size_limit {
                return Ok(None);
            }
            exclusions.reserve(entries);
            let mut offset = n + 1;
            for neighbors in q.excluded_neighbors {
                exclusions.push(offset as u32);
                offset += neighbors.len();
            }
            exclusions.push(offset as u32);
            exclusions.extend(q.excluded_neighbors.iter().flatten().map(|v| *v as u32));
        }
        let mut nodes = Vec::<Node>::new();
        if changed {
            order.reserve(nt + ne);
            nodes.reserve(node_count);
        }
        let mut roots = [u32::MAX; 4];

        for (tree_index, (tree, primitive_base)) in [(q.triangles, n), (q.edges, n + nt)]
            .into_iter()
            .enumerate()
        {
            let node_base = if tree_index == 0 {
                0
            } else {
                q.triangles.nodes().len()
            };
            let order_base = if tree_index == 0 { 0 } else { nt };
            if changed {
                order.extend(
                    tree.primitive_order()
                        .iter()
                        .map(|i| (primitive_base + i) as u32),
                );
            }
            for (i, root) in tree.roots().iter().enumerate() {
                roots[tree_index * 2 + i] = (node_base + root) as u32;
            }
            if changed {
                nodes.extend(tree.nodes().map(|node| Node {
                    lo: point(node.bounds.min),
                    hi: point(node.bounds.max),
                    info: [
                        (order_base + node.start) as u32,
                        (node.end - node.start) as u32,
                        (node_base + node.escape) as u32,
                        u32::from(node.children.is_none()) | (u32::from(node.dynamic) << 1),
                    ],
                }));
            }
        }
        let mut retries = 0;
        let mut timings = [started.elapsed().as_secs_f64() * 1000., 0., 0.];
        loop {
            let output_size = 16 + self.broad_capacity * 8;
            if output_size.next_power_of_two() > size_limit {
                return Ok(None);
            }
            let config = [
                query_count as u32,
                width as u32,
                ((radius + padding) as f32).to_bits(),
                (padding as f32).to_bits(),
                n as u32,
                nt as u32,
                ne as u32,
                q.moving as u32,
                roots[0],
                roots[1],
                roots[2],
                roots[3],
                self.broad_capacity as u32,
                q.triangles.dynamic_node_count() as u32,
                q.triangles.nodes().len() as u32,
                q.edges.dynamic_node_count() as u32,
            ];
            let upload_start = std::time::Instant::now();
            self.prepare(
                [
                    bytemuck::cast_slice(&positions),
                    bytemuck::cast_slice(&primitives),
                    bytemuck::cast_slice(&order),
                    if changed {
                        bytemuck::cast_slice(&nodes)
                    } else {
                        &[]
                    },
                    bytemuck::cast_slice(&config),
                    bytemuck::cast_slice(&exclusions),
                ],
                output_size,
            );
            self.spatial_layout_id = q.id;
            self.spatial_origin = origin;
            timings[0] += upload_start.elapsed().as_secs_f64() * 1000.;
            let dispatch_start = std::time::Instant::now();
            let mut encoder = self.device.create_command_encoder(&Default::default());
            encoder.clear_buffer(&self.buffers[5], 0, Some(16));
            {
                let mut pass = encoder.begin_compute_pass(&Default::default());
                pass.set_bind_group(0, self.bind.as_ref().unwrap(), &[]);
                let dynamic_nodes = q.triangles.dynamic_node_count() + q.edges.dynamic_node_count();
                if dynamic_nodes > 0 {
                    pass.set_pipeline(&self.refit_pipeline);
                    pass.dispatch_workgroups(
                        dynamic_nodes.min(width) as u32,
                        dynamic_nodes.div_ceil(width) as u32,
                        1,
                    );
                }
                pass.set_pipeline(&self.pipeline);
                pass.dispatch_workgroups(width as u32, groups.div_ceil(width) as u32, 1);
            }
            let read = self.read.as_ref().unwrap();
            encoder.copy_buffer_to_buffer(&self.buffers[5], 0, read, 0, output_size as u64);
            let submission = self.queue.submit([encoder.finish()]);
            let (tx, rx) = std::sync::mpsc::channel();
            read.map_async(wgpu::MapMode::Read, 0..output_size as u64, move |r| {
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
            timings[1] += dispatch_start.elapsed().as_secs_f64() * 1000.;
            let decode_start = std::time::Instant::now();
            let data = read
                .get_mapped_range(0..output_size as u64)
                .map_err(|e| e.to_string())?;
            let header: &[u32] = bytemuck::cast_slice(&data[..16]);
            let count = header[0] as usize;
            let capacity_overflow = header[1] & 2 != 0;
            if capacity_overflow || count > self.broad_capacity {
                drop(data);
                read.unmap();
                self.broad_capacity = count.next_power_of_two().max(self.broad_capacity * 2);
                retries += 1;
                continue;
            }
            let mut result = PrimitiveCandidates {
                retries,
                vf: Vec::with_capacity(count),
                ee: Vec::with_capacity(count),
                ..Default::default()
            };
            let records: &[[u32; 2]] = bytemuck::cast_slice(&data[16..16 + count * 8]);
            for &[a, b] in records {
                if b & 0x8000_0000 == 0 {
                    result.vf.push([a as usize, b as usize]);
                } else {
                    result.ee.push([a as usize, (b & 0x7fff_ffff) as usize]);
                }
            }
            drop(data);
            read.unmap();
            // Reservation order is immaterial: f64 distances reduce by min,
            // and OgcWorld canonicalizes the resulting contact keys.
            timings[2] += decode_start.elapsed().as_secs_f64() * 1000.;
            result.timings = timings;
            return Ok(Some(result));
        }
    }
}

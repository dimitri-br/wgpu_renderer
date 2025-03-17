use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;
use shipyard::Unique;
use wgpu::{Buffer, Device, Queue};
use crate::renderer::types::instance_data::InstanceData;

#[derive(Unique)]
pub struct InstancingComponent {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub instance_buffer: Buffer,
    pub instance_count: u32,
    // Mapping: (mesh_id, material_id) -> (offset, count)
    pub group_offsets: HashMap<(u64, u64), (u32, u32)>,
    // The bind group for instancing data; this will be bound to group 2.
    pub instancing_bind_group: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl InstancingComponent {
    /// Create a new instancing component with an initial maximum capacity.
    pub fn new(device: Arc<Device>, queue: Arc<Queue>, max_instances: u32) -> Self {
        let size = (max_instances as usize * std::mem::size_of::<InstanceData>()) as u64;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instancing SSBO"),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create the bind group layout for group 2.
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Instancing Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::all(),
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Create the instancing bind group using the entire buffer.
        let instancing_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Instancing Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: instance_buffer.as_entire_binding(),
            }],
        });

        Self {
            device,
            queue,
            instance_buffer,
            instance_count: 0,
            group_offsets: HashMap::new(),
            instancing_bind_group,
            bind_group_layout,
        }
    }

    /// Updates the instance buffer with a new contiguous array of InstanceData
    /// and a mapping from (mesh, material) pair to (offset, count).
    pub fn update(&mut self, instance_data: &[InstanceData], group_offsets: HashMap<(u64, u64), (u32, u32)>) {
        let required_size = (instance_data.len() * std::mem::size_of::<InstanceData>()) as u64;
        // Reallocate the SSBO if the new data doesn't fit.
        if required_size > self.instance_buffer.size() {
            self.instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Instancing SSBO (Resized)"),
                size: required_size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            // Recreate the bind group with the new buffer.
            self.instancing_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Instancing Bind Group"),
                layout: &self.bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.instance_buffer.as_entire_binding(),
                }],
            });
        }
        // Write the new instance data into the SSBO.
        self.queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(instance_data));
        self.instance_count = instance_data.len() as u32;
        self.group_offsets = group_offsets;
    }
}
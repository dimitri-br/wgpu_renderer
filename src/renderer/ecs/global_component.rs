use std::num::NonZeroU32;
use std::sync::Arc;
use shipyard::Unique;
use crate::renderer::bind_group_cache::{BindGroupCache, BindGroupKey};
use crate::renderer::types::light_storage::LightStorage;
use crate::renderer::State;
use crate::renderer::types::global::Globals;
use crate::renderer::types::light::Light;
use crate::renderer::types::uniform::UniformBuffer;

#[derive(Unique)]
pub struct GlobalComponent{
    pub directional_light: Option<Light>,
    pub directional_light_buffer: Option<Arc<UniformBuffer>>,
    pub point_light_storage: LightStorage,

    pub bind_group_cache: Arc<BindGroupCache>,

    pub global_data: Globals,
    pub global_uniform_buffer: Arc<UniformBuffer>,

    pub global_bind_group_key: BindGroupKey,
    pub global_bind_group_layout: Arc<wgpu::BindGroupLayout>,
    pub global_bind_group: Arc<wgpu::BindGroup>,
}

impl GlobalComponent {
    pub fn new(state: &State) -> Self{
        let global_data = Globals::new();

        let directional_light = Light::new(
            glam::Vec3::new(0.0, 1.0, 0.0),
            glam::Vec3::new(0.5, -1.0, 0.0),
            glam::Vec3::new(1.0, 1.0, 1.0),
            1.0,
            0.0,
        );

        // Create global uniform + bind group
        let global_uniform_buffer = Arc::new(UniformBuffer::new(&state.device, &state.queue, size_of::<Globals>() as u64));
        global_uniform_buffer.update(&global_data);

        let directional_light_buffer = Arc::new(UniformBuffer::new(&state.device, &state.queue, size_of::<Light>() as u64));
        directional_light_buffer.update(&directional_light);

        let global_bind_group_layout = Arc::new(state.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Global BindGroupLayout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        min_binding_size: None,
                        has_dynamic_offset: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        min_binding_size: None,
                        has_dynamic_offset: false,
                    },
                    count: None
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        min_binding_size: None,
                        has_dynamic_offset: false,
                    },
                    count: None
                }
            ]
        }));



        let point_light_storage = LightStorage::new(state.device.clone(), state.queue.clone());
        let global_bind_group_key = BindGroupKey::new(&global_bind_group_layout, vec![Arc::as_ptr(&global_uniform_buffer) as usize]);
        let global_bind_group = state.bind_group_cache.get_or_create(&global_bind_group_layout, &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(global_uniform_buffer.get_buffer_binding()),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Buffer(directional_light_buffer.get_buffer_binding()),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Buffer(point_light_storage.get_buffer_binding()),
            }
        ], global_bind_group_key.clone(), true);

        Self {
            directional_light: Some(directional_light),
            directional_light_buffer: Some(directional_light_buffer),
            point_light_storage,

            bind_group_cache: state.bind_group_cache.clone(),
            global_data,
            global_uniform_buffer,
            global_bind_group_key,
            global_bind_group_layout,
            global_bind_group,
        }
    }

    pub fn reconstruct_bind_group(&mut self){
        self.global_bind_group = self.bind_group_cache.get_or_create(&self.global_bind_group_layout, &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(self.global_uniform_buffer.get_buffer_binding()),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Buffer(self.directional_light_buffer.as_ref().unwrap().get_buffer_binding()),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Buffer(self.point_light_storage.get_buffer_binding()),
            }
        ], self.global_bind_group_key.clone(), false);
    }
}
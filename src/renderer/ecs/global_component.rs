use std::num::NonZeroU32;
use std::sync::{Arc, RwLock};
use shipyard::Unique;
use crate::renderer::bind_group_cache::{BindGroupCache, BindGroupKey};
use crate::renderer::shadow_atlas::{AtlasTile, ShadowAtlas};
use crate::renderer::types::light_storage::LightStorage;
use crate::renderer::State;
use crate::renderer::types::global::Globals;
use crate::renderer::types::light::Light;
use crate::renderer::types::uniform::{Uniform, UniformBuffer};


#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
pub struct ShadowData {
    pub(crate) light_view_proj: glam::Mat4,
    uv_offset: glam::Vec2,
    uv_scale: glam::Vec2,
    bias: f32,                    // Depth bias
    _padding: [f32; 3],
}

impl Uniform for ShadowData {
    fn update_uniforms(&self, buffer: &wgpu::Buffer, queue: &wgpu::Queue) {
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(&[*self]));
    }
}

#[derive(Unique)]
pub struct GlobalComponent{
    pub directional_light: Option<Light>,
    pub directional_light_shadow_map: Option<Arc<RwLock<AtlasTile>>>,
    pub directional_light_buffer: Option<Arc<UniformBuffer>>,

    pub directional_shadow_data: Option<ShadowData>,
    pub directional_shadow_buffer: Option<Arc<UniformBuffer>>,

    pub point_light_storage: LightStorage,

    pub bind_group_cache: Arc<BindGroupCache>,

    pub global_data: Globals,
    pub global_uniform_buffer: Arc<UniformBuffer>,

    pub global_bind_group_key: BindGroupKey,
    pub global_bind_group_layout: Arc<wgpu::BindGroupLayout>,
    pub global_bind_group: Arc<wgpu::BindGroup>,
}

impl GlobalComponent {
    pub fn new(state: &State, shadow_atlas: &mut ShadowAtlas) -> Self{
        let global_data = Globals::new();

        let directional_light = Light::new(
            // Position
            glam::Vec3::new(0.0, 1.0, 0.0),
            // Direction
            glam::Vec3::new(-0.5, -1.0, 0.0),
            glam::Vec3::new(1.0, 1.0, 1.0),
            0.5,
            0.0,
        );
        let directional_shadow_map = shadow_atlas.allocate_tile(2048, 2048).expect("Failed to allocate shadow map");

        // Create global uniform + bind group
        let global_uniform_buffer = Arc::new(UniformBuffer::new(&state.device, &state.queue, size_of::<Globals>() as u64));
        global_uniform_buffer.update(&global_data);

        let directional_light_buffer = Arc::new(UniformBuffer::new(&state.device, &state.queue, size_of::<Light>() as u64));
        directional_light_buffer.update(&directional_light);

        let directional_shadow_data = ShadowData {
            light_view_proj: glam::Mat4::IDENTITY,
            uv_offset: directional_shadow_map.read().unwrap().uv_offset,
            uv_scale: directional_shadow_map.read().unwrap().uv_scale,
            bias: 0.001,
            _padding: [0.0; 3],
        };

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
            directional_light_shadow_map: Some(directional_shadow_map),
            directional_light_buffer: Some(directional_light_buffer),

            directional_shadow_data: Some(directional_shadow_data),
            directional_shadow_buffer: Some(Arc::new(UniformBuffer::new(&state.device, &state.queue, size_of::<ShadowData>() as u64))),

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
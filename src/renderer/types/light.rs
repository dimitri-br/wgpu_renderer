use crate::renderer::types::shadow_data::ShadowData;
use crate::renderer::types::light_type::LightType;
use crate::renderer::types::uniform::Uniform;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct Light{
    pub position: glam::Vec3,
    pub range: f32,
    pub rotation: glam::Vec3, // Only applicable for directional lights and spotlights
    pub intensity: f32,
    pub color: glam::Vec3,
    pub light_type: u32,
    pub view_proj: glam::Mat4, // Only applicable for directional lights and spotlights
    pub shadow_data_offset: u32, // Only applicable for shadow casting lights
    pub shadow_data_count: u32, // Only applicable for shadow casting lights
    _padding: [f32; 2],
}

impl Light{
    pub fn new(position: glam::Vec3, rotation: glam::Vec3, color: glam::Vec3, intensity: f32, range: f32, light_type: LightType) -> Self {
        Self {
            position,
            rotation,
            color,
            intensity,
            range,
            light_type: light_type as u32,
            view_proj: glam::Mat4::IDENTITY,
            shadow_data_offset: 0,
            shadow_data_count: 0,
            _padding: [0.0; 2],
        }
    }

    pub fn set_shadow_data(&mut self, shadow_data_offset: u32, shadow_data_count: u32){
        self.shadow_data_offset = shadow_data_offset;
        self.shadow_data_count = shadow_data_count;
    }

    pub fn set_view_proj(&mut self, view_proj: glam::Mat4){
        self.view_proj = view_proj;
    }
}

impl Uniform for Light{
    fn update_uniforms(&self, buffer: &wgpu::Buffer, queue: &wgpu::Queue){
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(&[*self]));
    }
}

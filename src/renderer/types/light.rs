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
    pub light_type: LightType,
    pub view_proj: glam::Mat4, // Only applicable for directional lights and spotlights
}

impl Light{
    pub fn new(position: glam::Vec3, rotation: glam::Vec3, color: glam::Vec3, intensity: f32, range: f32, light_type: LightType) -> Self {
        Self {
            position,
            rotation,
            color,
            intensity,
            range,
            light_type,
            view_proj: glam::Mat4::IDENTITY,
        }
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

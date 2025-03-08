use crate::renderer::types::uniform::Uniform;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct Light{
    pub position: glam::Vec3,
    pub range: f32,
    pub rotation: glam::Vec3, // Only applicable for directional lights and spotlights
    pub intensity: f32,
    pub color: glam::Vec3,
    _padding: f32,
}

impl Light{
    pub fn new(position: glam::Vec3, rotation: glam::Vec3, color: glam::Vec3, intensity: f32, range: f32) -> Self{
        Self{
            position,
            rotation,
            color,
            intensity,
            range,
            _padding: 0.0,
        }
    }
}

impl Uniform for Light{
    fn update_uniforms(&self, buffer: &wgpu::Buffer, queue: &wgpu::Queue){
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(&[*self]));
    }
}

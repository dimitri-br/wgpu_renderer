use crate::renderer::types::uniform::Uniform;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
pub struct ShadowData {
    pub(crate) light_view_proj: glam::Mat4,
    pub uv_offset: glam::Vec2,
    pub uv_scale: glam::Vec2,
    pub bias: f32,                    // Depth bias
    _padding: [f32; 3],
}

impl ShadowData {
    pub fn new(light_view_proj: glam::Mat4, uv_offset: glam::Vec2, uv_scale: glam::Vec2, bias: f32) -> Self {
        Self {
            light_view_proj,
            uv_offset,
            uv_scale,
            bias,
            _padding: [0.0; 3],
        }
    }
}

impl Uniform for ShadowData {
    fn update_uniforms(&self, buffer: &wgpu::Buffer, queue: &wgpu::Queue) {
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(&[*self]));
    }
}
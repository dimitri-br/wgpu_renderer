use crate::renderer::types::camera::Camera;
use crate::renderer::types::uniform::Uniform;
use wgpu::{Buffer, Queue};

/// Global data that is shared between all shaders (group=0).
///
/// In this example, we store just the camera's view-projection matrix (4x4).
/// You can add more fields (time, lighting, etc.) as needed.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Globals {
    /// The camera's combined view-projection matrix.
    /// Stored as a row-major 4x4 for WGSL usage.
    pub view_proj: glam::Mat4,
}

impl Globals {
    /// Initialize with an identity matrix so nothing breaks if we forget to set it.
    pub fn new() -> Self {
        Self {
            view_proj: glam::Mat4::IDENTITY,
        }
    }

    /// Update global data here—particularly the camera transform in `view_proj`.
    /// We'll call this each frame (or whenever the camera changes).
    pub fn update_from_camera(&mut self, camera: &dyn Camera) {
        let vp = camera.build_view_projection_matrix();
        self.view_proj = vp;
    }
}

impl Uniform for Globals {
    fn update_uniforms(&self, buffer: &Buffer, queue: &Queue) {
        // Write our 4x4 matrix into the buffer at offset 0.
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(&[*self]));
    }
}

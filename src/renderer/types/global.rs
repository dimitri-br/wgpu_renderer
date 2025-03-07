use bytemuck::{Pod, Zeroable};
use crate::renderer::types::camera::Camera;
use crate::renderer::types::uniform::Uniform;
use wgpu::{Buffer, Queue};
use crate::renderer::ecs::components::LightComponent;

/// Global data that is shared between all shaders (group=0).
///
/// In this example, we store just the camera's view-projection matrix (4x4).
/// You can add more fields (time, lighting, etc.) as needed.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Globals {
    /// The camera's combined view-projection matrix.
    /// Stored as a row-major 4x4 for WGSL usage.
    pub view_proj: glam::Mat4,
    /// Inverse view-projection matrix
    pub inv_view_proj: glam::Mat4,
    /// The screen size
    pub screen_size: glam::Vec2,
    /// Time
    pub time: f32,
    /// Padding
    _padding: f32,
}

impl Globals {
    /// Initialize with an identity matrix so nothing breaks if we forget to set it.
    pub fn new() -> Self {
        Self {
            view_proj: glam::Mat4::IDENTITY,
            inv_view_proj: glam::Mat4::IDENTITY,
            screen_size: glam::Vec2::new(0.0, 0.0),
            time: 0.0,
            _padding: 0.0,
        }
    }

    /// Update global data here—particularly the camera transform in `view_proj`.
    /// We'll call this each frame (or whenever the camera changes).
        pub fn update_from_camera<T: Camera + ?Sized>(&mut self, camera: &T) {
            let vp = camera.build_view_projection_matrix();
            self.view_proj = vp;
            self.inv_view_proj = vp.inverse();
        }

    /// Update the screen size
    pub fn update_screen_size(&mut self, width: f32, height: f32) {
        self.screen_size = glam::Vec2::new(width, height);
    }

    pub fn update(&mut self, time: f32) {
        self.time = time;
    }
}

impl Uniform for Globals {
    fn update_uniforms(&self, buffer: &Buffer, queue: &Queue) {
        // Write our 4x4 matrix into the buffer at offset 0.
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(&[*self]));
    }
}


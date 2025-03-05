use crate::renderer::types::uniform::Uniform;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Zeroable, bytemuck::Pod)]
pub struct Transform {
    pub matrix: glam::Mat4,
}

impl Transform {
    pub fn new() -> Self {
        Self {
            matrix: glam::Mat4::default(),
        }
    }

    pub fn set_translation(&mut self, translation: glam::Vec3) {
        self.matrix = glam::Mat4::from_translation(translation);
    }

    pub fn set_rotation(&mut self, rotation: glam::Quat) {
        self.matrix = glam::Mat4::from_quat(rotation);
    }

    pub fn set_scale(&mut self, scale: glam::Vec3) {
        self.matrix = glam::Mat4::from_scale(scale);
    }

    pub fn set_transform(
        &mut self,
        translation: glam::Vec3,
        rotation: glam::Quat,
        scale: glam::Vec3,
    ) {
        self.matrix = glam::Mat4::from_scale_rotation_translation(scale, rotation, translation);
    }

    pub fn translate(&mut self, translation: glam::Vec3) {
        self.matrix = self.matrix * glam::Mat4::from_translation(translation);
    }

    pub fn rotate(&mut self, rotation: glam::Quat) {
        self.matrix = self.matrix * glam::Mat4::from_quat(rotation);
    }

    pub fn scale(&mut self, scale: glam::Vec3) {
        self.matrix = self.matrix * glam::Mat4::from_scale(scale);
    }

    pub fn transform(&mut self, translation: glam::Vec3, rotation: glam::Quat, scale: glam::Vec3) {
        self.matrix =
            self.matrix * glam::Mat4::from_scale_rotation_translation(scale, rotation, translation);
    }

    pub fn translation(&self) -> glam::Vec3 {
        self.matrix.to_scale_rotation_translation().2
    }

    pub fn rotation(&self) -> glam::Quat {
        self.matrix.to_scale_rotation_translation().1
    }

    pub fn to_scale(&self) -> glam::Vec3 {
        self.matrix.to_scale_rotation_translation().0
    }

    pub fn update_uniforms(&self, uniforms: &wgpu::Buffer, queue: &wgpu::Queue) {
        queue.write_buffer(uniforms, 0, bytemuck::cast_slice(&[*self]));
    }
}

impl Uniform for Transform {
    fn update_uniforms(&self, uniforms: &wgpu::Buffer, queue: &wgpu::Queue) {
        self.update_uniforms(uniforms, queue);
    }
}

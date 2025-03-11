#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
pub struct Vertex {
    pub position: glam::Vec3,
    pub normal: glam::Vec3,
    pub tex_coords: glam::Vec2,
}

impl Vertex{
    pub fn new(position: glam::Vec3, normal: glam::Vec3, tex_coords: glam::Vec2) -> Self {
        Self {
            position,
            normal,
            tex_coords,
        }
    }
}
// ----------------------------
// 1. Instance Data Structure
// ----------------------------
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceData {
    pub model: glam::Mat4,
    pub normal_matrix: glam::Mat4,
}
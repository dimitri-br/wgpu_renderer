use std::sync::Arc;
use crate::renderer::types::material::Material;
use crate::renderer::types::gpu_mesh::GpuMesh;
use crate::renderer::types::transform::Transform;
use shipyard::*;

#[derive(Component)]
pub struct MeshComponent {
    pub mesh: Arc<GpuMesh>,
}

#[derive(Component)]
pub struct MaterialComponent {
    pub material: Arc<Material>,
}

#[derive(Component)]
pub struct TransformComponent {
    pub transform: Transform,
}

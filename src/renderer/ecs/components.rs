use std::sync::Arc;
use crate::renderer::types::material::Material;
use crate::renderer::types::gpu_mesh::GpuMesh;
use crate::renderer::types::transform::Transform;
use shipyard::*;
use crate::renderer::types::light::Light;

#[derive(Component)]
pub struct MeshComponent {
    pub mesh: Arc<GpuMesh>,
}

impl MeshComponent {
    pub fn new(mesh: Arc<GpuMesh>) -> Self {
        Self { mesh }
    }
}

impl From<Arc<GpuMesh>> for MeshComponent {
    fn from(mesh: Arc<GpuMesh>) -> Self {
        Self::new(mesh)
    }
}

#[derive(Component)]
pub struct MaterialComponent {
    pub material: Arc<Material>,
}

impl MaterialComponent {
    pub fn new(material: Arc<Material>) -> Self {
        Self { material }
    }
}

impl From<Arc<Material>> for MaterialComponent {
    fn from(material: Arc<Material>) -> Self {
        Self::new(material)
    }
}

#[derive(Component)]
pub struct TransformComponent {
    pub transform: Transform,
}

impl TransformComponent {
    pub fn new(transform: Transform) -> Self {
        Self { transform }
    }
}

impl From<Transform> for TransformComponent {
    fn from(transform: Transform) -> Self {
        Self::new(transform)
    }
}


#[derive(Component)]
pub struct LightComponent{
    pub(crate) light: Light
}

impl LightComponent{
    pub fn new(light: Light) -> Self{
        Self{
            light
        }
    }
}

impl From<Light> for LightComponent{
    fn from(light: Light) -> Self{
        Self::new(light)
    }
}
use std::sync::Arc;
use crate::renderer::types::material::Material;
use crate::renderer::types::gpu_mesh::GpuMesh;
use crate::renderer::types::transform::Transform;
use shipyard::*;
use crate::renderer::types::light::Light;
use crate::renderer::types::light_type::LightType;
use crate::renderer::types::texture::Texture;

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
    pub(crate) light: Light,
    pub(crate) light_type: LightType,
    pub(crate) shadow_map: Option<Texture>,
}

impl LightComponent{
    pub fn new(light: Light, light_type: LightType) -> Self{
        Self{
            light,
            light_type,
            shadow_map: None,
        }
    }
}

impl From<Light> for LightComponent{
    fn from(light: Light) -> Self{
        Self::new(light, LightType::Directional)
    }
}
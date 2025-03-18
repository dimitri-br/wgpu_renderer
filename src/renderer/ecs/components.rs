use std::ops::Deref;
use std::sync::{Arc, RwLock};
use crate::renderer::types::material::Material;
use crate::renderer::types::gpu_mesh::GpuMesh;
use crate::renderer::types::transform::Transform;
use shipyard::*;
use crate::renderer::gpu_storage::GpuStorable;
use crate::renderer::shadow_atlas::AtlasTile;
use crate::renderer::types::light::Light;
use crate::renderer::types::light_type::LightType;
use crate::renderer::types::shadow_data::ShadowData;
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

impl Deref for MeshComponent {
    type Target = GpuMesh;

    fn deref(&self) -> &Self::Target {
        &*self.mesh
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

impl Deref for MaterialComponent {
    type Target = Material;

    fn deref(&self) -> &Self::Target {
        &*self.material
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

impl Deref for TransformComponent {
    type Target = Transform;

    fn deref(&self) -> &Self::Target {
        &self.transform
    }
}


#[derive(Component)]
pub struct LightComponent{
    pub(crate) light: Light,
    pub(crate) shadow_map: Option<Arc<Texture>>,
}

impl LightComponent{
    pub fn new(light: Light, light_type: LightType) -> Self{
        Self{
            light,
            shadow_map: None,
        }
    }

    pub fn with_shadow_map(light: Light, shadow_map: Arc<Texture>) -> Self{
        Self{
            light,
            shadow_map: Some(shadow_map),
        }
    }

    pub fn set_shadow_map(&mut self, shadow_map: Arc<Texture>){
        self.shadow_map = Some(shadow_map);
    }
}

impl From<Light> for LightComponent{
    fn from(light: Light) -> Self{
        Self::new(light, LightType::Directional)
    }
}

impl Deref for LightComponent{
    type Target = Light;

    fn deref(&self) -> &Self::Target{
        &self.light
    }
}

#[derive(Component)]
#[derive(Clone)]
pub struct ShadowMapComponent {
    pub shadow_data: ShadowData,
    pub tile: Arc<RwLock<AtlasTile>>,
    // plus additional data: light's projection matrix, etc.
}

impl ShadowMapComponent {
    pub fn new(shadow_data: ShadowData, tile: Arc<RwLock<AtlasTile>>) -> Self {
        Self { shadow_data, tile }
    }
}

impl GpuStorable for ShadowMapComponent {
    type Storage = ShadowData;

    fn as_storage(&self) -> Self::Storage {
        self.shadow_data
    }
}
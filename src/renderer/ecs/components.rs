use std::ops::Deref;
use std::sync::Arc;
use crate::renderer::types::material::Material;
use crate::renderer::types::gpu_mesh::GpuMesh;
use crate::renderer::types::transform::Transform;
use shipyard::*;
use crate::renderer::shadow_atlas::AtlasTile;
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

impl Deref for LightComponent{
    type Target = Light;

    fn deref(&self) -> &Self::Target{
        &self.light
    }
}

#[derive(Component)]
pub struct ShadowMapComponent {
    pub tile: AtlasTile,
    // plus additional data: light's projection matrix, etc.
}

impl ShadowMapComponent {
    pub fn new(tile: AtlasTile) -> Self {
        Self { tile }
    }
}

impl From<AtlasTile> for ShadowMapComponent {
    fn from(tile: AtlasTile) -> Self {
        Self::new(tile)
    }
}

impl Deref for ShadowMapComponent {
    type Target = AtlasTile;

    fn deref(&self) -> &Self::Target {
        &self.tile
    }
}

#[derive(Component)]
pub struct ShadowCastComponent {
    pub(crate) shadow_cast: bool,
}

impl ShadowCastComponent {
    pub fn new(shadow_cast: bool) -> Self {
        Self { shadow_cast }
    }
}

impl From<bool> for ShadowCastComponent {
    fn from(shadow_cast: bool) -> Self {
        Self::new(shadow_cast)
    }
}

impl Deref for ShadowCastComponent {
    type Target = bool;

    fn deref(&self) -> &Self::Target {
        &self.shadow_cast
    }
}
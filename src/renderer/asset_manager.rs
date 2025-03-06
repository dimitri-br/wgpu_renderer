// src/renderer/asset_manager.rs

use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::Path;
use std::sync::Arc;
use shipyard::Unique;
use wgpu::Device;
use uuid::Uuid;
use crate::renderer::types::gpu_mesh::GpuMesh;
use crate::renderer::types::mesh::Mesh;
use crate::renderer::types::material::Material;
use crate::renderer::types::texture::Texture;
use crate::renderer::State;

use super::shader_reflect::Shader;

/// Generates a UUID based on a given string using version 5 with the URL namespace.
/// This ensures that the same input always produces the same UUID.
fn uuid_from_string<S: AsRef<str>>(s: S) -> Uuid {
    let namespace = Uuid::NAMESPACE_URL;
    Uuid::new_v5(&namespace, s.as_ref().as_bytes())
}

/// The AssetManager caches loaded assets (meshes, materials, textures)
/// so that duplicates are not loaded multiple times. This is designed to
/// be used as a Unique resource in your ECS.
#[derive(Unique)]
pub struct AssetManager {
    pub meshes: HashMap<Uuid, Arc<GpuMesh>>,
    pub shaders: HashMap<Uuid, Arc<Shader>>,
    pub materials: HashMap<Uuid, Arc<Material>>,
    pub textures: HashMap<Uuid, Arc<Texture>>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            meshes: HashMap::new(),
            shaders: HashMap::new(),
            materials: HashMap::new(),
            textures: HashMap::new(),
        }
    }

    /// Get or create a mesh asset from an OBJ file.
    /// The UUID is generated from the file path.
    pub fn get_or_create_mesh<P: AsRef<Path>>(
        &mut self,
        path: P,
        state: &mut State,
    ) -> Arc<GpuMesh> {
        let path_str = path.as_ref().to_string_lossy();
        let key = uuid_from_string(&path_str);
        if let Some(mesh) = self.meshes.get(&key) {
            return mesh.clone();
        }
        let cpu_mesh = Mesh::load_obj(path).expect("Failed to load mesh");
        let gpu_mesh = GpuMesh::from_cpu_mesh(&state.device, &cpu_mesh);
        let gpu_mesh = Arc::new(gpu_mesh);
        self.meshes.insert(key, gpu_mesh.clone());
        gpu_mesh
    }

    /// Get or create a texture asset from a file.
    /// The UUID is generated from the provided name.
    pub fn get_or_create_texture<P: AsRef<Path>>(
        &mut self,
        name: &str,
        path: P,
        state: &State,
    ) -> Arc<Texture> {
        let key = uuid_from_string(name);
        if let Some(texture) = self.textures.get(&key) {
            return texture.clone();
        }
        let texture = state.load_texture(name, path.as_ref());
        self.textures.insert(key, texture.clone());
        texture
    }

    /// Get or create a material asset.
    /// The UUID is generated from the unique material identifier (e.g. name).
    pub fn get_or_create_material<S: AsRef<str>>(
        &mut self,
        id: S,
        shader: Arc<Shader>,
        state: &State,
    ) -> Arc<Material> {
        let key = uuid_from_string(id.as_ref());
        if let Some(mat) = self.materials.get(&key) {
            return mat.clone();
        }
        let material = stat
        let material = Arc::new(material);
        self.materials.insert(key, material.clone());
        material
    }

    pub fn get_or_create_shader<S: AsRef<str>, P: AsRef<Path>>(
        &mut self,
        id: S,
        path: P,
        state: &State,
    ) -> Arc<Shader>{
        let key = uuid_from_string(id.as_ref());
        if let Some(shader) = self.shaders.get(&key) {
            return shader.clone();
        }
        let file = read_to_string(path).expect("Failed to load shader from path!");
        state.register_shader(id.as_ref(), &file);
        state.get_shader(id.as_ref()).expect("Failed to get shader")
    }

    /// Retrieve a cached mesh by its asset name (using the same UUID generation).
    pub fn get_mesh_by_name<S: AsRef<str>>(&self, name: S) -> Option<Arc<GpuMesh>> {
        let key = uuid_from_string(name);
        self.meshes.get(&key).cloned()
    }

    /// Retrieve a cached texture by its asset name.
    pub fn get_texture_by_name<S: AsRef<str>>(&self, name: S) -> Option<Arc<Texture>> {
        let key = uuid_from_string(name);
        self.textures.get(&key).cloned()
    }

    /// Retrieve a cached material by its asset name.
    pub fn get_material_by_name<S: AsRef<str>>(&self, name: S) -> Option<Arc<Material>> {
        let key = uuid_from_string(name);
        self.materials.get(&key).cloned()
    }
}

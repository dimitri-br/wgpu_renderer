// src/renderer/asset_manager.rs

use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::Path;
use std::sync::{Arc, RwLock};
use log::{error, info, warn};
use shipyard::Unique;
use uuid::Uuid;
use wgpu::Device;

use crate::renderer::types::gpu_mesh::GpuMesh;
use crate::renderer::types::mesh::Mesh;
use crate::renderer::types::material::Material;
use crate::renderer::types::texture::Texture;
use crate::renderer::types::uniform::UniformBuffer;
use crate::renderer::shader_reflect::Shader;
use crate::renderer::pipeline_manager::PipelineManager;
use crate::renderer::bind_group_cache::BindGroupCache;

/// Generates a UUID based on a string (e.g., file path or asset name)
/// using version 5 with the URL namespace. This ensures that identical
/// inputs always produce the same UUID.
fn uuid_from_string<S: AsRef<str>>(s: S) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, s.as_ref().as_bytes())
}

/// The AssetManager caches loaded assets so that duplicates are not reloaded.
/// It also stores references to the wgpu Device, Queue, PipelineManager, and
/// BindGroupCache so that it can create GPU resources as needed.
///
/// This struct is intended to be added as a Unique resource in your ECS.
#[derive(Unique)]
pub struct AssetManager {
    // GPU resources needed for creating assets.
    pub device: Arc<Device>,
    pub queue: Arc<wgpu::Queue>,

    // Managers for pipelines and bind groups.
    pub pipeline_manager: Arc<PipelineManager>,
    pub bind_group_cache: Arc<BindGroupCache>,

    // Asset caches, keyed by a UUID generated from a unique asset identifier.
    pub meshes: HashMap<Uuid, Arc<GpuMesh>>,
    pub shaders: HashMap<Uuid, Arc<Shader>>,
    pub materials: HashMap<Uuid, Arc<Material>>,
    pub textures: HashMap<Uuid, Arc<Texture>>,
}

impl AssetManager {
    /// Creates a new AssetManager with empty caches.
    pub fn new(
        device: Arc<Device>,
        queue: Arc<wgpu::Queue>,
        pipeline_manager: Arc<PipelineManager>,
        bind_group_cache: Arc<BindGroupCache>,
    ) -> Self {
        info!("Creating new AssetManager.");
        Self {
            device,
            queue,
            pipeline_manager,
            bind_group_cache,
            meshes: HashMap::new(),
            shaders: HashMap::new(),
            materials: HashMap::new(),
            textures: HashMap::new(),
        }
    }

    // ---------------------
    // Shader Management
    // ---------------------

    /// Gets or creates a shader asset.
    ///
    /// * `id` - A unique identifier (e.g., a name) for the shader.
    /// * `path` - The file path to the WGSL shader source.
    ///
    /// Logs whether the shader was loaded from cache or read from disk.
    pub fn get_or_create_shader<S: AsRef<str>, P: AsRef<Path>>(
        &mut self,
        id: S,
        path: P,
    ) -> Arc<Shader> {
        let key = uuid_from_string(id.as_ref());
        if let Some(shader) = self.shaders.get(&key) {
            info!("Shader '{}' loaded from cache.", id.as_ref());
            return shader.clone();
        }

        info!("Loading shader '{}' from path: {:?}", id.as_ref(), path.as_ref());
        let file = read_to_string(path).expect("Failed to load shader from path!");
        let mut shader_obj = Shader::new(self.device.clone(), &file);
        shader_obj.analyze().expect("Failed to analyze shader!");
        let arc_shader = Arc::new(shader_obj);
        self.shaders.insert(key, arc_shader.clone());
        arc_shader
    }

    /// Retrieves a shader by its unique identifier.
    pub fn get_shader_by_name<S: AsRef<str>>(&self, name: S) -> Option<Arc<Shader>> {
        let key = uuid_from_string(name.as_ref());
        self.shaders.get(&key).cloned()
    }

    // ---------------------
    // Mesh Management
    // ---------------------

    /// Gets or creates a mesh asset from an OBJ file.
    ///
    /// * `path` - The file path to the OBJ file.
    ///
    /// Logs whether the mesh was loaded from cache or read from disk.
    pub fn get_or_create_mesh<P: AsRef<Path>>(
        &mut self,
        path: P,
    ) -> Arc<GpuMesh> {
        let path_str = path.as_ref().to_string_lossy();
        let key = uuid_from_string(&path_str);
        if let Some(mesh) = self.meshes.get(&key) {
            info!("Mesh '{}' loaded from cache.", path_str);
            return mesh.clone();
        }
        info!("Loading mesh from path: {}", path_str);
        let cpu_mesh = Mesh::load_obj(&path)
            .map_err(|e| error!("Error loading mesh '{}': {:?}", path_str, e))
            .expect("Failed to load mesh from path!");
        let gpu_mesh = GpuMesh::from_cpu_mesh(&self.device, &cpu_mesh);
        let gpu_mesh = Arc::new(gpu_mesh);
        self.meshes.insert(key, gpu_mesh.clone());
        gpu_mesh
    }

    /// Retrieves a cached mesh by its name.
    pub fn get_mesh_by_name<S: AsRef<str>>(&self, name: S) -> Option<Arc<GpuMesh>> {
        let key = uuid_from_string(name.as_ref());
        self.meshes.get(&key).cloned()
    }

    // ---------------------
    // Texture Management
    // ---------------------

    /// Gets or creates a texture asset from a file.
    ///
    /// * `name` - A unique name for the texture.
    /// * `path` - The file path to the texture.
    ///
    /// Logs whether the texture was loaded from cache or read from disk.
    pub fn get_or_create_texture<S: AsRef<str>, P: AsRef<Path>>(
        &mut self,
        name: S,
        path: P,
    ) -> Arc<Texture> {
        let key = uuid_from_string(name.as_ref());
        if let Some(texture) = self.textures.get(&key) {
            info!("Texture '{}' loaded from cache.", name.as_ref());
            return texture.clone();
        }
        info!("Loading texture '{}' from path: {:?}", name.as_ref(), path.as_ref());
        let texture = Texture::load_from_file(
            &self.device,
            &self.queue,
            path.as_ref(),
            wgpu::TextureFormat::Rgba8UnormSrgb,
        );
        let arc_tex = Arc::new(texture);
        self.textures.insert(key, arc_tex.clone());
        arc_tex
    }

    pub fn get_or_create_screen_texture(&mut self, name: &str, size: (u32, u32), format: wgpu::TextureFormat) -> Arc<Texture> {
        let key = uuid_from_string(name);
        if let Some(texture) = self.textures.get(&key) {
            info!("Texture '{}' loaded from cache.", name);
            return texture.clone();
        }
        info!("Creating screen texture '{}'.", name);
        let texture = Texture::new_screen_texture(&self.device, &self.queue, size, format, false);
        let arc_tex = Arc::new(texture);
        self.textures.insert(key, arc_tex.clone());
        arc_tex
    }
    
    pub fn replace_screen_texture(&mut self, name: &str, size: (u32, u32), format: wgpu::TextureFormat, is_cube: bool) -> Arc<Texture> {
        let key = uuid_from_string(name);
        if let Some(texture) = self.textures.get(&key) {
            info!("Replacing screen texture '{}'.", name);
            let new_texture = Texture::new_screen_texture(&self.device, &self.queue, size, format, is_cube);
            let arc_tex = Arc::new(new_texture);
            self.textures.insert(key, arc_tex.clone());
            arc_tex
        } else {
            warn!("Screen texture '{}' not found, creating new one.", name);
            self.get_or_create_screen_texture(name, size, format)
        }
    }

    /// Retrieves a cached texture by its name.
    pub fn get_texture_by_name<S: AsRef<str>>(&self, name: S) -> Option<Arc<Texture>> {
        let key = uuid_from_string(name.as_ref());
        self.textures.get(&key).cloned()
    }

    // ---------------------
    // Material Management
    // ---------------------

    /// Gets or creates a material asset.
    ///
    /// * `id` - A unique identifier for the material.
    /// * `shader_name` - The name of the shader this material uses.
    ///
    /// Logs whether the material was loaded from cache or created anew.
    pub fn get_or_create_material<S: AsRef<str>>(
        &mut self,
        id: S,
        shader_name: &str,
    ) -> Arc<Material> {
        let key = uuid_from_string(id.as_ref());
        if let Some(mat) = self.materials.get(&key) {
            info!("Material '{}' loaded from cache.", id.as_ref());
            return mat.clone();
        }

        // Retrieve the shader by name.
        let shader = self
            .get_shader_by_name(shader_name)
            .expect("Shader not loaded or created yet!");
        info!("Creating new material '{}' using shader '{}'.", id.as_ref(), shader_name);
        let material = Material::new(
            shader,
            self.pipeline_manager.clone(),
            self.device.clone(),
            self.bind_group_cache.clone(),
        );
        let mat_arc = Arc::new(material);
        self.materials.insert(key, mat_arc.clone());
        mat_arc
    }

    /// Retrieves a cached material by its name.
    pub fn get_material_by_name<S: AsRef<str>>(&self, name: S) -> Option<Arc<Material>> {
        let key = uuid_from_string(name.as_ref());
        self.materials.get(&key).cloned()
    }
}

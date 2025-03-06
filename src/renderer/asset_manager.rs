use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::Path;
use std::sync::{Arc, RwLock};
use shipyard::Unique;
use uuid::{Uuid};
use wgpu::Device;

use crate::renderer::types::gpu_mesh::GpuMesh;
use crate::renderer::types::mesh::Mesh;
use crate::renderer::types::material::Material;
use crate::renderer::types::texture::Texture;
use crate::renderer::types::uniform::UniformBuffer;
use crate::renderer::shader_reflect::Shader;
use crate::renderer::pipeline_manager::PipelineManager;
use crate::renderer::bind_group_cache::BindGroupCache;

/// Generates a UUID based on a string (e.g., file path or name) using version 5 with the URL namespace.
fn uuid_from_string<S: AsRef<str>>(s: S) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, s.as_ref().as_bytes())
}

/// The AssetManager caches loaded assets so duplicates aren't reloaded.
/// It also stores references to wgpu::Device, wgpu::Queue, etc., so it can create GPU resources.
#[derive(Unique)]
pub struct AssetManager {
    // GPU references
    pub device: Arc<Device>,
    pub queue: Arc<wgpu::Queue>,

    // Pipeline & bind group managers if needed
    pub pipeline_manager: Arc<PipelineManager>,
    pub bind_group_cache: Arc<BindGroupCache>,

    // Cached assets
    pub meshes: HashMap<Uuid, Arc<GpuMesh>>,
    pub shaders: HashMap<Uuid, Arc<Shader>>,
    pub materials: HashMap<Uuid, Arc<Material>>,
    pub textures: HashMap<Uuid, Arc<Texture>>,
    pub uniform_buffers: RwLock<Vec<Arc<UniformBuffer>>>,
}

impl AssetManager {
    pub fn new(
        device: Arc<Device>,
        queue: Arc<wgpu::Queue>,
        pipeline_manager: Arc<PipelineManager>,
        bind_group_cache: Arc<BindGroupCache>,
    ) -> Self {
        Self {
            device,
            queue,
            pipeline_manager,
            bind_group_cache,
            meshes: HashMap::new(),
            shaders: HashMap::new(),
            materials: HashMap::new(),
            textures: HashMap::new(),
            uniform_buffers: RwLock::new(Vec::new()),
        }
    }

    // ---------------------
    // Shader
    // ---------------------
    pub fn get_or_create_shader<S: AsRef<str>, P: AsRef<Path>>(
        &mut self,
        id: S,
        path: P,
    ) -> Arc<Shader> {
        let key = uuid_from_string(id.as_ref());
        if let Some(shader) = self.shaders.get(&key) {
            return shader.clone();
        }

        let file = read_to_string(path).expect("Failed to load shader from path!");
        let mut shader_obj = Shader::new(self.device.clone(), &file);
        shader_obj.analyze().expect("Failed to analyze shader!");
        let arc_shader = Arc::new(shader_obj);
        self.shaders.insert(key, arc_shader.clone());
        arc_shader
    }

    /// Retrieve an already-loaded shader by name, if any.
    pub fn get_shader_by_name<S: AsRef<str>>(&self, name: S) -> Option<Arc<Shader>> {
        let key = uuid_from_string(name.as_ref());
        self.shaders.get(&key).cloned()
    }

    // ---------------------
    // Mesh
    // ---------------------
    pub fn get_or_create_mesh<P: AsRef<Path>>(
        &mut self,
        path: P,
    ) -> Arc<GpuMesh> {
        let path_str = path.as_ref().to_string_lossy();
        let key = uuid_from_string(&path_str);
        if let Some(mesh) = self.meshes.get(&key) {
            return mesh.clone();
        }
        let cpu_mesh = Mesh::load_obj(path).expect("Failed to load mesh");
        let gpu_mesh = GpuMesh::from_cpu_mesh(&self.device, &cpu_mesh);
        let gpu_mesh = Arc::new(gpu_mesh);
        self.meshes.insert(key, gpu_mesh.clone());
        gpu_mesh
    }

    pub fn get_mesh_by_name<S: AsRef<str>>(&self, name: S) -> Option<Arc<GpuMesh>> {
        let key = uuid_from_string(name.as_ref());
        self.meshes.get(&key).cloned()
    }

    // ---------------------
    // Texture
    // ---------------------
    pub fn get_or_create_texture<S: AsRef<str>, P: AsRef<Path>>(
        &mut self,
        name: S,
        path: P,
    ) -> Arc<Texture> {
        let key = uuid_from_string(name.as_ref());
        if let Some(texture) = self.textures.get(&key) {
            return texture.clone();
        }
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

    pub fn get_texture_by_name<S: AsRef<str>>(&self, name: S) -> Option<Arc<Texture>> {
        let key = uuid_from_string(name.as_ref());
        self.textures.get(&key).cloned()
    }

    // ---------------------
    // Material
    // ---------------------
    /// Create or retrieve a material referencing a specific shader name.
    /// If the material already exists under `id`, return it; otherwise create a new one.
    pub fn get_or_create_material<S: AsRef<str>>(
        &mut self,
        id: S,
        shader_name: &str,
    ) -> Arc<Material> {
        let key = uuid_from_string(id.as_ref());
        if let Some(mat) = self.materials.get(&key) {
            return mat.clone();
        }

        // Retrieve the shader
        let shader = self
            .get_shader_by_name(shader_name)
            .expect("Shader not loaded or created yet!");

        // Create a new Material
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

    pub fn get_material_by_name<S: AsRef<str>>(&self, name: S) -> Option<Arc<Material>> {
        let key = uuid_from_string(name.as_ref());
        self.materials.get(&key).cloned()
    }

    // ---------------------
    // Uniform Buffers
    // ---------------------
    pub fn create_uniform_buffer(&self, size: u64) -> Arc<UniformBuffer> {
        let uniform_buffer = UniformBuffer::new(&self.device, &self.queue, size);
        let arc_ub = Arc::new(uniform_buffer);
        self.uniform_buffers.write().unwrap().push(arc_ub.clone());
        arc_ub
    }
}

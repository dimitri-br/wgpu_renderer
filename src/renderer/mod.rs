use std::sync::{Arc, RwLock};
use winit::window::Window;
use wgpu::{Backends, Buffer, BufferDescriptor, BufferUsages, CompositeAlphaMode, Device, DeviceDescriptor, Features, Instance, InstanceDescriptor, InstanceFlags, Limits, PowerPreference, PresentMode, Queue, RequestAdapterOptions, Surface, SurfaceConfiguration, TextureFormat, TextureUsages};
use std::collections::HashMap;
use crate::renderer::bind_group_cache::BindGroupCache;
use crate::renderer::pipeline_manager::PipelineManager;
use crate::renderer::shader_reflect::Shader;
use crate::renderer::types::material::Material;
use crate::renderer::types::sampler::SamplerParameters;
use crate::renderer::types::texture::Texture;
use crate::renderer::types::uniform::UniformBuffer;

pub mod shader_reflect;
pub mod pipeline_manager;
pub mod bind_group_cache;
pub mod types;

pub struct State {
    pub device: Device,
    pub queue: Queue,
    pub surface: Surface<'static>,
    pub surface_config: SurfaceConfiguration,

    pipeline_manager: Arc<PipelineManager>,
    bind_group_cache: Arc<BindGroupCache>,
    shaders: RwLock<HashMap<String, Arc<Shader>>>,
    textures: RwLock<HashMap<String, Arc<Texture>>>,
    uniform_buffers: RwLock<Vec<Arc<UniformBuffer>>>,
}

impl State {
    pub async fn new(window: Arc<Window>) -> Self {
        // Create WGPU instance
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::PRIMARY,
            flags: InstanceFlags::empty(),
            backend_options: Default::default(),
        });

        // Create surface
        let surface = instance.create_surface(window.clone()).unwrap();

        // Choose adapter
        let adapter = instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }).await.unwrap();

        // Request device/queue
        let (device, queue) = adapter.request_device(
            &DeviceDescriptor {
                label: None,
                required_features: Features::empty(),
                required_limits: Limits::default(),
                memory_hints: Default::default(),
            },
            None
        ).await.unwrap();

        // Create surface configuration
        let size = window.inner_size();
        let surface_format = TextureFormat::Bgra8UnormSrgb;
        let surface_config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: PresentMode::Fifo,
            desired_maximum_frame_latency: 3,
            alpha_mode: CompositeAlphaMode::Auto,
            view_formats: vec![surface_format],
        };

        surface.configure(&device, &surface_config);

        let pipeline_manager = Arc::new(PipelineManager::new(device.clone()));
        let bind_group_cache = Arc::new(BindGroupCache::new(device.clone()));
        let shaders = RwLock::new(HashMap::new());
        let textures = RwLock::new(HashMap::new());
        let uniform_buffers = RwLock::new(Vec::new());

        Self {
            device,
            queue,
            surface,
            surface_config,
            pipeline_manager,
            bind_group_cache,
            shaders,
            textures,
            uniform_buffers,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
        }
        else
        {
            log::warn!("Invalid window size: {}x{}", width, height);
        }
    }

    /// Register a WGSL shader by name.
    pub fn register_shader(&self, name: &str, wgsl_source: &str) {
        // Create and analyze
        let mut shader = Shader::new(self.device.clone(), wgsl_source);
        shader.analyze().expect("Failed to analyze shader!");
        self.shaders.write().unwrap().insert(name.to_string(), Arc::new(shader));
    }

    /// Retrieve a shader by name.
    pub fn get_shader(&self, name: &str) -> Option<Arc<Shader>> {
        self.shaders.read().unwrap().get(name).cloned()
    }

    /// Create a material that references a previously-registered shader.
    pub fn create_material(&self, shader_name: &str) -> Material {
        let shader = self.get_shader(shader_name)
            .unwrap_or_else(|| panic!("No shader named '{}'", shader_name));
        Material::new(shader, self.pipeline_manager.clone(), self.device.clone(), self.bind_group_cache.clone())
    }

    pub fn create_texture(&self, name: &str, bytes: &[u8]) -> Arc<Texture> {
        let texture = Texture::load_from_bytes(&self.device, &self.queue, bytes);
        self.textures.write().unwrap().insert(name.to_string(), Arc::new(texture));
        self.textures.read().unwrap().get(name).unwrap().clone()
    }

    pub fn load_texture(&self, name: &str, path: &std::path::Path) -> Arc<Texture> {
        let texture = Texture::load_from_file(&self.device, &self.queue, path);
        self.textures.write().unwrap().insert(name.to_string(), Arc::new(texture));
        self.textures.read().unwrap().get(name).unwrap().clone()
    }

    pub fn create_uniform_buffer(&self, size: u64) -> Arc<UniformBuffer> {
        let uniform_buffer = UniformBuffer::new(&self.device, &self.queue, size);
        let uniform_buffer = Arc::new(uniform_buffer);
        self.uniform_buffers.write().unwrap().push(uniform_buffer.clone());
        uniform_buffer
    }
}
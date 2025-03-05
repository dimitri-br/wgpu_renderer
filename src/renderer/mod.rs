use crate::renderer::bind_group_cache::{BindGroupCache, BindGroupKey};
use crate::renderer::pipeline_manager::PipelineManager;
use crate::renderer::shader_reflect::Shader;
use crate::renderer::types::fps_camera::FpsCamera;
use crate::renderer::types::global::Globals;
use crate::renderer::types::material::Material;
use crate::renderer::types::texture::Texture;
use crate::renderer::types::uniform::UniformBuffer;
use glam::vec3;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use wgpu::{
    Backends, BindGroup, CompositeAlphaMode, Device, DeviceDescriptor, Features, Instance,
    InstanceDescriptor, InstanceFlags, Limits, PowerPreference, PresentMode, Queue,
    RequestAdapterOptions, Surface, SurfaceConfiguration, TextureFormat, TextureUsages,
};
use winit::event::KeyEvent;
use winit::window::Window;

pub mod bind_group_cache;
pub mod pipeline_manager;
pub mod shader_reflect;
pub mod types;

pub struct State {
    pub device: Device,
    pub queue: Queue,
    pub surface: Surface<'static>,
    pub surface_config: SurfaceConfiguration,

    // dt
    delta_time: std::time::Instant,

    pipeline_manager: Arc<PipelineManager>,
    pub bind_group_cache: Arc<BindGroupCache>,
    shaders: RwLock<HashMap<String, Arc<Shader>>>,
    textures: RwLock<HashMap<String, Arc<Texture>>>,
    uniform_buffers: RwLock<Vec<Arc<UniformBuffer>>>,

    camera: FpsCamera,

    global_data: Globals,
    global_uniform_buffer: Arc<UniformBuffer>,
    pub global_bind_group: Arc<BindGroup>,
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
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .unwrap();

        // Request device/queue
        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: None,
                    required_features: Features::default(),
                    required_limits: Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance
                },
                None,
            )
            .await
            .unwrap();

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

        let fps_camera = FpsCamera::new(
            vec3(0.0, 0.0, -3.0),
            //yaw degs
            0.0,
            //pitch degs
            0.0,
            //fovy degs
            45.0,
            //aspect
            size.width as f32 / size.height as f32,
            //znear
            0.1,
            //zfar
            100.0,
            //speed
            1.0,
            //sensitivity
            0.01,
        );

        let mut global_data = Globals::new();

        global_data.update_from_camera(&fps_camera);

        // Create global data bind group
        let (global_uniform_buffer, global_bind_group) = State::create_global_data_bind_group(
            &device,
            &queue,
            global_data,
            bind_group_cache.clone(),
        );

        let delta_time = std::time::Instant::now();

        Self {
            device,
            queue,
            surface,
            surface_config,
            delta_time,
            pipeline_manager,
            bind_group_cache,
            shaders,
            textures,
            uniform_buffers,
            camera: fps_camera,
            global_data,
            global_uniform_buffer,
            global_bind_group,
        }
    }

    fn create_global_data_bind_group(
        device: &Device,
        queue: &Queue,
        global_data: Globals,
        bind_group_cache: Arc<BindGroupCache>,
    ) -> (Arc<UniformBuffer>, Arc<BindGroup>) {
        let global_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Global Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::all(),
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    min_binding_size: None,
                    has_dynamic_offset: false,
                },
                count: None,
            }],
        });

        let global_buffer = Arc::new(UniformBuffer::new(
            &device,
            &queue,
            std::mem::size_of::<Globals>() as u64,
        ));
        global_buffer.update(&global_data);

        let global_bind_group = bind_group_cache.get_or_create(
            &global_layout,
            &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(global_buffer.get_buffer_binding()),
            }],
            BindGroupKey::new(
                &global_layout,
                vec![Arc::<UniformBuffer>::as_ptr(&global_buffer) as usize],
            ),
        );

        (global_buffer, global_bind_group)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);

            // Update camera aspect ratio
            self.camera.set_aspect(width as f32 / height as f32);
        } else {
            log::warn!("Invalid window size: {}x{}", width, height);
        }
    }

    pub fn update(&mut self) {
        let dt = self.delta_time.elapsed().as_secs_f32();
        println!("DT: {:?}", dt);
        println!("FPS: {:?}", 1.0 / dt);
        self.camera.update(dt);
        self.global_data.update_from_camera(&self.camera);
        self.global_uniform_buffer.update(&self.global_data);
        self.delta_time = std::time::Instant::now();
    }

    pub fn handle_keyboard(&mut self, key_event: KeyEvent) {
        self.camera.process_keyboard(key_event);
    }

    pub fn handle_mouse(&mut self, delta: (f64, f64)) {
        self.camera.process_mouse(delta.0 as f32, delta.1 as f32);
    }

    /// Register a WGSL shader by name.
    pub fn register_shader(&self, name: &str, wgsl_source: &str) {
        // Create and analyze
        let mut shader = Shader::new(self.device.clone(), wgsl_source);
        shader.analyze().expect("Failed to analyze shader!");
        self.shaders
            .write()
            .unwrap()
            .insert(name.to_string(), Arc::new(shader));
    }

    /// Retrieve a shader by name.
    pub fn get_shader(&self, name: &str) -> Option<Arc<Shader>> {
        self.shaders.read().unwrap().get(name).cloned()
    }

    /// Create a material that references a previously-registered shader.
    pub fn create_material(&self, shader_name: &str) -> Material {
        let shader = self
            .get_shader(shader_name)
            .unwrap_or_else(|| panic!("No shader named '{}'", shader_name));
        Material::new(
            shader,
            self.pipeline_manager.clone(),
            self.device.clone(),
            self.bind_group_cache.clone(),
        )
    }

    pub fn create_texture(&self, name: &str, bytes: &[u8]) -> Arc<Texture> {
        let texture = Texture::load_from_bytes(&self.device, &self.queue, bytes);
        self.textures
            .write()
            .unwrap()
            .insert(name.to_string(), Arc::new(texture));
        self.textures.read().unwrap().get(name).unwrap().clone()
    }

    pub fn load_texture(&self, name: &str, path: &std::path::Path) -> Arc<Texture> {
        let texture = Texture::load_from_file(&self.device, &self.queue, path);
        self.textures
            .write()
            .unwrap()
            .insert(name.to_string(), Arc::new(texture));
        self.textures.read().unwrap().get(name).unwrap().clone()
    }

    pub fn create_uniform_buffer(&self, size: u64) -> Arc<UniformBuffer> {
        let uniform_buffer = UniformBuffer::new(&self.device, &self.queue, size);
        let uniform_buffer = Arc::new(uniform_buffer);
        self.uniform_buffers
            .write()
            .unwrap()
            .push(uniform_buffer.clone());
        uniform_buffer
    }
}

pub mod types;
pub mod asset_manager;
pub mod components;
pub mod bind_group_cache;
pub mod pipeline_manager;
pub mod shader_reflect;
pub mod systems;

use std::sync::Arc;
use shipyard::Unique;
use wgpu::{Backends, CompositeAlphaMode, Device, DeviceDescriptor, Features, Instance, InstanceDescriptor, InstanceFlags, Limits, PowerPreference, PresentMode, Queue, RequestAdapterOptions, Surface, SurfaceConfiguration, TextureFormat, TextureUsages};
use winit::window::Window;
use crate::renderer::bind_group_cache::{BindGroupCache, BindGroupKey};
use crate::renderer::pipeline_manager::PipelineManager;
use crate::renderer::types::fps_camera::FpsCamera;
use crate::renderer::types::global::Globals;
use crate::renderer::types::texture::Texture;
use crate::renderer::types::uniform::UniformBuffer;

#[derive(Unique)]
pub struct State {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub surface: Surface<'static>,
    pub surface_config: SurfaceConfiguration,

    pub pipeline_manager: Arc<PipelineManager>,
    pub bind_group_cache: Arc<BindGroupCache>,

    pub depth_texture: Option<Texture>,

    pub camera: FpsCamera,
    pub global_data: Globals,
    pub global_uniform_buffer: Arc<UniformBuffer>,
    pub global_bind_group: Arc<wgpu::BindGroup>,

    delta_time: std::time::Instant,
}

impl State {
    pub async fn new(window: Arc<Window>) -> Self {
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::PRIMARY,
            flags: InstanceFlags::empty(),
            backend_options: Default::default(),
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }).await.unwrap();

        let (device, queue) = adapter.request_device(
            &DeviceDescriptor {
                label: None,
                required_features: Features::PUSH_CONSTANTS,
                required_limits: Limits {
                    max_push_constant_size: 256,
                    ..Default::default()
                },
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None
        ).await.unwrap();

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let size = window.inner_size();
        let surface_format = TextureFormat::Bgra8UnormSrgb;
        let surface_config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: PresentMode::Immediate,
            desired_maximum_frame_latency: 3,
            alpha_mode: CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        surface.configure(&device, &surface_config);

        let pipeline_manager = Arc::new(PipelineManager::new(device.clone()));
        let bind_group_cache = Arc::new(BindGroupCache::new(device.clone()));

        let fps_camera = FpsCamera::new(
            glam::vec3(0.0, 0.0, -3.0),
            0.0, 0.0,
            45.0,
            size.width as f32 / size.height as f32,
            0.1, 100.0,
            1.0,
            0.01,
        );

        let mut global_data = Globals::new();
        global_data.update_from_camera(&fps_camera);

        // Create global uniform + bind group
        let global_uniform_buffer = Arc::new(UniformBuffer::new(&device, &queue, std::mem::size_of::<Globals>() as u64));
        global_uniform_buffer.update(&global_data);

        let global_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Global BindGroupLayout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        min_binding_size: None,
                        has_dynamic_offset: false,
                    },
                    count: None,
                }
            ]
        });

        let global_bg_key = BindGroupKey::new(&global_layout, vec![Arc::as_ptr(&global_uniform_buffer) as usize]);
        let global_bind_group = bind_group_cache.get_or_create(&global_layout, &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(global_uniform_buffer.get_buffer_binding()),
            }
        ], global_bg_key);

        let depth_texture = Some(Texture::new_screen_texture(&device, &queue, (surface_config.width, surface_config.height), wgpu::TextureFormat::Depth32Float));

        Self {
            device,
            queue,
            surface,
            surface_config,
            pipeline_manager,
            bind_group_cache,
            depth_texture,
            camera: fps_camera,
            global_data,
            global_uniform_buffer,
            global_bind_group,
            delta_time: std::time::Instant::now(),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
            self.depth_texture = Some(Texture::new_screen_texture(&self.device, &self.queue, (width, height), wgpu::TextureFormat::Depth32Float));

            self.camera.set_aspect(width as f32 / height as f32);
        } else {
            log::warn!("Invalid window size: {}x{}", width, height);
        }
    }

    pub fn update(&mut self) {
        let dt = self.delta_time.elapsed().as_secs_f32();
        self.camera.update(dt);
        self.global_data.update_from_camera(&self.camera);
        self.global_uniform_buffer.update(&self.global_data);
        self.delta_time = std::time::Instant::now();
    }
    
    // Input handling (camera movement)
    pub fn handle_keyboard(&mut self, event: winit::event::KeyEvent) {
        self.camera.process_keyboard(event);
    }

    pub fn handle_mouse(&mut self, delta: (f64, f64)) {
        self.camera.process_mouse(delta.0 as f32, delta.1 as f32);
    }
}

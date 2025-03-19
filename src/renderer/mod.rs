pub mod types;
pub mod asset_manager;
pub mod bind_group_cache;
pub mod pipeline_manager;
pub mod shader_reflect;
pub mod ecs;
pub mod auto_mipmapper;
pub mod shadow_atlas;
mod render_graph;
mod shadow_data_storage;
pub mod light_storage;
mod gpu_storage;
pub mod render_batcher;

use std::sync::Arc;
use log::{error, info};
use shipyard::Unique;
use wgpu::{BackendOptions, Backends, CompositeAlphaMode, Device, DeviceDescriptor, Dx12BackendOptions, Dx12Compiler, Features, Instance, InstanceDescriptor, InstanceFlags, Limits, PowerPreference, PresentMode, Queue, RequestAdapterOptions, Surface, SurfaceConfiguration, TextureFormat, TextureUsages};
use winit::window::Window;
use crate::renderer::bind_group_cache::{BindGroupCache, BindGroupKey};
use ecs::global_component::GlobalComponent;
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

    pub delta_time: f32,
    last_update: std::time::Instant,

    // We don't want to resize until the next frame
    should_resize: bool,
    resize_size: (u32, u32),
}

impl State {
    pub async fn new(window: Arc<Window>) -> Self {
        info!("Initializing state...");
        let instance = Instance::new(&InstanceDescriptor {
            backends: if cfg!(target_arch = "wasm32") {
                Backends::BROWSER_WEBGPU
            } else if cfg!(target_os = "macos") {
                Backends::METAL
            } else {
                Backends::DX12
            },
            flags: InstanceFlags::default(),
            backend_options: BackendOptions{
                gl: Default::default(),
                dx12: Dx12BackendOptions{
                    shader_compiler: Dx12Compiler::DynamicDxc {
                        dxc_path: "bin/dxcompiler.dll".to_string(),
                        dxil_path: "bin/dxil.dll".to_string(),
                    },
                }
            }
        });

        let surface = instance.create_surface(window.clone())
            .map_err(|e| error!("Failed to create surface: {}", e)).unwrap();

        let adapter = match instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }).await{
            Some(a) => a,
            None => {
                error!("Failed to find a suitable adapter");
                panic!("Failed to find a suitable adapter");
            }
        };


        let (device, queue) = match adapter.request_device(
            &DeviceDescriptor {
                label: None,
                required_features: Features::PUSH_CONSTANTS
                    | Features::TEXTURE_COMPRESSION_BC
                | Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
                | Features::TEXTURE_FORMAT_16BIT_NORM,
                required_limits: Limits {
                    max_push_constant_size: 128,
                    max_texture_dimension_2d: 8192,
                    ..Default::default()
                },
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None
        ).await {
            Ok((d, q)) => (Arc::new(d), Arc::new(q)),
            Err(e) => {
                error!("Failed to create device and queue: {}", e);
                panic!("Failed to create device and queue");
            }
        };
        
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
            view_formats: vec![],
        };
        surface.configure(&device, &surface_config);

        info!("State initialized");

        let pipeline_manager = Arc::new(PipelineManager::new(device.clone()));
        let bind_group_cache = Arc::new(BindGroupCache::new(device.clone()));

        Self {
            device,
            queue,
            surface,
            surface_config,
            pipeline_manager,
            bind_group_cache,
            delta_time: 0.0,
            last_update: std::time::Instant::now(),
            should_resize: false,
            resize_size: (0, 0),
        }
    }

    pub fn trigger_resize(&mut self, width: u32, height: u32) {
        self.should_resize = true;
        self.resize_size = (width, height);
    }

    pub fn resize(&mut self) {
        if !self.should_resize {
            return;
        }
        self.should_resize = false;

        let (width, height) = self.resize_size;
        if width > 0 && height > 0 {
            info!("Resizing window to {}x{}", width, height);
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
        } else {
            log::warn!("Invalid window size: {}x{}", width, height);
        }
    }

    pub fn update(&mut self) {
        self.delta_time = self.last_update.elapsed().as_secs_f32();
        self.last_update = std::time::Instant::now();
    }

    pub fn get_screen_size(&self) -> (u32, u32) {
        (self.surface_config.width, self.surface_config.height)
    }

    pub fn get_aspect_ratio(&self) -> f32 {
        self.surface_config.width as f32 / self.surface_config.height as f32
    }
}

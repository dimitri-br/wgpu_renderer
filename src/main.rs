use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use wgpu::*;
use winit::event::*;
use winit::event_loop::EventLoopBuilder;
use winit::window::Window;
use crate::renderer::material::Material;
use crate::renderer::pipeline_manager::PipelineManager;
use crate::renderer::shader_reflect::Shader;

mod renderer;

use renderer::texture::Texture;

pub struct State {
    pub device: Arc<Device>,
    pub queue: Queue,
    pub surface: Surface<'static>,
    pub surface_config: SurfaceConfiguration,

    pipeline_manager: Arc<PipelineManager>,
    shaders: RwLock<HashMap<String, Arc<Shader>>>,
    textures: RwLock<HashMap<String, Arc<Texture>>>,
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

        let device = Arc::new(device);

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
        let shaders = RwLock::new(HashMap::new());
        let textures = RwLock::new(HashMap::new());

        Self {
            device,
            queue,
            surface,
            surface_config,
            pipeline_manager,
            shaders,
            textures,
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
        Material::new(shader, self.pipeline_manager.clone(), self.device.clone())
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
}

fn main() {
    // Create event loop and window
    let event_loop = EventLoopBuilder::new().build().unwrap();
    let window = Arc::new(Window::new(&event_loop).unwrap());

    // init env_logger
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let mut state = pollster::block_on(State::new(window.clone()));

    state.register_shader("main", include_str!("../shaders/shader.wgsl"));

    let mut material = state.create_material("main");
    material.set_cull_mode(Some(Face::Back));
    material.set_transparent(false);

    let texture = state.load_texture("texture", std::path::Path::new("assets/texture.png"));
    material.set_texture("color_texture", texture.view.clone());
    material.set_sampler("color_sampler", Arc::new(state.device.create_sampler(&SamplerDescriptor {
        label: Some("Color Sampler"),
        address_mode_u: AddressMode::Repeat,
        address_mode_v: AddressMode::Repeat,
        address_mode_w: AddressMode::Repeat,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        mipmap_filter: FilterMode::Linear,
        lod_min_clamp: 0.0,
        lod_max_clamp: 1000.0,
        compare: None,
        anisotropy_clamp: 1,
        border_color: None,
    })));

    // Run the event loop
    event_loop.run(move |event, tgt| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    tgt.exit();
                },
                WindowEvent::Resized(size) => {
                    state.resize(size.width, size.height);
                },
                WindowEvent::RedrawRequested => {
                    // Acquire next swapchain frame
                    let frame = match state.surface.get_current_texture() {
                        Ok(frame) => frame,
                        Err(_e) => {
                            // reconfigure or skip
                            return;
                        }
                    };

                    // Create a command encoder
                    let mut encoder = state.device.create_command_encoder(&CommandEncoderDescriptor {
                        label: Some("Main Command Encoder"),
                    });


                    let frame_view = frame.texture.create_view(&Default::default());

                    {
                        let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                            label: Some("Render Pass"),
                            color_attachments: &[Some(RenderPassColorAttachment {
                                view: &frame_view,
                                resolve_target: None,
                                ops: Operations{
                                    load: LoadOp::Load,
                                    store: StoreOp::Store
                                }
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });

                        rpass.set_pipeline(&material.get_pipeline());

                        material.bind(&mut rpass);

                        rpass.draw(0..3, 0..1);
                    }

                    // Submit command buffers
                    state.queue.submit(std::iter::once(encoder.finish()));
                    frame.present();
                },
                _ => {}
            },
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => {}
        }
    }).unwrap();
}

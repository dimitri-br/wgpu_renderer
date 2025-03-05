#![feature(duration_millis_float)]

use crate::renderer::bind_group_cache::BindGroupKey;
use crate::renderer::types::gpu_mesh::GpuMesh;
use crate::renderer::types::mesh::Mesh;
use crate::renderer::types::sampler::SamplerParameters;
use crate::renderer::types::transform::Transform;
use crate::renderer::types::uniform::UniformBuffer;
use renderer::State;
use std::fs::read_to_string;
use std::sync::Arc;
use wgpu::*;
use winit::event::*;
use winit::event_loop::{ControlFlow, EventLoopBuilder};
use winit::window::Window;

mod renderer;

fn main() {
    // Create event loop and window
    let event_loop = EventLoopBuilder::new().build().unwrap();
    let window = Arc::new(Window::new(&event_loop).unwrap());

    // init env_logger
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let mut state = pollster::block_on(State::new(window.clone()));

    state.register_shader(
        "main",
        &*read_to_string("assets/shaders/shader.wgsl").unwrap(),
    );

    let mut material = state.create_material("main");
    material.set_cull_mode(Some(Face::Back));
    material.set_transparent(false);
    material.set_front_face(FrontFace::Ccw);

    let texture = state.load_texture("texture", std::path::Path::new("assets/capsule0.jpg"));

    material.set_texture("color_texture", texture.view.clone());
    material.set_sampler(
        "color_sampler",
        SamplerParameters {
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 1.0,
            ..Default::default()
        },
    );

    let mesh = Mesh::load_obj(std::path::Path::new("assets/capsule.obj")).unwrap();
    let gpu_mesh = GpuMesh::from_cpu_mesh(&state.device, &mesh);

    let transform_uniform = state.create_uniform_buffer(std::mem::size_of::<Transform>() as u64);
    let mut transform = Transform::new();
    transform.set_transform(
        glam::Vec3::new(0.0, 0.0, 1.0),
        glam::Quat::from_euler(glam::EulerRot::XYZ, 45.0, 45.0, 45.0),
        glam::Vec3::new(0.3, 0.3, 0.3),
    );

    transform_uniform.update(&transform);

    let transform_uniform_bg_entry = BindGroupEntry {
        binding: 0,
        resource: BindingResource::Buffer(transform_uniform.get_buffer_binding()),
    };

    let shader = material.get_shader();
    let layout = shader.get_bind_group_layout(2).unwrap();
    let key = BindGroupKey::new(
        layout,
        vec![Arc::<UniformBuffer>::as_ptr(&transform_uniform) as usize],
    );
    let bg = state.bind_group_cache.get_or_create(
        layout,
        &vec![transform_uniform_bg_entry.clone()],
        key,
    );

    // Run the event loop
    event_loop
        .run(move |event, tgt| {
            tgt.set_control_flow(ControlFlow::Poll);
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => {
                        tgt.exit();
                    }
                    WindowEvent::Resized(size) => {
                        state.resize(size.width, size.height);
                    }
                    WindowEvent::RedrawRequested => {
                        state.update();
                        // Acquire next swapchain frame
                        let frame = match state.surface.get_current_texture() {
                            Ok(frame) => frame,
                            Err(_e) => {
                                // reconfigure or skip
                                return;
                            }
                        };

                        // Create a command encoder
                        let mut encoder =
                            state
                                .device
                                .create_command_encoder(&CommandEncoderDescriptor {
                                    label: Some("Main Command Encoder"),
                                });

                        let frame_view = frame.texture.create_view(&Default::default());

                        {
                            let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                                label: Some("Render Pass"),
                                color_attachments: &[Some(RenderPassColorAttachment {
                                    view: &frame_view,
                                    resolve_target: None,
                                    ops: Operations {
                                        load: LoadOp::Load,
                                        store: StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: None,
                                timestamp_writes: None,
                                occlusion_query_set: None,
                            });

                            rpass.set_pipeline(&material.get_pipeline());

                            rpass.set_bind_group(0, &*state.global_bind_group, &[]);

                            material.bind(&mut rpass);

                            rpass.set_bind_group(2, &*bg, &[]);

                            gpu_mesh.draw(&mut rpass);
                        }

                        // Submit command buffers
                        state.queue.submit(std::iter::once(encoder.finish()));
                        frame.present();
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        state.handle_keyboard(event);
                    }
                    _ => {}
                },
                winit::event::Event::DeviceEvent { event, .. } => {
                    if let winit::event::DeviceEvent::MouseMotion { delta } = event {
                        state.handle_mouse(delta);
                    }
                }
                Event::AboutToWait => {
                    window.request_redraw();
                }
                _ => {}
            }
        })
        .unwrap();
}

use crate::renderer::bind_group_cache::BindGroupKey;
use crate::renderer::types::gpu_mesh::GpuMesh;
use crate::renderer::types::mesh::Mesh;
use crate::renderer::types::sampler::SamplerParameters;
use crate::renderer::types::transform::Transform;
use crate::renderer::types::uniform::UniformBuffer;
use renderer::systems::load_assets;
use renderer::State;
use std::fs::read_to_string;
use std::sync::Arc;
use shipyard::World;
use wgpu::*;
use winit::event::*;
use winit::event_loop::{ControlFlow, EventLoopBuilder};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window};
use crate::renderer::asset_manager::AssetManager;
use crate::renderer::components::{MaterialComponent, MeshComponent, TransformComponent};
use crate::renderer::systems::{add_entities, handle_keyboard_input, handle_mouse_input, render_system, resize_system, update_system};

mod renderer;

fn main() {
    // Create event loop and window
    let event_loop = EventLoopBuilder::new().build().unwrap();
    let window = Arc::new(Window::new(&event_loop).unwrap());

    // init env_logger
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();


    let world = World::new();

    let state = pollster::block_on(State::new(window.clone()));
    let asset_manager = AssetManager::new(
        state.device.clone(),
        state.queue.clone(),
        state.pipeline_manager.clone(),
        state.bind_group_cache.clone(),
    );

    world.add_unique(state);
    world.add_unique(asset_manager);

    world.run(load_assets);
    world.run(add_entities);

    // Capture the mouse
    window.set_cursor_grab(CursorGrabMode::Confined).unwrap();
    window.set_cursor_visible(false);

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
                        world.run_with_data(resize_system, (size.width, size.height));
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        world.run_with_data(handle_keyboard_input, event.clone());

                        match event.physical_key{
                            PhysicalKey::Code(code) => {
                                match code {
                                    KeyCode::Escape => {
                                        tgt.exit();
                                    }
                                    _ => {}
                                }
                            }
                            _ => {
                                // Ignore non-code keys
                            }
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        world.run_with_data(render_system, &world);
                    }
                    _ => {}
                },
                Event::DeviceEvent { event, .. } => {
                    if let DeviceEvent::MouseMotion { delta } = event {
                        world.run_with_data(handle_mouse_input, delta);
                    }
                }
                Event::AboutToWait => {
                    world.run(update_system);
                    window.request_redraw();
                }
                _ => {}
            }
        })
        .unwrap();
}

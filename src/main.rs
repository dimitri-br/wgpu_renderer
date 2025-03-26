use renderer::ecs::systems::load_assets;
use renderer::State;
use std::sync::Arc;
use log::error;
use shipyard::World;
use winit::event::*;
use winit::event_loop::{ControlFlow, EventLoopBuilder};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window};
use crate::renderer::asset_manager::AssetManager;
use renderer::ecs::systems::{add_entities, handle_keyboard_input, handle_mouse_input, resize_system, update_system};
use renderer::ecs::global_component::GlobalComponent;
use crate::renderer::auto_mipmapper::AutoMipmapper;
use crate::renderer::ecs::camera_component::CameraComponent;
use crate::renderer::ecs::instancing_component::InstancingComponent;
use crate::renderer::ecs::light_manager::LightManager;
use crate::renderer::ecs::systems::{update_lighting, mipmap_system, render_graph_system, update_instancing};
use crate::renderer::render_batcher::RenderBatcher;
use crate::renderer::shadow_atlas::ShadowAtlas;
use crate::renderer::types::fps_camera::FpsCamera;

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
    let auto_mipmapper = AutoMipmapper::new(state.device.clone(), wgpu::TextureFormat::Rgba8UnormSrgb);
    let shadow_atlas = ShadowAtlas::new(&state.device.clone(), 2048*2, 2048, wgpu::TextureFormat::Depth24Plus);
    let light_manager = LightManager::new(state.device.clone(), state.queue.clone());
    let global_component = GlobalComponent::new(&state, &light_manager.shadow_data_storage, &light_manager.light_storage);
    let instancing_component = InstancingComponent::new(state.device.clone(), state.queue.clone(), 1000);
    let render_batcher = RenderBatcher::new();
    let camera_component: CameraComponent = FpsCamera::new(
        glam::vec3(0.0, 0.0, -3.0),
        0.0, 0.0,
        45.0,
        state.get_aspect_ratio(),
        0.1, 100.0,
        5.0,
        0.01,
    ).into();

    world.add_unique(state);
    world.add_unique(asset_manager);
    world.add_unique(auto_mipmapper);
    world.add_unique(shadow_atlas);
    world.add_unique(light_manager);
    world.add_unique(global_component);
    world.add_unique(instancing_component);
    world.add_unique(render_batcher);
    world.add_unique(camera_component);

    // Load assets, then add entities
    // Lastly, generate mipmaps
    world.run(load_assets);
    world.run(add_entities);
    world.run(mipmap_system);

    // Capture the mouse
    if cfg!(target_os = "windows") || cfg!(target_os = "linux") {
        window.set_cursor_grab(CursorGrabMode::Confined).unwrap();
    }else if cfg!(target_os = "macos") {
        window.set_cursor_grab(CursorGrabMode::Locked).unwrap();
    }else{
        error!("Unable to capture mouse");
    }
    window.set_cursor_visible(false);

    world.run(update_system);
    world.run(update_lighting);
    world.run(update_instancing);

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
                        world.run(render_graph_system);
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
                    world.run(update_lighting);
                    world.run(update_instancing);
                    window.request_redraw();
                }
                _ => {}
            }
        })
        .unwrap();
}

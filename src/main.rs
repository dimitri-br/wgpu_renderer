use crate::renderer::bind_group_cache::BindGroupKey;
use crate::renderer::types::gpu_mesh::GpuMesh;
use crate::renderer::types::mesh::Mesh;
use crate::renderer::types::sampler::SamplerParameters;
use crate::renderer::types::transform::Transform;
use crate::renderer::types::uniform::UniformBuffer;
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
use crate::renderer::systems::{handle_keyboard_input, handle_mouse_input, render_system, resize_system, update_system};

mod renderer;

fn main() {
    // Create event loop and window
    let event_loop = EventLoopBuilder::new().build().unwrap();
    let window = Arc::new(Window::new(&event_loop).unwrap());

    // init env_logger
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let state = pollster::block_on(State::new(window.clone()));

    let world = World::new();

    world.add_unique(state);

    let asset_manager = AssetManager::new();

    world.add_unique(asset_manager);

    /*let mut material = state.create_material("main");
    material.set_cull_mode(None);
    material.set_transparent(false);
    material.set_front_face(FrontFace::Cw);
    material.set_depth(true);

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

    let mut transform = Transform::new();
    transform.set_transform(
        glam::Vec3::new(0.0, 0.0, 1.0),
        glam::Quat::from_euler(glam::EulerRot::XYZ, -90.0, 45.0, 0.0),
        glam::Vec3::new(0.3, 0.3, 0.3),
    );


    let mesh_comp = MeshComponent{
        mesh: Arc::new(gpu_mesh),
    };
    let material_comp = MaterialComponent{
        material: Arc::new(material),
    };
    let transform_comp = TransformComponent{
        transform
    };

    // Spawn new entity with the components
    let entity = world.add_entity((mesh_comp, material_comp, transform_comp));*/

    // Capture the mouse
    window.set_cursor_grab(CursorGrabMode::Locked).unwrap();
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

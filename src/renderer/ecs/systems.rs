use std::ops::Deref;
use glam::vec3;
use log::{error, info};
use shipyard::EntitiesViewMut;
use shipyard::{IntoIter, UniqueView, UniqueViewMut, View, ViewMut};
use wgpu::{Color, LoadOp, Operations, RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor, StoreOp, TextureFormat};
use winit::event::KeyEvent;
use crate::renderer::ecs::components::*;
use crate::renderer::State;
use crate::renderer::ecs::render_graphics_view::RenderGraphicsViewMut;
use crate::renderer::asset_manager::AssetManager;
use crate::renderer::ecs::camera_component::CameraComponent;
use crate::renderer::ecs::global_component::GlobalComponent;
use crate::renderer::types::camera::Camera;
use crate::renderer::types::light::Light;
use crate::renderer::types::sampler::SamplerParameters;
use crate::renderer::types::transform::Transform;

pub fn load_assets( mut state: UniqueViewMut<State>, mut asset_manager: UniqueViewMut<AssetManager>){
    let mesh = asset_manager.get_or_create_mesh("assets/capsule.obj");
    // load a texture
    let texture = asset_manager.get_or_create_texture("capsule_tex", "assets/capsule0.jpg");

    // create a shader
    let shader = asset_manager.get_or_create_shader("main", "assets/shaders/shader.wgsl");

    let sampler = SamplerParameters {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        lod_min_clamp: 0.0,
        lod_max_clamp: 1.0,
        ..Default::default()
    };

    // create a material referencing that shader
    let material = asset_manager.get_or_create_material("capsule_mat", "main");
    material.set_cull_mode(Some(wgpu::Face::Back));
    material.set_depth(true);
    material.set_transparent(false);
    material.set_texture("color_texture", texture.view.clone());
    material.set_sampler("color_sampler", sampler.clone());


    // GBuffer
    let albedo_texture = asset_manager.get_or_create_screen_texture("albedo_texture", state.get_screen_size(), wgpu::TextureFormat::Bgra8UnormSrgb);
    let position_texture = asset_manager.get_or_create_screen_texture("position_texture", state.get_screen_size(), wgpu::TextureFormat::Bgra8UnormSrgb);
    let normal_texture = asset_manager.get_or_create_screen_texture("normal_texture", state.get_screen_size(), wgpu::TextureFormat::Bgra8UnormSrgb);
    let depth_texture = asset_manager.get_or_create_screen_texture("depth_texture", state.get_screen_size(), wgpu::TextureFormat::Depth32Float);

    let output_texture = asset_manager.get_or_create_screen_texture("output_texture", state.get_screen_size(), wgpu::TextureFormat::Bgra8UnormSrgb);
    let gbuffer_shader = asset_manager.get_or_create_shader("gbuffer", "assets/shaders/deferred.wgsl");
    let gbuffer_material = asset_manager.get_or_create_material("gbuffer_mat", "gbuffer");
    gbuffer_material.set_cull_mode(None);
    gbuffer_material.set_depth(false);
    gbuffer_material.set_transparent(false);
    gbuffer_material.set_texture("g_albedo", albedo_texture.view.clone());
    gbuffer_material.set_texture("g_position", position_texture.view.clone());
    gbuffer_material.set_texture("g_normal", normal_texture.view.clone());
    gbuffer_material.set_texture("g_depth", depth_texture.view.clone());
    gbuffer_material.set_sampler("g_sampler", sampler.clone());

    let post_processing_shader = asset_manager.get_or_create_shader("invert", "assets/shaders/invert.wgsl");
    let post_processing_material = asset_manager.get_or_create_material("invert_mat", "invert");
    post_processing_material.set_cull_mode(None);
    post_processing_material.set_depth(false);
    post_processing_material.set_transparent(false);
    post_processing_material.set_sampler("u_sampler", sampler.clone());


}
pub fn add_entities(mut entities: EntitiesViewMut, asset_manager: UniqueView<AssetManager>, mut lights: ViewMut<LightComponent>, mut meshes: ViewMut<MeshComponent>, mut materials: ViewMut<MaterialComponent>, mut transforms: ViewMut<TransformComponent>){
    entities.bulk_add_entity((&mut meshes, &mut materials, &mut transforms), (0..100).map(|n| {
        let mesh_component = MeshComponent{
            mesh: asset_manager.get_mesh_by_name("assets/capsule.obj").unwrap()
        };
        let material_component = MaterialComponent{
            material: asset_manager.get_material_by_name("capsule_mat").unwrap()
        };

        let mut transform = Transform::new();
        // Grid
        let x = (n % 10) as f32;
        let z = (n / 10) as f32;
        transform.translate(vec3(x * 2.0 - 10.0, 0.0, z * 2.0 - 10.0));
        transform.scale(vec3(0.5, 0.5, 0.5));
        let transform_component = TransformComponent{
            transform
        };
        (mesh_component, material_component, transform_component)
    }));




    entities.bulk_add_entity((&mut lights, &mut transforms), (0..10).map(|n| {
        // Scatter a few lights around
        let mut light_transform = Transform::new();
        light_transform.translate(vec3((n % 10) as f32, 2.0, (n / 10) as f32));
        light_transform.scale(vec3(0.1, 0.1, 0.1));

        let light = Light::new(light_transform.translation(), vec3(1.0, 1.0, 1.0), 1.0);
        let light_component = LightComponent{
            light
        };

        let transform_component = TransformComponent{
            transform: light_transform
        };
        (light_component, transform_component)
    }));
}

pub fn handle_keyboard_input(key_event: KeyEvent, mut camera_component: UniqueViewMut<CameraComponent>) {
    camera_component.camera.process_keyboard(key_event);
}

pub fn handle_mouse_input(delta: (f64, f64), mut state: UniqueViewMut<CameraComponent>) {
    state.camera.process_mouse(delta.0 as f32, delta.1 as f32);
}

pub fn resize_system((width, height): (u32, u32), mut state: UniqueViewMut<State>, mut asset_manager: UniqueViewMut<AssetManager>, mut camera_component: UniqueViewMut<CameraComponent>, mut global_component: UniqueViewMut<GlobalComponent>) {
    state.trigger_resize(width, height);
    camera_component.camera.resize(width as f32, height as f32);
    global_component.global_data.update_screen_size(width as f32, height as f32);
    // Resize the output texture
    asset_manager.replace_screen_texture("output_texture", (width, height), TextureFormat::Bgra8UnormSrgb);
    // Resize the GBuffer textures
    asset_manager.replace_screen_texture("albedo_texture", (width, height), TextureFormat::Bgra8UnormSrgb);
    asset_manager.replace_screen_texture("position_texture", (width, height), TextureFormat::Bgra8UnormSrgb);
    asset_manager.replace_screen_texture("normal_texture", (width, height), TextureFormat::Bgra8UnormSrgb);
    asset_manager.replace_screen_texture("depth_texture", (width, height), TextureFormat::Depth32Float);

    // We need to re-reference the material to update the texture views
    let gbuffer_material = asset_manager.get_material_by_name("gbuffer_mat").unwrap();
    gbuffer_material.set_texture("g_albedo", asset_manager.get_texture_by_name("albedo_texture").unwrap().view.clone());
    gbuffer_material.set_texture("g_position", asset_manager.get_texture_by_name("position_texture").unwrap().view.clone());
    gbuffer_material.set_texture("g_normal", asset_manager.get_texture_by_name("normal_texture").unwrap().view.clone());
    gbuffer_material.set_texture("g_depth", asset_manager.get_texture_by_name("depth_texture").unwrap().view.clone());
}

pub fn update_system(mut state: UniqueViewMut<State>, mut global_component: UniqueViewMut<GlobalComponent>, mut camera_component: UniqueViewMut<CameraComponent>) {
    state.update();
    camera_component.camera.update(state.delta_time);
    global_component.global_data.update_from_camera(camera_component.camera.deref());
    global_component.global_uniform_buffer.update(&global_component.global_data);
}

pub fn light_update_system(mut global_component: UniqueViewMut<GlobalComponent>, lights: View<LightComponent>) {
    error!("Updating light buffer");

    let mut light_data = Vec::new();
    for light in lights.iter() {
        light_data.push(light.light);
    }
    global_component.light_storage.set_lights(light_data);

    if global_component.light_storage.delta {
        global_component.reconstruct_bind_group();
    }

    global_component.light_storage.update();


}

pub fn render_system(mut graphics: RenderGraphicsViewMut, asset_manager: UniqueView<AssetManager>, mesh_comps: View<MeshComponent>, mat_comps: View<MaterialComponent>, transform_comps: View<TransformComponent>){
    let albedo_texture = asset_manager.get_texture_by_name("albedo_texture").unwrap();
    let position_texture = asset_manager.get_texture_by_name("position_texture").unwrap();
    let normal_texture = asset_manager.get_texture_by_name("normal_texture").unwrap();
    let depth_texture = asset_manager.get_texture_by_name("depth_texture").unwrap();

    let albedo_view = albedo_texture.view.clone();
    let position_view = position_texture.view.clone();
    let normal_view = normal_texture.view.clone();
    let depth_view = depth_texture.view.clone();

    let output_texture = asset_manager.get_texture_by_name("output_texture").unwrap();
    let output_view = output_texture.view.clone();

    let gbuffer_material = asset_manager.get_material_by_name("gbuffer_mat").unwrap();
    let gbuffer_pipeline = gbuffer_material.get_pipeline();

    let post_processing_material = asset_manager.get_material_by_name("invert_mat").unwrap();
    let post_processing_pipeline = post_processing_material.get_pipeline();

    info!("Rendering frame");
    post_processing_material.set_texture("u_texture", output_texture.view.clone());
    {
        // No Depth render pass
        let mut render_pass = graphics.encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[
                Some(RenderPassColorAttachment {
                    view: &albedo_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0
                        }),
                        store: StoreOp::Store,
                    },
                }),
                Some(RenderPassColorAttachment {
                    view: &position_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0
                        }),
                        store: StoreOp::Store,
                    },
                }),
                Some(RenderPassColorAttachment {
                    view: &normal_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0
                        }),
                        store: StoreOp::Store,
                    },
                }),
            ],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Bind global resources first (group 0)
        render_pass.set_bind_group(0, &*graphics.global_component.global_bind_group, &[]);

        // Get the pipeline from the material.
        (&mesh_comps, &mat_comps, &transform_comps).iter().for_each(|(mesh_comp, mat_comp, transform_comp)| {
            if mat_comp.material.get_depth() {
                return;
            }

            let pipeline = mat_comp.material.get_pipeline();
            render_pass.set_pipeline(&pipeline);

            mat_comp.material.bind(&mut render_pass);
            // Bind per-object data (e.g., the transform)
            // Here, you could have a system that either uses a per-object uniform buffer
            // or dynamic offsets to bind the transform data in group 2.
            // For simplicity, assume we have a helper:
            render_pass.set_push_constants(
                wgpu::ShaderStages::all(),
                0,
                bytemuck::cast_slice(&[transform_comp.transform]),
            );

            // Draw the mesh.
            mesh_comp.mesh.draw(&mut render_pass);
        });
    }
    info!("Depth pass");
    {
        // Depth-Enabled render pass
        let mut render_pass = graphics.encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Depth Render Pass"),
            color_attachments: &[
                Some(RenderPassColorAttachment {
                    view: &albedo_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                }),
                Some(RenderPassColorAttachment {
                    view: &position_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                }),
                Some(RenderPassColorAttachment {
                    view: &normal_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                }),
            ],
            depth_stencil_attachment: Some(
                RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(
                        Operations {
                            load: LoadOp::Clear(1.0),
                            store: StoreOp::Store,
                        }
                    ),
                    stencil_ops: None
                }
            ),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Bind global resources first (group 0)
        render_pass.set_bind_group(0, &*graphics.global_component.global_bind_group, &[]);

        // Get the pipeline from the material.
        (&mesh_comps, &mat_comps, &transform_comps).iter().for_each(|(mesh_comp, mat_comp, transform_comp)| {
            if !mat_comp.material.get_depth() {
                return;
            }

            let pipeline = mat_comp.material.get_pipeline();
            render_pass.set_pipeline(&pipeline);

            mat_comp.material.bind(&mut render_pass);
            // Bind per-object data (e.g., the transform)
            // Here, you could have a system that either uses a per-object uniform buffer
            // or dynamic offsets to bind the transform data in group 2.
            // For simplicity, assume we have a helper:
            render_pass.set_push_constants(
                wgpu::ShaderStages::all(),
                0,
                bytemuck::cast_slice(&[transform_comp.transform]),
            );

            // Draw the mesh.
            mesh_comp.mesh.draw(&mut render_pass);
        });
    }
    info!("GBuffer pass");
    // GBuffer composite pass
    {
        let mut render_pass = graphics.encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("GBuffer Pass"),
            color_attachments: &[
                Some(RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0
                        }),
                        store: StoreOp::Store,
                    },
                }),
            ],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&gbuffer_pipeline);
        render_pass.set_bind_group(0, &*graphics.global_component.global_bind_group, &[]);
        gbuffer_material.bind(&mut render_pass);
        render_pass.draw(0..3, 0..1);
    }
    info!("Post Processing pass");
    // Post-Processing pass
    {
        let mut render_pass = graphics.encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Post Processing Pass"),
            color_attachments: &[
                Some(RenderPassColorAttachment {
                    view: &graphics.view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0
                        }),
                        store: StoreOp::Store,
                    },
                }),
            ],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&post_processing_pipeline);
        render_pass.set_bind_group(0, &*graphics.global_component.global_bind_group, &[]);
        post_processing_material.bind(&mut render_pass);
        render_pass.draw(0..3, 0..1);
    }
}

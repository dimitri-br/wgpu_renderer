// src/renderer/ecs/systems.rs

use std::ops::Deref;
use std::sync::Arc;

use glam::{vec3, vec4, Vec4Swizzles};
use log::{error, info, warn};
use rand::random;
use shipyard::{
    EntitiesViewMut, IntoIter, UniqueView, UniqueViewMut, View, ViewMut,
};
use wgpu::{
    Color, LoadOp, Operations, RenderPassColorAttachment, RenderPassDepthStencilAttachment,
    RenderPassDescriptor, StoreOp, TextureFormat,
};
use winit::event::KeyEvent;
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::renderer::asset_manager::AssetManager;
use crate::renderer::ecs::camera_component::CameraComponent;
use crate::renderer::ecs::global_component::GlobalComponent;
use crate::renderer::ecs::light_manager::LightManager;
use crate::renderer::ecs::render_graphics_view::RenderGraphicsViewMut;
use crate::renderer::ecs::components::*;
use crate::renderer::ecs::light_update_view::LightUpdateViewMut;
use crate::renderer::render_graph::{RenderGraph, RenderGraphContext, RenderGraphNode};
use crate::renderer::shadow_atlas::ShadowAtlas;
use crate::renderer::shadow_data_storage::ShadowDataStorage;
use crate::renderer::State;
use crate::renderer::types::camera::Camera;
use crate::renderer::types::light::Light;
use crate::renderer::types::light_type::LightType;
use crate::renderer::types::material::Material;
use crate::renderer::types::sampler::SamplerParameters;
use crate::renderer::types::shadow_data::ShadowData;
use crate::renderer::types::transform::Transform;
use crate::renderer::types::uniform::{Uniform, UniformBuffer};

/// Loads assets (meshes, textures, shaders, materials, and screen textures) into the asset manager.
pub fn load_assets(
    mut state: UniqueViewMut<State>,
    mut asset_manager: UniqueViewMut<AssetManager>,
    mut auto_mipmapper: UniqueViewMut<crate::renderer::auto_mipmapper::AutoMipmapper>,
    shadow_atlas: UniqueView<ShadowAtlas>,
) {
    // Load meshes and textures.
    let mesh = asset_manager.get_or_create_mesh("assets/capsule.obj");
    let texture = asset_manager.get_or_create_texture("capsule_tex", "assets/capsule0.jpg");
    let box_mesh = asset_manager.get_or_create_mesh("assets/cube.obj");
    let white_texture = asset_manager.get_or_create_texture("white_tex", "assets/solid_white.png");

    // GBuffer setup.
    let screen_size = state.get_screen_size();
    let albedo_texture = asset_manager.get_or_create_screen_texture(
        "albedo_texture",
        screen_size,
        wgpu::TextureFormat::Rgba16Float,
    );
    let normal_texture = asset_manager.get_or_create_screen_texture(
        "normal_texture",
        screen_size,
        wgpu::TextureFormat::Rgba16Float,
    );
    let depth_texture = asset_manager.get_or_create_screen_texture(
        "depth_texture",
        screen_size,
        wgpu::TextureFormat::Depth32Float,
    );
    let output_texture = asset_manager.get_or_create_screen_texture(
        "output_texture",
        screen_size,
        wgpu::TextureFormat::Rgba16Float,
    );

    // Generate mipmaps for the texture.
    let mut encoder = state.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Texture Mipmapping Encoder"),
    });
    auto_mipmapper.generate_mipmaps(&mut encoder, &[texture.clone()], &[texture.mip_level_count]);
    state.queue.submit(std::iter::once(encoder.finish()));

    // Load shaders.
    asset_manager.get_or_create_shader("main", "assets/shaders/shader.wgsl");
    asset_manager.get_or_create_shader("shadow", "assets/shaders/shadow.wgsl");
    asset_manager.get_or_create_shader("gbuffer", "assets/shaders/deferred.wgsl");
    asset_manager.get_or_create_shader("invert", "assets/shaders/post_process.wgsl");

    // Define a sampler.
    let sampler = SamplerParameters {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        lod_min_clamp: 0.0,
        lod_max_clamp: 4.0,
        anisotropy_clamp: 16,
        ..Default::default()
    };

    // Create materials using a helper.
    create_material_with(&mut asset_manager, "capsule_mat", "main", |material| {
        material.set_cull_mode(None);
        material.set_depth(true);
        material.set_transparent(false);
        material.set_texture("color_texture", texture.view.clone());
        material.set_sampler("color_sampler", sampler.clone());
    });

    create_material_with(&mut asset_manager, "box_mat", "main", |material| {
        material.set_cull_mode(None);
        material.set_depth(true);
        material.set_transparent(false);
        material.set_texture("color_texture", white_texture.view.clone());
        material.set_sampler("color_sampler", sampler.clone());
    });

    create_material_with(&mut asset_manager, "gbuffer_mat", "gbuffer", |material| {
        material.set_cull_mode(Some(wgpu::Face::Back));
        material.set_depth(false);
        material.set_transparent(false);
        material.set_texture("g_albedo", albedo_texture.view.clone());
        material.set_texture("g_normal", normal_texture.view.clone());
        material.set_texture("g_depth", depth_texture.view.clone());
        material.set_sampler("g_sampler", sampler.clone());
        material.set_sampler("shadow_sampler", shadow_atlas.shadow_sampler.clone());
    });

    create_material_with(&mut asset_manager, "invert_mat", "invert", |material| {
        material.set_cull_mode(Some(wgpu::Face::Front));
        material.set_depth(false);
        material.set_transparent(false);
        material.set_sampler("u_sampler", sampler.clone());
    });

    create_material_with(&mut asset_manager, "shadow_mat", "shadow", |material| {
        material.set_cull_mode(Some(wgpu::Face::Front));
        material.set_depth(true);
        material.set_transparent(false);
    });
}

/// Helper to create materials.
fn create_material_with<F>(
    asset_manager: &mut AssetManager,
    name: &str,
    shader_name: &str,
    config: F,
) -> Arc<Material>
where
    F: FnOnce(Arc<Material>),
{
    let material = asset_manager.get_or_create_material(name, shader_name);
    config(material.clone());
    material
}

/// Adds entities to the ECS world.
pub fn add_entities(
    mut entities: EntitiesViewMut,
    asset_manager: UniqueView<AssetManager>,
    mut shadow_atlas: UniqueViewMut<ShadowAtlas>,
    mut global_component: UniqueViewMut<GlobalComponent>,
    mut meshes: ViewMut<MeshComponent>,
    mut materials: ViewMut<MaterialComponent>,
    mut transforms: ViewMut<TransformComponent>,
    mut shadow_cast_component: ViewMut<ShadowCastComponent>,
    mut lights: ViewMut<LightComponent>,
    mut light_manager: UniqueViewMut<LightManager>,
) {
    // Ground entity.
    entities.add_entity(
        (&mut meshes, &mut materials, &mut transforms, &mut shadow_cast_component),
        (
            MeshComponent {
                mesh: asset_manager.get_mesh_by_name("assets/cube.obj").unwrap(),
            },
            MaterialComponent {
                material: asset_manager.get_material_by_name("box_mat").unwrap(),
            },
            TransformComponent {
                transform: {
                    let mut t = Transform::new();
                    t.translate(vec3(0.0, -1.0, 0.0));
                    t.scale(vec3(100.0, 0.1, 100.0));
                    t
                },
            },
            ShadowCastComponent { shadow_cast: false },
        ),
    );

    // Box entity.
    entities.add_entity(
        (&mut meshes, &mut materials, &mut transforms, &mut shadow_cast_component),
        (
            MeshComponent {
                mesh: asset_manager.get_mesh_by_name("assets/cube.obj").unwrap(),
            },
            MaterialComponent {
                material: asset_manager.get_material_by_name("box_mat").unwrap(),
            },
            TransformComponent {
                transform: {
                    let mut t = Transform::new();
                    t.translate(vec3(0.0, 1.0, 0.0));
                    t.scale(vec3(1.0, 1.0, 1.0));
                    t
                },
            },
            ShadowCastComponent { shadow_cast: true },
        ),
    );

    // Spawn many renderable entities.
    entities.bulk_add_entity(
        (&mut meshes, &mut materials, &mut transforms, &mut shadow_cast_component),
        (0..40).map(|_| {
            let mesh_component = MeshComponent {
                mesh: asset_manager.get_mesh_by_name("assets/capsule.obj").unwrap(),
            };
            let material_component = MaterialComponent {
                material: asset_manager.get_material_by_name("capsule_mat").unwrap(),
            };
            let mut transform = Transform::new();
            let x: f32 = random::<f32>() * 30.0 - 15.0;
            let z: f32 = random::<f32>() * 30.0 - 15.0;
            transform.translate(vec3(x * 2.0 - 10.0, 0.0, z * 2.0 - 10.0));
            transform.rotate(glam::Quat::from_euler(
                glam::EulerRot::YXZ,
                random::<f32>() * 360.0,
                random::<f32>() * 360.0,
                random::<f32>() * 360.0,
            ));
            transform.scale(vec3(0.5, 0.5, 0.5));
            (
                mesh_component,
                material_component,
                TransformComponent { transform },
                ShadowCastComponent { shadow_cast: true },
            )
        }),
    );

    // Create a directional light using the manager.
    let dir_light = light_manager.create_directional_light(&mut shadow_atlas);
    let dir_light_component = LightComponent::new(dir_light, LightType::Directional);
    entities.add_entity(
        (&mut lights, &mut transforms),
        (dir_light_component, TransformComponent { transform: Transform::new() }),
    );

    // Create point lights.
    for _ in 0..4 {
        let position = vec3(
            random::<f32>() * 25.0 - 12.5,
            10.0,
            random::<f32>() * 25.0 - 12.5,
        );
        let color = match random::<u8>() % 3 {
            0 => vec3(1.0, 0.0, 0.0),
            1 => vec3(0.0, 1.0, 0.0),
            _ => vec3(0.0, 0.0, 1.0),
        };
        let intensity = random::<f32>() * 5.0 + 2.5;
        let range = 15.0;
        let point_light = light_manager.create_point_light(position, color, intensity, range, &mut shadow_atlas);
        let point_light_component = LightComponent::new(point_light, LightType::Point);
        let transform_component = TransformComponent { transform: Transform::new() };
        entities.add_entity((&mut lights, &mut transforms), (point_light_component, transform_component));
    }

    // Create a spotlight.
    let spot_position = vec3(-8.0, 2.0, 0.0);
    let spot_direction = vec3(0.5, -0.2, 0.0);
    let spot_color = vec3(1.0, 1.0, 0.8);
    let spot_intensity = 2.0;
    let spot_range = 40.0;
    let spot_angle = std::f32::consts::FRAC_PI_6;
    let spot_light = light_manager.create_spot_light(spot_position, spot_direction, spot_color, spot_intensity, spot_range, spot_angle, &mut shadow_atlas);
    let spot_light_component = LightComponent::new(spot_light, LightType::Spot);
    let spot_transform = TransformComponent {
        transform: {
            let mut t = Transform::new();
            t.translate(spot_position);
            t
        },
    };
    entities.add_entity((&mut lights, &mut transforms), (spot_light_component, spot_transform));
}

/// Handles keyboard input for the camera.
pub fn handle_keyboard_input(
    key_event: KeyEvent,
    mut camera_component: UniqueViewMut<CameraComponent>,
) {
    camera_component.camera.process_keyboard(key_event);
}

/// Handles mouse input for the camera.
pub fn handle_mouse_input(
    delta: (f64, f64),
    mut camera_component: UniqueViewMut<CameraComponent>,
) {
    camera_component.camera.process_mouse(delta.0 as f32, delta.1 as f32);
}

/// Handles window resize events.
pub fn resize_system(
    (width, height): (u32, u32),
    mut state: UniqueViewMut<State>,
    mut asset_manager: UniqueViewMut<AssetManager>,
    mut camera_component: UniqueViewMut<CameraComponent>,
    mut global_component: UniqueViewMut<GlobalComponent>,
    shadow_atlas: UniqueView<ShadowAtlas>,
) {
    state.trigger_resize(width, height);
    camera_component.camera.resize(width as f32, height as f32);
    global_component.global_data.update_screen_size(width as f32, height as f32);

    asset_manager.replace_screen_texture("output_texture", (width, height), TextureFormat::Rgba16Float, false);
    asset_manager.replace_screen_texture("albedo_texture", (width, height), TextureFormat::Rgba16Float, false);
    asset_manager.replace_screen_texture("normal_texture", (width, height), TextureFormat::Rgba16Float, false);
    asset_manager.replace_screen_texture("depth_texture", (width, height), TextureFormat::Depth32Float, false);

    let gbuffer_material = asset_manager.get_material_by_name("gbuffer_mat").unwrap();
    let albedo_tex = asset_manager.get_texture_by_name("albedo_texture").unwrap();
    let normal_tex = asset_manager.get_texture_by_name("normal_texture").unwrap();
    let depth_tex = asset_manager.get_texture_by_name("depth_texture").unwrap();
    let output_tex = asset_manager.get_texture_by_name("output_texture").unwrap();
    gbuffer_material.set_texture("g_albedo", albedo_tex.view.clone());
    gbuffer_material.set_texture("g_normal", normal_tex.view.clone());
    gbuffer_material.set_texture("g_depth", depth_tex.view.clone());
    gbuffer_material.set_texture("shadow_map", shadow_atlas.texture.view.clone());

    let post_processing_material = asset_manager.get_material_by_name("invert_mat").unwrap();
    post_processing_material.set_texture("u_texture", output_tex.view.clone());
}

/// Updates global state, camera, and uniform buffers.
pub fn update_system(
    mut state: UniqueViewMut<State>,
    mut global_component: UniqueViewMut<GlobalComponent>,
    mut camera_component: UniqueViewMut<CameraComponent>,
) {
    state.update();
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f32();
    global_component.global_data.update(time);
    camera_component.camera.update(state.delta_time);
    global_component.global_data.update_from_camera(camera_component.camera.deref());
    global_component.global_uniform_buffer.update(&global_component.global_data);
}

pub fn light_update_system(
    mut light_update: LightUpdateViewMut,
    mut lights: shipyard::ViewMut<LightComponent>,
) {
    // Collect all lights into a contiguous vector.
    let mut light_data: Vec<Light> = lights.iter().map(|lc| lc.light.clone()).collect();

    // Use the custom view to update all lights in batch.
    light_update.light_manager.update_lights(&mut light_data, &light_update.camera_component.camera);

    // Write back the updated light data to each LightComponent.
    for (mut light_comp, updated_light) in (&mut lights).iter().zip(light_data.into_iter()) {
        light_comp.light = updated_light;
    }
}

/// Helper to create a RenderPassColorAttachment with a clear color.
fn generate_color_attachment(view: &wgpu::TextureView) -> RenderPassColorAttachment {
    RenderPassColorAttachment {
        view,
        resolve_target: None,
        ops: Operations {
            load: LoadOp::Clear(Color::BLACK),
            store: StoreOp::Store,
        },
    }
}

/// The main render graph system integrated into the ECS.
pub fn render_graph_system(
    mut graphics: RenderGraphicsViewMut,
    asset_manager: UniqueView<AssetManager>,
    mesh_comps: View<MeshComponent>,
    mat_comps: View<MaterialComponent>,
    transform_comps: View<TransformComponent>,
    shadow_cast_component: View<ShadowCastComponent>,
) {
    let shadow_atlas_view = graphics.shadow_atlas.texture.view.clone();
    let mut context = RenderGraphContext {
        encoder: &mut graphics.encoder,
        asset_manager: &*asset_manager,
        global_component: &*graphics.global_component,
        output_view: graphics.view.clone(),
        shadow_atlas_view,
    };

    let mut render_graph = RenderGraph::new();

    // Depth Pass Node.
    render_graph.add_node(RenderGraphNode {
        name: "depth_pass".into(),
        dependencies: vec![],
        execute: Box::new(|ctx: &mut RenderGraphContext| {
            let albedo_tex = ctx.asset_manager.get_texture_by_name("albedo_texture").unwrap();
            let normal_tex = ctx.asset_manager.get_texture_by_name("normal_texture").unwrap();
            let depth_tex = ctx.asset_manager.get_texture_by_name("depth_texture").unwrap();
            let albedo_view = albedo_tex.view.clone();
            let normal_view = normal_tex.view.clone();
            let depth_view = depth_tex.view.clone();

            if let Some(encoder) = ctx.encoder {
                let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Depth Pass"),
                    color_attachments: &[
                        Some(generate_color_attachment(&albedo_view)),
                        Some(generate_color_attachment(&normal_view)),
                    ],
                    depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                        view: &depth_view,
                        depth_ops: Some(Operations {
                            load: LoadOp::Clear(1.0),
                            store: StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_bind_group(0, &*ctx.global_component.global_bind_group, &[]);
                (&mesh_comps, &mat_comps, &transform_comps).iter().for_each(|(mesh_comp, mat_comp, transform_comp)| {
                    if !mat_comp.material.get_depth() {
                        return;
                    }
                    pass.set_pipeline(&mat_comp.material.get_pipeline());
                    mat_comp.material.bind(&mut pass);
                    pass.set_push_constants(
                        wgpu::ShaderStages::VERTEX_FRAGMENT,
                        0,
                        bytemuck::cast_slice(&[transform_comp.transform]),
                    );
                    mesh_comp.mesh.draw(&mut pass);
                });
            }
        }),
    });

    // Shadow Pass Node.
    render_graph.add_node(RenderGraphNode {
        name: "shadow_pass".into(),
        dependencies: vec!["depth_pass".into()],
        execute: Box::new(|ctx: &mut RenderGraphContext| {
            let shadow_material = ctx.asset_manager.get_material_by_name("shadow_mat").unwrap();
            let shadow_view = ctx.shadow_atlas_view.clone();

            if let Some(encoder) = ctx.encoder {
                let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Shadow Pass"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                        view: &shadow_view,
                        depth_ops: Some(Operations {
                            load: LoadOp::Clear(1.0),
                            store: StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_pipeline(&shadow_material.get_pipeline());
                // Fetch updated light/shadow data from the LightManager.
                let shadow_data = graphics.light_manager.shadow_data_storage.get_all_shadow_data();
                let light_data = graphics.light_manager.light_storage.get_all_lights();
                for light in light_data.iter() {
                    for i in 0..light.shadow_data_count {
                        let offset = light.shadow_data_offset as usize + i as usize;
                        let smc = &shadow_data[offset];
                        let rect = smc.tile.read().unwrap().rect;
                        pass.set_viewport(
                            rect.x as f32,
                            rect.y as f32,
                            rect.width as f32,
                            rect.height as f32,
                            0.0,
                            1.0,
                        );
                        pass.set_scissor_rect(
                            rect.x as u32,
                            rect.y as u32,
                            rect.width as u32,
                            rect.height as u32,
                        );
                        pass.set_bind_group(0, &*ctx.global_component.global_bind_group, &[]);
                        (&mesh_comps, &transform_comps, &shadow_cast_component)
                            .iter()
                            .for_each(|(mesh_comp, transform_comp, shadow_caster)| {
                                if shadow_caster.shadow_cast {
                                    let model_matrix = [transform_comp.transform.matrix];
                                    let mut push_data = Vec::with_capacity(32);
                                    push_data.extend_from_slice(bytemuck::bytes_of(&model_matrix));
                                    push_data.extend_from_slice(bytemuck::bytes_of(&smc.shadow_data.light_view_proj));
                                    pass.set_push_constants(
                                        wgpu::ShaderStages::VERTEX_FRAGMENT,
                                        0,
                                        &push_data,
                                    );
                                    mesh_comp.mesh.draw(&mut pass);
                                }
                            });
                    }
                }
            }
        }),
    });

    // GBuffer Composite Pass Node.
    render_graph.add_node(RenderGraphNode {
        name: "gbuffer_pass".into(),
        dependencies: vec!["shadow_pass".into()],
        execute: Box::new(|ctx: &mut RenderGraphContext| {
            let output = ctx.asset_manager.get_texture_by_name("output_texture").unwrap();
            let gbuffer_material = ctx.asset_manager.get_material_by_name("gbuffer_mat").unwrap();
            let pipeline = gbuffer_material.get_pipeline();
            gbuffer_material.set_texture("shadow_map", ctx.shadow_atlas_view.clone());
            if let Some(encoder) = ctx.encoder {
                let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("GBuffer Pass"),
                    color_attachments: &[Some(generate_color_attachment(&output.view))],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &*ctx.global_component.global_bind_group, &[]);
                gbuffer_material.bind(&mut pass);
                pass.draw(0..3, 0..1);
            }
        }),
    });

    // Post-Processing Pass Node.
    render_graph.add_node(RenderGraphNode {
        name: "post_process".into(),
        dependencies: vec!["gbuffer_pass".into()],
        execute: Box::new(|ctx: &mut RenderGraphContext| {
            let post_material = ctx.asset_manager.get_material_by_name("invert_mat").unwrap();
            let texture = ctx.asset_manager.get_texture_by_name("output_texture").unwrap();
            post_material.set_texture("u_texture", texture.view.clone());
            let pipeline = post_material.get_pipeline();
            let final_view = ctx.output_view.clone();
            if let Some(encoder) = ctx.encoder {
                let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Post Processing Pass"),
                    color_attachments: &[Some(generate_color_attachment(&final_view))],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &*ctx.global_component.global_bind_group, &[]);
                post_material.bind(&mut pass);
                pass.draw(0..3, 0..1);
            }
        }),
    });

    render_graph.build_dependency_graph();
    render_graph.execute(&mut context);
}

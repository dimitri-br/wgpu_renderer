// src/renderer/ecs/systems.rs

use std::ops::Deref;
use std::sync::Arc;
use glam::vec3;
use log::{error, info, warn};
use rand::random;
use shipyard::{
    EntitiesViewMut, IntoIter, UniqueView, UniqueViewMut, View, ViewMut,
};
use wgpu::{
    Color, LoadOp, Operations, RenderPassColorAttachment, RenderPassDepthStencilAttachment,
    RenderPassDescriptor, StoreOp, TextureFormat,
};
use winit::event::{KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::renderer::ecs::components::*;
use crate::renderer::ecs::camera_component::CameraComponent;
use crate::renderer::ecs::global_component::GlobalComponent;
use crate::renderer::ecs::render_graphics_view::RenderGraphicsViewMut;
use crate::renderer::asset_manager::AssetManager;
use crate::renderer::render_graph::{RenderGraph, RenderGraphContext, RenderGraphNode};
use crate::renderer::shadow_atlas::ShadowAtlas;
use crate::renderer::State;
use crate::renderer::types::camera::Camera;
use crate::renderer::types::light::Light;
use crate::renderer::types::light_type::LightType;
use crate::renderer::types::material::Material;
use crate::renderer::types::sampler::SamplerParameters;
use crate::renderer::types::transform::Transform;
use crate::renderer::types::uniform::{Uniform, UniformBuffer};

/// Loads assets (meshes, textures, shaders, materials, and screen textures)
/// into the asset manager.
pub fn load_assets(
    mut state: UniqueViewMut<State>,
    mut asset_manager: UniqueViewMut<AssetManager>,
    mut auto_mipmapper: UniqueViewMut<crate::renderer::auto_mipmapper::AutoMipmapper>,
    shadow_atlas: UniqueView<ShadowAtlas>,
) {
    // Load mesh and texture.
    let mesh = asset_manager.get_or_create_mesh("assets/capsule.obj");
    let texture = asset_manager.get_or_create_texture("capsule_tex", "assets/capsule0.jpg");

    let box_mesh = asset_manager.get_or_create_mesh("assets/box.obj");
    let white_texture = asset_manager.get_or_create_texture("white_tex", "assets/solid_white.png");

    // GBuffer setup.
    let screen_size = state.get_screen_size();
    let albedo_texture = asset_manager.get_or_create_screen_texture(
        "albedo_texture", screen_size, wgpu::TextureFormat::Bgra8UnormSrgb,
    );
    let normal_texture = asset_manager.get_or_create_screen_texture(
        "normal_texture", screen_size, wgpu::TextureFormat::Bgra8UnormSrgb,
    );
    let depth_texture = asset_manager.get_or_create_screen_texture(
        "depth_texture", screen_size, wgpu::TextureFormat::Depth32Float,
    );
    let output_texture = asset_manager.get_or_create_screen_texture(
        "output_texture", screen_size, wgpu::TextureFormat::Bgra8UnormSrgb,
    );

    // Generate mipmaps for the texture.
    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
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

    // Capsule material but with the helper function
    create_material_with(&mut asset_manager, "capsule_mat", "main", |material| {
        material.set_cull_mode(Some(wgpu::Face::Back));
        material.set_depth(true);
        material.set_transparent(false);
        material.set_texture("color_texture", texture.view.clone());
        material.set_sampler("color_sampler", sampler.clone());
    });

    create_material_with(&mut asset_manager, "box_mat", "main", |material| {
        material.set_cull_mode(Some(wgpu::Face::Back));
        material.set_depth(true);
        material.set_transparent(false);
        material.set_texture("color_texture", white_texture.view.clone());
        material.set_sampler("color_sampler", sampler.clone());
    });

    create_material_with(&mut asset_manager, "gbuffer_mat", "gbuffer", |material| {
        material.set_cull_mode(None);
        material.set_depth(false);
        material.set_transparent(false);
        material.set_texture("g_albedo", albedo_texture.view.clone());
        material.set_texture("g_normal", normal_texture.view.clone());
        material.set_texture("g_depth", depth_texture.view.clone());
        material.set_sampler("g_sampler", sampler.clone());
        material.set_sampler("shadow_sampler", shadow_atlas.shadow_sampler.clone());
    });

    create_material_with(&mut asset_manager, "invert_mat", "invert", |material| {
        material.set_cull_mode(None);
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

fn create_material_with<F>(
    asset_manager: &mut AssetManager,
    name: &str,
    shader_name: &str,
    config: F,
) -> Arc<Material>
where
    F: FnOnce(Arc<Material>)
{
    let material = asset_manager.get_or_create_material(name, shader_name);
    config(material.clone());
    material
}


/// Adds entities to the ECS world.
pub fn add_entities(
    mut entities: EntitiesViewMut,
    asset_manager: UniqueView<AssetManager>,
    mut meshes: ViewMut<MeshComponent>,
    mut materials: ViewMut<MaterialComponent>,
    mut transforms: ViewMut<TransformComponent>,
    mut shadow_cast_component: ViewMut<ShadowCastComponent>,
    mut lights: ViewMut<LightComponent>,
) {
    // Spawn one box entity. Make it really long and wide, but thin.
    entities.add_entity(
        (&mut meshes, &mut materials, &mut transforms, &mut shadow_cast_component),
        (
            MeshComponent {
                mesh: asset_manager.get_mesh_by_name("assets/box.obj").unwrap(),
            },
            MaterialComponent {
                material: asset_manager.get_material_by_name("box_mat").unwrap(),
            },
            TransformComponent {
                transform: (|| {
                    let mut t = Transform::new();
                    t.translate(vec3(0.0, -1.0, 0.0));
                    t.scale(vec3(100.0, 0.1, 100.0));
                    t
                })(),
            },
            ShadowCastComponent {
                shadow_cast: false,
            },
        ),
    );

    entities.add_entity(
        (&mut meshes, &mut materials, &mut transforms, &mut shadow_cast_component),
        (
            MeshComponent {
                mesh: asset_manager.get_mesh_by_name("assets/box.obj").unwrap(),
            },
            MaterialComponent {
                material: asset_manager.get_material_by_name("box_mat").unwrap(),
            },
            TransformComponent {
                transform: (|| {
                    let mut t = Transform::new();
                    t.translate(vec3(0.0, 1.0, 0.0));
                    t.scale(vec3(1.0, 1.0, 1.0));
                    t
                })(),
            },
            ShadowCastComponent {
                shadow_cast: true,
            },
        ),
    );

    // Spawn many renderable entities.
    entities.bulk_add_entity(
        (&mut meshes, &mut materials, &mut transforms, &mut shadow_cast_component),
        (0..150).map(|_| {
            let mesh_component = MeshComponent {
                mesh: asset_manager.get_mesh_by_name("assets/capsule.obj").unwrap(),
            };
            let material_component = MaterialComponent {
                material: asset_manager.get_material_by_name("capsule_mat").unwrap(),
            };

            let mut transform = Transform::new();
            // Randomize position.
            let x: f32 = random::<f32>() * 30.0 - 15.0;
            let z: f32 = random::<f32>() * 30.0 - 15.0;
            transform.translate(vec3(x * 2.0 - 10.0, 0.0, z * 2.0 - 10.0));
            // Random rotation.
            transform.rotate(glam::Quat::from_euler(
                glam::EulerRot::YXZ,
                random::<f32>() * 360.0,
                random::<f32>() * 360.0,
                random::<f32>() * 360.0,
            ));
            // Scale.
            transform.scale(vec3(0.5, 0.5, 0.5));
            let transform_component = TransformComponent { transform };

            let shadow_cast_component = ShadowCastComponent {
                shadow_cast: true,
            };

            (mesh_component, material_component, transform_component, shadow_cast_component)
        }),
    );

    // Spawn a few light entities.
    entities.bulk_add_entity(
        (&mut lights, &mut transforms),
        (0..2).map(|_| {
            let mut light_transform = Transform::new();
            light_transform.translate(vec3(
                random::<f32>() * 25.0 - 12.5,
                2.0,
                random::<f32>() * 25.0 - 12.5,
            ));
            // Color is either red, green, or blue.
            let color = match random::<u8>() % 3 {
                0 => vec3(1.0, 0.0, 0.0),
                1 => vec3(0.0, 1.0, 0.0),
                _ => vec3(0.0, 0.0, 1.0),
            };
            let intensity = random::<f32>() * 10.0 + 10.0;
            let range = 10.0;
            let rotation = vec3(0.0, 0.0, 0.0);
            let light = Light::new(light_transform.translation(), rotation, color, intensity, range);
            let light_component = LightComponent::new(light, LightType::Point);
            let transform_component = TransformComponent { transform: light_transform };

            (light_component, transform_component)
        }),
    );
}

/// Handles keyboard input for the camera.
pub fn handle_keyboard_input(key_event: KeyEvent, mut camera_component: UniqueViewMut<CameraComponent>) {
    camera_component.camera.process_keyboard(key_event);
}

/// Handles mouse input for the camera.
pub fn handle_mouse_input(delta: (f64, f64), mut camera_component: UniqueViewMut<CameraComponent>) {
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

    // Replace screen textures in the asset manager.
    asset_manager.replace_screen_texture("output_texture", (width, height), TextureFormat::Bgra8UnormSrgb, false);
    asset_manager.replace_screen_texture("albedo_texture", (width, height), TextureFormat::Bgra8UnormSrgb, false);
    asset_manager.replace_screen_texture("normal_texture", (width, height), TextureFormat::Bgra8UnormSrgb, false);
    asset_manager.replace_screen_texture("depth_texture", (width, height), TextureFormat::Depth32Float, false);

    // Update GBuffer material with latest texture views.
    let gbuffer_material = asset_manager.get_material_by_name("gbuffer_mat").unwrap();
    let albedo_tex = asset_manager.get_texture_by_name("albedo_texture").unwrap();
    let normal_tex = asset_manager.get_texture_by_name("normal_texture").unwrap();
    let depth_tex = asset_manager.get_texture_by_name("depth_texture").unwrap();
    let output_tex = asset_manager.get_texture_by_name("output_texture").unwrap();

    let albedo_view = albedo_tex.view.clone();
    let normal_view = normal_tex.view.clone();
    let depth_view = depth_tex.view.clone();
    let output_view = output_tex.view.clone();

    let shadow_atlas_view = shadow_atlas.texture.view.clone();

    gbuffer_material.set_texture("g_albedo", albedo_view.clone());
    gbuffer_material.set_texture("g_normal", normal_view.clone());
    gbuffer_material.set_texture("g_depth", depth_view.clone());
    gbuffer_material.set_texture("shadow_map", shadow_atlas_view.clone());
    gbuffer_material.set_uniform("shadow_data", global_component.directional_shadow_buffer.clone().unwrap());

    // Update the post-processing material with the latest output texture view.
    let post_processing_material = asset_manager.get_material_by_name("invert_mat").unwrap();
    post_processing_material.set_texture("u_texture", output_view.clone());
}

/// Updates global state, camera, and global uniform buffer.
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

/// Updates light data from light components into the global light storage.
pub fn light_update_system(
    mut global_component: UniqueViewMut<GlobalComponent>,
    mut lights: ViewMut<LightComponent>,
    camera: UniqueView<CameraComponent>,
) {
    let mut light_data: Vec<Light> = Vec::new();
    (&mut lights).iter().for_each(|light| {
        light_data.push(light.clone());
    });
    global_component.point_light_storage.set_lights(light_data);

    if global_component.point_light_storage.needs_rebuild {
        info!("Rebuilding light storage buffer");
        global_component.reconstruct_bind_group();
        global_component.point_light_storage.needs_rebuild = false;
    }

    global_component.point_light_storage.update_buffer();

    // Calculate the directional light view projection matrix.
    let light = &mut global_component.directional_light.unwrap();
    let camera = &camera.camera;
    let camera_pos = camera.position();
    let light_dir = -light.rotation.normalize();
    let scene_center = camera_pos;
    let light_distance = -50.0;
    let light_pos = scene_center - light_dir * light_distance;
    let light_view = glam::Mat4::look_at_rh(light_pos, scene_center, glam::Vec3::Y);
    let proj = glam::Mat4::orthographic_rh(-30.0, 30.0, -30.0, 30.0, 0.1, 100.0);
    let light_view_proj = proj * light_view;
    global_component.directional_light_view_proj = light_view_proj;
    global_component.directional_light_buffer.as_ref().unwrap().update(light);
}

/// Helper function to create a RenderPassColorAttachment with a clear color.
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

/// The main render graph system integrated into the ECS. This system can be registered with your ECS world.
pub fn render_graph_system(
    mut graphics: RenderGraphicsViewMut,
    asset_manager: UniqueView<AssetManager>,
    mesh_comps: View<MeshComponent>,
    mat_comps: View<MaterialComponent>,
    transform_comps: View<TransformComponent>,
    shadow_cast_component: View<ShadowCastComponent>,
) {
    // For demonstration, obtain our output and shadow atlas texture views from the asset manager.
    let shadow_atlas_view = graphics.shadow_atlas.texture.view.clone();

    // Prepare the render graph context.
    let mut context = RenderGraphContext {
        encoder: &mut graphics.encoder,
        asset_manager: &*asset_manager,
        global_component: &*graphics.global_component,
        camera_component: &*graphics.camera_component,
        output_view: graphics.view.clone(),
        shadow_atlas_view,
    };

    // Build the render graph.
    let mut render_graph = RenderGraph::new();

    // --- Depth Pass Node ---
    render_graph.add_node(RenderGraphNode {
        name: "depth_pass".into(),
        dependencies: vec![],
        execute: Box::new(|ctx: &mut RenderGraphContext| {
            // Retrieve necessary resources.
            let albedo_tex = ctx.asset_manager.get_texture_by_name("albedo_texture").unwrap();
            let normal_tex = ctx.asset_manager.get_texture_by_name("normal_texture").unwrap();
            let depth_tex = ctx.asset_manager.get_texture_by_name("depth_texture").unwrap();
            let albedo_view = albedo_tex.view.clone();
            let normal_view = normal_tex.view.clone();
            let depth_view = depth_tex.view.clone();

            // Begin the depth-enabled render pass.
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

                // Iterate over mesh entities and issue draw calls.
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

    // --- Shadow Pass Node ---
    render_graph.add_node(RenderGraphNode {
        name: "shadow_pass".into(),
        dependencies: vec!["depth_pass".into()],
        execute: Box::new(|ctx: &mut RenderGraphContext| {
            // Use asset manager to get the shadow shader and material.
            let shadow_material = ctx.asset_manager.get_material_by_name("shadow_mat").unwrap();
            // Create a view for the shadow atlas.
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

                // Set the pipeline and bind group.
                pass.set_pipeline(&shadow_material.get_pipeline());
                pass.set_bind_group(0, &*ctx.global_component.global_bind_group, &[]);
                // Set the viewport.
                let shadow_map_data = ctx.global_component.directional_light_shadow_map.as_ref().unwrap().read().unwrap();
                let rect = shadow_map_data.rect;
                pass.set_viewport(rect.x as f32, rect.y as f32, rect.width as f32, rect.height as f32, 0.0, 1.0);
                // Set the light view-projection matrix as a push constant.
                pass.set_push_constants(wgpu::ShaderStages::VERTEX_FRAGMENT, size_of::<glam::Mat4>() as u32, &bytemuck::cast_slice(&[ctx.global_component.directional_light_view_proj]));
                // Iterate over mesh entities and issue draw calls.
                (&mesh_comps, &transform_comps, &shadow_cast_component).iter().for_each(|(mesh_comp, transform_comp, shadow_caster)| {
                    if shadow_caster.shadow_cast {
                        let transform = [transform_comp.transform.matrix];
                        let transform_push_const = bytemuck::cast_slice(&transform);
                        pass.set_push_constants(wgpu::ShaderStages::VERTEX_FRAGMENT, 0, &transform_push_const);
                        mesh_comp.mesh.draw(&mut pass);
                    }
                });
            }
        }),
    });

    // --- GBuffer Composite Pass Node ---
    render_graph.add_node(RenderGraphNode {
        name: "gbuffer_pass".into(),
        dependencies: vec!["shadow_pass".into()],
        execute: Box::new(|ctx: &mut RenderGraphContext| {
            let output = ctx.asset_manager.get_texture_by_name("output_texture").unwrap();


            let gbuffer_material = ctx.asset_manager.get_material_by_name("gbuffer_mat").unwrap();
            let pipeline = gbuffer_material.get_pipeline();
            // Get the shadow map view so we can bind it to the material.
            let shadow_atlas_view = ctx.shadow_atlas_view.clone();
            // Update the GBuffer material with the latest texture views.
            gbuffer_material.set_texture("shadow_map", shadow_atlas_view);
            // Update the shadow data uniform buffer.
            if let Some(mut shadow_data) = ctx.global_component.directional_shadow_data {
                shadow_data.light_view_proj = ctx.global_component.directional_light_view_proj;
                ctx.global_component.directional_shadow_buffer.as_ref().unwrap().update(&shadow_data);
            }
            // Bind the shadow data uniform buffer to the material.
            gbuffer_material.set_uniform("shadow_data", ctx.global_component.directional_shadow_buffer.clone().unwrap());

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
                // Bind the gbuffer material resources and draw a full-screen triangle.
                gbuffer_material.bind(&mut pass);
                pass.draw(0..3, 0..1);
            }
        }),
    });

    // --- Post-Processing Pass Node ---
    render_graph.add_node(RenderGraphNode {
        name: "post_process".into(),
        dependencies: vec!["gbuffer_pass".into()],
        execute: Box::new(|ctx: &mut RenderGraphContext| {
            let post_material = ctx.asset_manager.get_material_by_name("invert_mat").unwrap();
            // Bind the intermediate render target as the input texture.
            let texture = ctx.asset_manager.get_texture_by_name("output_texture").unwrap();
            post_material.set_texture("u_texture", texture.view.clone());
            let pipeline = post_material.get_pipeline();
            // Assume we have an intermediate render target that holds our post-process input.
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

    // Build the dependency graph from the added nodes.
    render_graph.build_dependency_graph();

    // Execute the render graph.
    render_graph.execute(&mut context);
}
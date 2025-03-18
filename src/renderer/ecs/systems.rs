// src/renderer/ecs/systems.rs

use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

use glam::{vec3};
use rand::random;
use shipyard::{
    EntitiesViewMut, IntoIter, UniqueView, UniqueViewMut, View, ViewMut,
};
use wgpu::{
    Color, LoadOp, Operations, RenderPassColorAttachment, RenderPassDepthStencilAttachment,
    RenderPassDescriptor, StoreOp, TextureFormat,
};
use winit::event::KeyEvent;

use crate::renderer::asset_manager::AssetManager;
use crate::renderer::auto_mipmapper::AutoMipmapper;
use crate::renderer::ecs::camera_component::CameraComponent;
use crate::renderer::ecs::global_component::GlobalComponent;
use crate::renderer::ecs::light_manager::LightManager;
use crate::renderer::ecs::render_graphics_view::RenderGraphicsViewMut;
use crate::renderer::ecs::components::*;
use crate::renderer::ecs::instancing_component::InstancingComponent;
use crate::renderer::ecs::light_update_view::LightUpdateViewMut;
use crate::renderer::render_batcher::{RenderBatcher, RenderCommand};
use crate::renderer::render_graph::{RenderGraph, RenderGraphContext, RenderGraphNode};
use crate::renderer::shadow_atlas::ShadowAtlas;
use crate::renderer::State;
use crate::renderer::types::instance_data::InstanceData;
use crate::renderer::types::light::Light;
use crate::renderer::types::light_type::LightType;
use crate::renderer::types::material::Material;
use crate::renderer::types::sampler::SamplerParameters;
use crate::renderer::types::transform::Transform;

/// Loads assets (meshes, textures, shaders, materials, and screen textures) into the asset manager.
/// Loads assets (meshes, textures, shaders, materials, screen textures) into the asset manager.
pub fn load_assets(
    state: UniqueViewMut<State>,
    mut asset_manager: UniqueViewMut<AssetManager>,
    shadow_atlas: UniqueView<ShadowAtlas>,
) {
    // Meshes & textures
    let capsule_mesh = asset_manager.get_or_create_mesh("assets/capsule.obj");
    let capsule_tex = asset_manager.get_or_create_texture("capsule_tex", "assets/capsule0.jpg", true);
    let cube_mesh = asset_manager.get_or_create_mesh("assets/cube.obj");
    let white_tex = asset_manager.get_or_create_texture("white_tex", "assets/solid_white.png", false);

    // Screen textures (GBuffer)
    let screen_size = state.get_screen_size();
    let albedo_tex = asset_manager.get_or_create_screen_texture("albedo_texture", screen_size, TextureFormat::Rgba16Float);
    let normal_tex = asset_manager.get_or_create_screen_texture("normal_texture", screen_size, TextureFormat::Rg16Snorm);
    let depth_tex = asset_manager.get_or_create_screen_texture("depth_texture", screen_size, TextureFormat::Depth16Unorm);
    let output_tex = asset_manager.get_or_create_screen_texture("output_texture", screen_size, TextureFormat::Rgba16Float);

    // Load shaders
    asset_manager.get_or_create_shader("main", "assets/shaders/shader.wgsl");
    asset_manager.get_or_create_shader("main_instanced", "assets/shaders/shader_instanced.wgsl");
    asset_manager.get_or_create_shader("shadow", "assets/shaders/shadow.wgsl");
    asset_manager.get_or_create_shader("gbuffer", "assets/shaders/deferred.wgsl");
    asset_manager.get_or_create_shader("invert", "assets/shaders/post_process.wgsl");

    // Sampler
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

    // Materials
    create_material_with(
        &mut asset_manager,
        "capsule_mat",
        "main_instanced",
        true,
        true,
        |material| {
            material.set_cull_mode(None);
            material.set_depth(true, wgpu::TextureFormat::Depth16Unorm);
            material.set_transparent(false);
            material.set_texture("color_texture", capsule_tex.view.clone());
            material.set_sampler("color_sampler", sampler.clone());
        },
    );

    create_material_with(
        &mut asset_manager,
        "box_mat",
        "main_instanced",
        true,
        true,
        |material| {
            material.set_cull_mode(None);
            material.set_depth(true, wgpu::TextureFormat::Depth16Unorm);
            material.set_transparent(false);
            material.set_texture("color_texture", white_tex.view.clone());
            material.set_sampler("color_sampler", sampler.clone());
        },
    );

    create_material_with(
        &mut asset_manager,
        "gbuffer_mat",
        "gbuffer",
        false,
        false,
        |material| {
            material.set_cull_mode(Some(wgpu::Face::Back));
            material.set_depth(false, wgpu::TextureFormat::Depth16Unorm);
            material.set_transparent(false);
            material.set_texture("g_albedo", albedo_tex.view.clone());
            material.set_texture("g_normal", normal_tex.view.clone());
            material.set_texture("g_depth", depth_tex.view.clone());
            material.set_sampler("g_sampler", sampler.clone());
            material.set_sampler("shadow_sampler", shadow_atlas.shadow_sampler.clone());
        },
    );

    create_material_with(
        &mut asset_manager,
        "invert_mat",
        "invert",
        false,
        false,
        |material| {
            material.set_cull_mode(Some(wgpu::Face::Front));
            material.set_depth(false, wgpu::TextureFormat::Depth16Unorm);
            material.set_transparent(false);
            material.set_sampler("u_sampler", sampler.clone());
        },
    );

    create_material_with(
        &mut asset_manager,
        "shadow_mat",
        "shadow",
        false,
        false,
        |material| {
            material.set_cull_mode(Some(wgpu::Face::Front));
            material.set_depth(true, wgpu::TextureFormat::Depth24Plus);
            material.set_transparent(false);
        },
    );
}

/// Helper to create a material with a configuration callback.
fn create_material_with<F>(
    asset_manager: &mut AssetManager,
    name: &str,
    shader_name: &str,
    instanced: bool,
    cast_shadows: bool,
    config: F,
) -> Arc<Material>
where
    F: FnOnce(Arc<Material>),
{
    let material = asset_manager.get_or_create_material(name, shader_name, instanced, cast_shadows);
    config(material.clone());
    material
}

/// Spawns some default entities into the ECS (ground, box, capsules, lights).
pub fn add_entities(
    mut entities: EntitiesViewMut,
    asset_manager: UniqueView<AssetManager>,
    mut shadow_atlas: UniqueViewMut<ShadowAtlas>,
    mut meshes: ViewMut<MeshComponent>,
    mut materials: ViewMut<MaterialComponent>,
    mut transforms: ViewMut<TransformComponent>,
    mut shadow_cast: ViewMut<ShadowCastComponent>,
    mut lights: ViewMut<LightComponent>,
    mut light_manager: UniqueViewMut<LightManager>,
) {
    // Ground
    entities.add_entity(
        (&mut meshes, &mut materials, &mut transforms, &mut shadow_cast),
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

    // Box
    entities.add_entity(
        (&mut meshes, &mut materials, &mut transforms, &mut shadow_cast),
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

    // Capsules
    entities.bulk_add_entity(
        (&mut meshes, &mut materials, &mut transforms, &mut shadow_cast),
        (0..250).map(|_| {
            let mesh_comp = MeshComponent {
                mesh: asset_manager.get_mesh_by_name("assets/capsule.obj").unwrap(),
            };
            let mat_comp = MaterialComponent {
                material: asset_manager.get_material_by_name("capsule_mat").unwrap(),
            };
            let mut transform = Transform::new();
            let x = random::<f32>() * 30.0 - 15.0;
            let z = random::<f32>() * 30.0 - 15.0;
            transform.translate(vec3(x * 2.0 - 10.0, 0.0, z * 2.0 - 10.0));
            transform.rotate(glam::Quat::from_euler(
                glam::EulerRot::YXZ,
                random::<f32>() * 360.0,
                random::<f32>() * 360.0,
                random::<f32>() * 360.0,
            ));
            transform.scale(vec3(0.5, 0.5, 0.5));
            (mesh_comp, mat_comp, TransformComponent { transform }, ShadowCastComponent { shadow_cast: true })
        }),
    );

    // Directional light
    let dir_light = light_manager.create_directional_light(&mut shadow_atlas);
    let dir_comp = LightComponent::new(dir_light, LightType::Directional);
    entities.add_entity(
        (&mut lights, &mut transforms),
        (dir_comp, TransformComponent { transform: Transform::new() }),
    );

    // Point lights
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
        let point_comp = LightComponent::new(point_light, LightType::Point);
        entities.add_entity(
            (&mut lights, &mut transforms),
            (point_comp, TransformComponent { transform: Transform::new() }),
        );
    }

    // Spotlight
    let spot_position = vec3(-8.0, 2.0, 0.0);
    let spot_direction = vec3(0.5, -0.2, 0.0);
    let spot_color = vec3(1.0, 1.0, 0.8);
    let spot_intensity = 2.0;
    let spot_range = 40.0;
    let spot_angle = std::f32::consts::FRAC_PI_6;
    let spot_light = light_manager.create_spot_light(
        spot_position,
        spot_direction,
        spot_color,
        spot_intensity,
        spot_range,
        spot_angle,
        &mut shadow_atlas,
    );
    let spot_comp = LightComponent::new(spot_light, LightType::Spot);
    let spot_transform = TransformComponent {
        transform: {
            let mut t = Transform::new();
            t.translate(spot_position);
            t
        },
    };
    entities.add_entity((&mut lights, &mut transforms), (spot_comp, spot_transform));
}

/// Generates mipmaps for any texture that supports it.
pub fn mipmap_system(
    mut auto_mipmapper: UniqueViewMut<AutoMipmapper>,
    asset_manager: UniqueView<AssetManager>,
) {
    let mut encoder = asset_manager.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Texture Mipmapping Encoder"),
    });
    let textures = asset_manager.get_all_textures();
    for tex in textures.iter() {
        if tex.mipmappable {
            auto_mipmapper.generate_mipmaps(&mut encoder, &[tex.clone()], &[tex.mip_level_count]);
        }
    }
    asset_manager.queue.submit(std::iter::once(encoder.finish()));
}

/// For keyboard input (e.g., WASD).
pub fn handle_keyboard_input(
    key_event: KeyEvent,
    mut camera_component: UniqueViewMut<CameraComponent>,
) {
    camera_component.camera.process_keyboard(key_event);
}

/// For mouse input (pitch/yaw).
pub fn handle_mouse_input(
    delta: (f64, f64),
    mut camera_component: UniqueViewMut<CameraComponent>,
) {
    camera_component.camera.process_mouse(delta.0 as f32, delta.1 as f32);
}

/// Resizes relevant resources on window resize.
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
    asset_manager.replace_screen_texture("normal_texture", (width, height), TextureFormat::Rg16Snorm, false);
    asset_manager.replace_screen_texture("depth_texture", (width, height), TextureFormat::Depth16Unorm, false);

    let gbuffer_material = asset_manager.get_material_by_name("gbuffer_mat").unwrap();
    let albedo = asset_manager.get_texture_by_name("albedo_texture").unwrap();
    let normal = asset_manager.get_texture_by_name("normal_texture").unwrap();
    let depth = asset_manager.get_texture_by_name("depth_texture").unwrap();
    let output = asset_manager.get_texture_by_name("output_texture").unwrap();

    gbuffer_material.set_texture("g_albedo", albedo.view.clone());
    gbuffer_material.set_texture("g_normal", normal.view.clone());
    gbuffer_material.set_texture("g_depth", depth.view.clone());
    gbuffer_material.set_texture("shadow_map", shadow_atlas.texture.view.clone());

    let post_mat = asset_manager.get_material_by_name("invert_mat").unwrap();
    post_mat.set_texture("u_texture", output.view.clone());
}

/// Updates global time, camera, and uniform buffers, plus rebuilds the render batch.
pub fn update_system(
    mut state: UniqueViewMut<State>,
    mut global_comp: UniqueViewMut<GlobalComponent>,
    mut camera_comp: UniqueViewMut<CameraComponent>,
    mut render_batcher: UniqueViewMut<RenderBatcher>,
    meshes: View<MeshComponent>,
    materials: View<MaterialComponent>,
    transforms: View<TransformComponent>,
) {
    state.update();
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f32();

    global_comp.global_data.update(time);
    camera_comp.camera.update(state.delta_time);
    global_comp.global_data.update_from_camera(camera_comp.camera.deref());
    global_comp.global_uniform_buffer.update(&global_comp.global_data);

    // Rebuild the render batch
    render_batcher.clear();
    for (mesh, material, transform) in (&meshes, &materials, &transforms).iter() {
        render_batcher.add(
            mesh.mesh.clone(),
            material.material.clone(),
            transform.transform.clone(),
        );
    }
}

/// Updates lights in bulk via the LightManager.
pub fn update_lighting(
    mut light_update: LightUpdateViewMut,
    mut lights: ViewMut<LightComponent>,
) {
    let mut light_data: Vec<Light> = lights.iter().map(|lc| lc.light.clone()).collect();
    light_update.light_manager.update_lights(&mut light_data, &light_update.camera_component.camera);

    for (mut lc, updated) in (&mut lights).iter().zip(light_data.into_iter()) {
        lc.light = updated;
    }
}

/// Builds a contiguous array of InstanceData from the batcher, then updates the InstancingComponent.
pub fn update_instancing(
    batcher: UniqueView<RenderBatcher>,
    mut instancing: UniqueViewMut<InstancingComponent>,
) {
    let mut cpu_data = Vec::new();
    let mut offsets = HashMap::new();
    let mut total = 0;

    for command in &batcher.commands {
        if let RenderCommand::Instanced { mesh, material, transforms } = command {
            let key = (Arc::as_ptr(&mesh) as u64, Arc::as_ptr(&material) as u64);
            offsets.insert(key, (total, transforms.len() as u32));
            for t in transforms {
                cpu_data.push(InstanceData {
                    model: t.matrix,
                    normal_matrix: t.normal_matrix,
                });
                total += 1;
            }
        }
    }
    instancing.update(&cpu_data, offsets);
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
                pass.set_bind_group(2, &graphics.instancing_component.instancing_bind_group, &[]);
                for command in &graphics.render_batcher.commands {
                    match command {
                        RenderCommand::Instanced { mesh, material, transforms: _ } => {
                            if !material.get_depth().0{
                                continue;
                            }
                            pass.set_pipeline(&material.get_pipeline());
                            // For single draws, use push constants.
                            pass.set_push_constants(
                                wgpu::ShaderStages::VERTEX_FRAGMENT,
                                0,
                                bytemuck::cast_slice(&[glam::Mat4::IDENTITY]),
                            );
                            // Get the group mapping for this (mesh, material) pair.
                            let key = (Arc::as_ptr(&mesh) as u64, Arc::as_ptr(&material) as u64);
                            if let Some(&(offset, count)) = graphics.instancing_component.group_offsets.get(&key) {
                                // Bind the instancing bind group to group 2.
                                material.bind(&mut pass);
                                // Draw instanced with the instance count from the group.
                                mesh.draw_instanced(&mut pass, offset, count);
                            }
                        }
                        RenderCommand::Single { mesh, material, transform } => {
                            if !material.get_depth().0{
                                continue;
                            }
                            pass.set_pipeline(&material.get_pipeline());
                            // For single draws, use push constants.
                            pass.set_push_constants(
                                wgpu::ShaderStages::VERTEX_FRAGMENT,
                                0,
                                bytemuck::cast_slice(&[*transform]),
                            );
                            material.bind(&mut pass);
                            mesh.draw(&mut pass);
                        }
                    }
                }
            }
        }),
    });

    // Shadow Pass Node
    render_graph.add_node(RenderGraphNode {
        name: "shadow_pass".into(),
        dependencies: vec![],
        execute: Box::new(|ctx: &mut RenderGraphContext| {
            let shadow_material = ctx.asset_manager.get_material_by_name("shadow_mat").unwrap();
            let shadow_view = ctx.shadow_atlas_view.clone();

            if let Some(encoder) = ctx.encoder {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Shadow Pass"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &shadow_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                pass.set_pipeline(&shadow_material.get_pipeline());
                // Bind group 0 for global data, group 2 for instancing (if needed).
                pass.set_bind_group(0, &*ctx.global_component.global_bind_group, &[]);
                pass.set_bind_group(1, &graphics.instancing_component.instancing_bind_group, &[]);

                // Retrieve the shadow data from the LightManager
                let shadow_data = graphics.light_manager.shadow_data_storage.get_all_shadow_data();
                let light_data = graphics.light_manager.light_storage.get_all_lights();

                for light in &light_data {
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
                        pass.set_scissor_rect(rect.x, rect.y, rect.width, rect.height);

                        // For each command in the batcher:
                        for command in &graphics.render_batcher.commands {
                            match command {
                                RenderCommand::Instanced { mesh, material, transforms: _ } => {
                                    if !material.is_shadow_caster() {
                                        // Skip if these instances don't cast shadows
                                        continue;
                                    }
                                    // We'll assume your "shadow_mat" doesn't rely on the mesh's material
                                    // pipeline states, or that it's the same pipeline you already set.
                                    // For instanced draws, we do the same approach as in the depth pass:
                                    pass.set_pipeline(&shadow_material.get_pipeline());
                                    // We also set push constants for each instance, but we only have
                                    // one shadow matrix. The typical approach is to store (model + shadow_matrix)
                                    // in push constants. However, to replicate the same approach as your
                                    // depth pass, you'd set push constants to identity, and let the
                                    // vertex shader fetch the instance data from the SSBO.

                                    // But we still need to combine each instance model with smc.shadow_data.light_view_proj.
                                    // Typically you'd do that in the shader. If your shader expects push constants,
                                    // you'd need to do a loop. Instead, many do a per-instance push of model
                                    // plus the light_view_proj. That might require a separate instanced pipeline.

                                    // For a quick approach: set the push constants to identity for the model,
                                    // and let the vertex shader multiply the instance transform by the shadow matrix
                                    // from a uniform or push constants. If your shadow shader requires both, you'd do:
                                    pass.set_push_constants(
                                        wgpu::ShaderStages::VERTEX_FRAGMENT,
                                        0,
                                        bytemuck::cast_slice(&[glam::Mat4::IDENTITY, smc.shadow_data.light_view_proj]),
                                    );

                                    // Retrieve the offset/count from the instancing component
                                    let key = (Arc::as_ptr(&mesh) as u64, Arc::as_ptr(&material) as u64);
                                    // Or if you do store a "shadow_mat" in the batch, you'd match that.
                                    // If your real code uses the same key as the depth pass (mesh+material), do that:
                                    // let key = (Arc::as_ptr(mesh) as u64, Arc::as_ptr(material) as u64);

                                    if let Some(&(inst_offset, inst_count)) = graphics.instancing_component.group_offsets.get(&key) {
                                        mesh.draw_instanced(&mut pass, inst_offset, inst_count);
                                    }
                                }
                                RenderCommand::Single { mesh, material, transform } => {
                                    if !material.is_shadow_caster() {
                                        continue;
                                    }
                                    pass.set_pipeline(&shadow_material.get_pipeline());
                                    // Combine model + smc.shadow_data.light_view_proj in push constants
                                    // so the vertex shader can do: shadow_matrix * model * position.
                                    let mut push_data = Vec::with_capacity(64);
                                    push_data.extend_from_slice(bytemuck::bytes_of(&transform.matrix));
                                    push_data.extend_from_slice(bytemuck::bytes_of(&smc.shadow_data.light_view_proj));
                                    pass.set_push_constants(wgpu::ShaderStages::VERTEX_FRAGMENT, 0, &push_data);
                                    mesh.draw(&mut pass);
                                }
                            }
                        }
                    }
                }
            }
        }),
    });


    // GBuffer Composite Pass Node.
    render_graph.add_node(RenderGraphNode {
        name: "gbuffer_pass".into(),
        dependencies: vec!["depth_pass".into(), "shadow_pass".into()],
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
                // Figure out how many lights we have.
                let light_count = graphics.light_manager.light_storage.get_all_lights().len();
                // Set the light count.
                pass.set_push_constants(
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                    0,
                    bytemuck::cast_slice(&[light_count as u32]),
                );
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

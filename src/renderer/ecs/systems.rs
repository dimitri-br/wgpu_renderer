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
use crate::renderer::shadow_atlas::ShadowAtlas;
use crate::renderer::State;
use crate::renderer::types::camera::Camera;
use crate::renderer::types::light::Light;
use crate::renderer::types::light_type::LightType;
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

    // Generate mipmaps for the texture.
    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Texture Mipmapping Encoder"),
        });
    auto_mipmapper.generate_mipmaps(&mut encoder, &[texture.clone()], &[texture.mip_level_count]);
    state.queue.submit(std::iter::once(encoder.finish()));

    // Create main shader.
    let shader = asset_manager.get_or_create_shader("main", "assets/shaders/shader.wgsl");

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

    // Create a material using the shader.
    let material = asset_manager.get_or_create_material("capsule_mat", "main");
    material.set_cull_mode(Some(wgpu::Face::Back));
    material.set_depth(true);
    material.set_transparent(false);
    material.set_texture("color_texture", texture.view.clone());
    material.set_sampler("color_sampler", sampler.clone());

    let box_material = asset_manager.get_or_create_material("box_mat", "main");
    box_material.set_cull_mode(Some(wgpu::Face::Back));
    box_material.set_depth(true);
    box_material.set_transparent(false);
    box_material.set_texture("color_texture", white_texture.view.clone());
    box_material.set_sampler("color_sampler", sampler.clone());

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

    // Create a deferred (GBuffer) shader and material.
    let gbuffer_shader = asset_manager.get_or_create_shader("gbuffer", "assets/shaders/deferred.wgsl");
    let gbuffer_material = asset_manager.get_or_create_material("gbuffer_mat", "gbuffer");
    gbuffer_material.set_cull_mode(None);
    gbuffer_material.set_depth(false);
    gbuffer_material.set_transparent(false);
    gbuffer_material.set_texture("g_albedo", albedo_texture.view.clone());
    gbuffer_material.set_texture("g_normal", normal_texture.view.clone());
    gbuffer_material.set_texture("g_depth", depth_texture.view.clone());
    gbuffer_material.set_sampler("g_sampler", sampler.clone());
    gbuffer_material.set_sampler("shadow_sampler", shadow_atlas.shadow_sampler.clone());

    // Create a post-processing shader and material.
    let post_processing_shader = asset_manager.get_or_create_shader("invert", "assets/shaders/post_process.wgsl");
    let post_processing_material = asset_manager.get_or_create_material("invert_mat", "invert");
    post_processing_material.set_cull_mode(None);
    post_processing_material.set_depth(false);
    post_processing_material.set_transparent(false);
    post_processing_material.set_sampler("u_sampler", sampler.clone());

    // Get the shadow shader
    let shadow_shader = asset_manager.get_or_create_shader("shadow", "assets/shaders/shadow.wgsl");
    // Create the shadow material
    let shadow_material = asset_manager.get_or_create_material("shadow_mat", "shadow");
    shadow_material.set_cull_mode(Some(wgpu::Face::Front));
    shadow_material.set_depth(true);
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
) {
    state.trigger_resize(width, height);
    camera_component.camera.resize(width as f32, height as f32);
    global_component.global_data.update_screen_size(width as f32, height as f32);

    // Replace screen textures in the asset manager.
    asset_manager.replace_screen_texture("output_texture", (width, height), TextureFormat::Bgra8UnormSrgb, false);
    asset_manager.replace_screen_texture("albedo_texture", (width, height), TextureFormat::Bgra8UnormSrgb, false);
    asset_manager.replace_screen_texture("normal_texture", (width, height), TextureFormat::Bgra8UnormSrgb, false);
    asset_manager.replace_screen_texture("depth_texture", (width, height), TextureFormat::Depth32Float, false);
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
}

/// Main render system: performs multiple passes (depth-enabled, GBuffer composite, post-processing).
pub fn render_system(
    mut graphics: RenderGraphicsViewMut,
    asset_manager: UniqueView<AssetManager>,
    mesh_comps: View<MeshComponent>,
    mat_comps: View<MaterialComponent>,
    transform_comps: View<TransformComponent>,
    shadow_cast_component: View<ShadowCastComponent>,
) {
    // Get the shadow atlas texture view.
    let shadow_atlas_view = graphics.shadow_atlas.texture.view.clone();
    // Get the shadow shader and material.
    let shadow_shader = asset_manager.get_shader_by_name("shadow").unwrap();
    let shadow_material = asset_manager.get_material_by_name("shadow_mat").unwrap();


    // Retrieve textures and material pipelines.
    let albedo_tex = asset_manager.get_texture_by_name("albedo_texture").unwrap();
    let normal_tex = asset_manager.get_texture_by_name("normal_texture").unwrap();
    let depth_tex = asset_manager.get_texture_by_name("depth_texture").unwrap();
    let output_tex = asset_manager.get_texture_by_name("output_texture").unwrap();

    let albedo_view = albedo_tex.view.clone();
    let normal_view = normal_tex.view.clone();
    let depth_view = depth_tex.view.clone();
    let output_view = output_tex.view.clone();

    let gbuffer_material = asset_manager.get_material_by_name("gbuffer_mat").unwrap();
    let gbuffer_pipeline = gbuffer_material.get_pipeline();
    // Add the shadow atlas view and sampler to the material
    let global_component = &mut graphics.global_component;
    // Update GBuffer material with latest texture views.
    gbuffer_material.set_texture("g_albedo", albedo_view.clone());
    gbuffer_material.set_texture("g_normal", normal_view.clone());
    gbuffer_material.set_texture("g_depth", depth_view.clone());
    gbuffer_material.set_texture("shadow_map", shadow_atlas_view.clone());
    gbuffer_material.set_uniform("shadow_data", global_component.directional_shadow_buffer.clone().unwrap());

    let post_processing_material = asset_manager.get_material_by_name("invert_mat").unwrap();
    let post_processing_pipeline = post_processing_material.get_pipeline();
    post_processing_material.set_texture("u_texture", output_view.clone());

    // ---- Depth-Enabled Render Pass (GBuffer population) ----
    {
        let mut render_pass = graphics.encoder.as_mut().unwrap().begin_render_pass(&RenderPassDescriptor {
            label: Some("Depth Render Pass"),
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

        // Bind global resources (group 0)
        render_pass.set_bind_group(0, &*graphics.global_component.global_bind_group, &[]);

        // Render all mesh entities that require depth writing.
        (&mesh_comps, &mat_comps, &transform_comps).iter().for_each(|(mesh_comp, mat_comp, transform_comp)| {
            if !mat_comp.material.get_depth() {
                return;
            }

            render_pass.set_pipeline(&mat_comp.material.get_pipeline());
            mat_comp.material.bind(&mut render_pass);

            render_pass.set_push_constants(
                wgpu::ShaderStages::VERTEX_FRAGMENT,
                0,
                bytemuck::cast_slice(&[transform_comp.transform]),
            );

            mesh_comp.mesh.draw(&mut render_pass);
        });
    }


    // Assume our directional light's 'rotation' encodes its direction,
    // but the shader uses the negative of that value as the light's direction.
    let light = &mut graphics.global_component.directional_light.unwrap();

    // Compute a normalized light direction (make sure light.rotation is a proper direction).
    let light_dir = -light.rotation.normalize(); // Negate if the shader uses -direction

    // Choose a scene center (this might be a fixed point or the center of your scene)
    let scene_center = glam::Vec3::ZERO;

    // Pick a distance such that the orthographic bounds cover your scene.
    let light_distance = -50.0;

    // Compute the light's position.
    let light_pos = scene_center - light_dir * light_distance;

    // Now compute a stable light view matrix.
    let light_view = glam::Mat4::look_at_rh(
        light_pos,       // Eye position: fixed relative to scene center.
        scene_center,    // Look at the center of the scene.
        glam::Vec3::Y,   // Up vector.
    );

    let proj = glam::Mat4::orthographic_rh(-30.0, 30.0, -30.0, 30.0, 0.1, 100.0);
    let light_view_proj = [proj * light_view];


    let light_push_const = bytemuck::cast_slice(&light_view_proj);
    if let Some(mut shadow_data) = graphics.global_component.directional_shadow_data {
        shadow_data.light_view_proj = light_view_proj[0];
        graphics.global_component.directional_shadow_buffer.as_ref().unwrap().update(&shadow_data);
    }

    // ---- Shadow Pass ----
    {
        let view = graphics.shadow_atlas.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut render_pass = graphics.encoder.as_mut().unwrap().begin_render_pass(&RenderPassDescriptor {
            label: Some("Shadow Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &view,
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(1.0),
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&shadow_material.get_pipeline());
        render_pass.set_bind_group(0, &*graphics.global_component.global_bind_group, &[]);

        // Set the viewport
        let shadow_map_data = graphics.global_component.directional_light_shadow_map.as_ref().unwrap().read().unwrap();
        let rect = shadow_map_data.rect;

        render_pass.set_viewport(rect.x as f32, rect.y as f32, rect.width as f32, rect.height as f32, 0.0, 1.0);
        render_pass.set_push_constants(wgpu::ShaderStages::VERTEX_FRAGMENT, size_of::<glam::Mat4>() as u32, &light_push_const);


        (&mesh_comps, &transform_comps, &shadow_cast_component).iter().for_each(|(mesh_comp, transform_comp, shadow_caster)| {
            if shadow_caster.shadow_cast {
                let transform = [transform_comp.transform.matrix];
                let transform_push_const = bytemuck::cast_slice(&transform);
                render_pass.set_push_constants(wgpu::ShaderStages::VERTEX_FRAGMENT, 0, &transform_push_const);
                mesh_comp.mesh.draw(&mut render_pass);
            }
        });
    }


    // ---- GBuffer Composite Pass ----
    {
        let mut render_pass = graphics.encoder.as_mut().unwrap().begin_render_pass(&RenderPassDescriptor {
            label: Some("GBuffer Pass"),
            color_attachments: &[Some(generate_color_attachment(&output_view))],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&gbuffer_pipeline);
        render_pass.set_bind_group(0, &*graphics.global_component.global_bind_group, &[]);
        gbuffer_material.bind(&mut render_pass);
        render_pass.draw(0..3, 0..1);
    }

    // ---- Post-Processing Pass ----
    {
        let mut render_pass = graphics.encoder.as_mut().unwrap().begin_render_pass(&RenderPassDescriptor {
            label: Some("Post Processing Pass"),
            color_attachments: &[Some(generate_color_attachment(&graphics.view))],
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

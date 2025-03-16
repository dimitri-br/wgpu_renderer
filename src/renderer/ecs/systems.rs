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

use crate::renderer::ecs::components::*;
use crate::renderer::ecs::camera_component::CameraComponent;
use crate::renderer::ecs::global_component::GlobalComponent;
use crate::renderer::ecs::render_graphics_view::RenderGraphicsViewMut;
use crate::renderer::asset_manager::AssetManager;
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

/// Loads assets (meshes, textures, shaders, materials, and screen textures)
/// into the asset manager.
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
        "albedo_texture", screen_size, wgpu::TextureFormat::Rgba16Float,
    );
    let normal_texture = asset_manager.get_or_create_screen_texture(
        "normal_texture", screen_size, wgpu::TextureFormat::Rgba16Float,
    );
    let depth_texture = asset_manager.get_or_create_screen_texture(
        "depth_texture", screen_size, wgpu::TextureFormat::Depth32Float,
    );
    let output_texture = asset_manager.get_or_create_screen_texture(
        "output_texture", screen_size, wgpu::TextureFormat::Rgba16Float,
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

    // Create materials using a helper.
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

            (
                mesh_component,
                material_component,
                TransformComponent { transform },
                ShadowCastComponent { shadow_cast: true },
            )
        }),
    );

    // Directional light entity.
    let directional_shadow_map = shadow_atlas.allocate_tile(2048, 2048).unwrap();
    let directional_shadow_data = ShadowData::new(
        glam::Mat4::IDENTITY,
        directional_shadow_map.read().unwrap().uv_offset,
        directional_shadow_map.read().unwrap().uv_scale,
        0.0015,
    );

    let mut directional_light = LightComponent::new(
        Light::new(
            vec3(0.0, 5.0, 0.0),
            vec3(0.5, -1.0, 0.0),
            vec3(1.0, 1.0, 1.0),
            0.5,
            100.0,
            0.0,
            LightType::Directional,
        ),
        LightType::Directional,
    );

    let transform = TransformComponent { transform: Transform::new() };
    let smc = ShadowMapComponent::new(directional_shadow_data, directional_shadow_map);
    let idx = global_component.shadow_data_storage.add_shadow_data(smc);
    directional_light.light.set_shadow_data(idx as u32, 1);

    entities.add_entity((&mut lights, &mut transforms), (directional_light, transform));

    // Point light entities.
    entities.bulk_add_entity(
        (&mut lights, &mut transforms),
        (0..4).map(|_| {
            let mut light_transform = Transform::new();
            light_transform.translate(vec3(
                random::<f32>() * 25.0 - 12.5,
                10.0,
                random::<f32>() * 25.0 - 12.5,
            ));
            let color = match random::<u8>() % 3 {
                0 => vec3(1.0, 0.0, 0.0),
                1 => vec3(0.0, 1.0, 0.0),
                _ => vec3(0.0, 0.0, 1.0),
            };
            let intensity = random::<f32>() * 5.0 + 2.5;
            let range = 15.0;
            let rotation = vec3(0.0, 0.0, 0.0);
            let mut point_light = Light::new(
                light_transform.translation(),
                rotation,
                color,
                intensity,
                range,
                0.0,
                LightType::Point,
            );

            let shadow_map_resolution = 1024;
            let proj = glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.1, point_light.range);
            let views = [
                glam::Mat4::look_at_rh(light_transform.translation(), light_transform.translation() + vec3(1.0, 0.0, 0.0), vec3(0.0, -1.0, 0.0)),
                glam::Mat4::look_at_rh(light_transform.translation(), light_transform.translation() + vec3(-1.0, 0.0, 0.0), vec3(0.0, -1.0, 0.0)),
                glam::Mat4::look_at_rh(light_transform.translation(), light_transform.translation() + vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0)),
                glam::Mat4::look_at_rh(light_transform.translation(), light_transform.translation() + vec3(0.0, -1.0, 0.0), vec3(0.0, 0.0, -1.0)),
                glam::Mat4::look_at_rh(light_transform.translation(), light_transform.translation() + vec3(0.0, 0.0, 1.0), vec3(0.0, -1.0, 0.0)),
                glam::Mat4::look_at_rh(light_transform.translation(), light_transform.translation() + vec3(0.0, 0.0, -1.0), vec3(0.0, -1.0, 0.0)),
            ];
            let mut start_index: u32 = 0;
            for i in 0..6 {
                let shadow_map = shadow_atlas.allocate_tile(shadow_map_resolution, shadow_map_resolution).unwrap();
                let shadow_data = ShadowData::new(
                    proj * views[i],
                    shadow_map.read().unwrap().uv_offset,
                    shadow_map.read().unwrap().uv_scale,
                    0.000005,
                );
                let shadow_map_component = ShadowMapComponent::new(shadow_data, shadow_map);
                let idx = global_component.shadow_data_storage.add_shadow_data(shadow_map_component);
                if i == 0 {
                    start_index = idx as u32;
                    println!("Start index: {}", start_index);
                }
            }
            point_light.set_shadow_data(start_index, 6);

            (
                LightComponent::new(point_light, LightType::Point),
                TransformComponent { transform: light_transform },
            )
        }),
    );

    // Spotlight entity.
    {
        let mut spotlight_transform = Transform::new();
        spotlight_transform.translate(vec3(-8.0, 2.0, 0.0));
        let spotlight_color = vec3(1.0, 1.0, 0.8);
        let spotlight_intensity = 2.0;
        let spotlight_range = 40.0;
        let spotlight_angle = std::f32::consts::FRAC_PI_6; // 30° half-angle
        let mut spotlight = Light::new(
            spotlight_transform.translation(),
            vec3(0.5, -0.2, 0.0),
            spotlight_color,
            spotlight_intensity,
            spotlight_range,
            spotlight_angle,
            LightType::Spot,
        );

        let shadow_map_resolution = 512;
        let proj = glam::Mat4::perspective_rh(spotlight_angle * 2.0, 1.0, 0.01, spotlight_range);
        let view = glam::Mat4::look_at_rh(spotlight_transform.translation(), spotlight_transform.translation() + vec3(0.0, -1.0, 0.0), glam::Vec3::Y);
        let tile = shadow_atlas.allocate_tile(shadow_map_resolution, shadow_map_resolution).unwrap();

        let spotlight_shadow_data = ShadowData::new(
            proj * view,
            {
                tile.read().unwrap().uv_offset
            },
            {
                tile.read().unwrap().uv_scale
            },
            0.000005,
        );
        // Allocate one tile for spotlight shadow.
        let shadow_map_component = ShadowMapComponent::new(spotlight_shadow_data, tile);
        let idx = global_component.shadow_data_storage.add_shadow_data(shadow_map_component);
        spotlight.set_shadow_data(idx as u32, 1);

        let spotlight_component = LightComponent::new(spotlight, LightType::Spot);
        let transform_component = TransformComponent { transform: spotlight_transform };
        entities.add_entity((&mut lights, &mut transforms), (spotlight_component, transform_component));
    }
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

    // Replace screen textures.
    asset_manager.replace_screen_texture("output_texture", (width, height), TextureFormat::Rgba16Float, false);
    asset_manager.replace_screen_texture("albedo_texture", (width, height), TextureFormat::Rgba16Float, false);
    asset_manager.replace_screen_texture("normal_texture", (width, height), TextureFormat::Rgba16Float, false);
    asset_manager.replace_screen_texture("depth_texture", (width, height), TextureFormat::Depth32Float, false);

    // Update GBuffer and post-processing materials.
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

/// Updates light data from light components.
pub fn light_update_system(
    mut global_component: UniqueViewMut<GlobalComponent>,
    mut lights: ViewMut<LightComponent>,
    camera: UniqueView<CameraComponent>,
) {
    let mut light_data: Vec<Light> = Vec::new();
    (&mut lights).iter().for_each(|light_comp| {
        let light_type = LightType::from_u32(light_comp.light_type);
        match light_type {
            LightType::Directional => {
                let light = &mut light_comp.light;
                let camera_pos = camera.camera.position();
                let light_dir = -light.rotation.normalize();
                let scene_center = camera_pos;
                let light_distance = -50.0;
                let light_pos = scene_center - light_dir * light_distance;
                let light_view = glam::Mat4::look_at_rh(light_pos, scene_center, glam::Vec3::Y);
                let left = -30.0;
                let right = 30.0;
                let bottom = -30.0;
                let top = 30.0;
                let near = 0.1;
                let far = 100.0;
                let proj = glam::Mat4::orthographic_rh(left, right, bottom, top, near, far);
                let mut light_view_proj = proj * light_view;

                // Shadow snapping.
                let camera_pos_ls = light_view * vec4(camera_pos.x, camera_pos.y, camera_pos.z, 1.0);
                let ortho_width = right - left;
                let shadow_resolution = 2048.0;
                let texel_size = ortho_width / shadow_resolution;
                let snapped_x = (camera_pos_ls.x / texel_size).round() * texel_size;
                let snapped_y = (camera_pos_ls.y / texel_size).round() * texel_size;
                let snap_offset = glam::Mat4::from_translation(vec3(
                    snapped_x - camera_pos_ls.x,
                    snapped_y - camera_pos_ls.y,
                    0.0,
                ));
                light_view_proj = snap_offset * light_view_proj;

                for i in 0..light.shadow_data_count {
                    if let Some(mut shadow_map_data) = global_component
                        .shadow_data_storage
                        .get_shadow_data((light.shadow_data_offset + i) as usize)
                    {
                        shadow_map_data.shadow_data.light_view_proj = light_view_proj;
                        global_component
                            .shadow_data_storage
                            .set_shadow_data((light.shadow_data_offset + i) as usize, shadow_map_data)
                            .unwrap();
                    }
                }
                light.view_proj = light_view_proj;
            },
            LightType::Point => {
                let light = &mut light_comp.light;
                let light_pos = light.position;
                let proj = glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.01, light.range);
                let views = [
                    glam::Mat4::look_at_rh(light_pos, light_pos + vec3(1.0, 0.0, 0.0), vec3(0.0, -1.0, 0.0)),
                    glam::Mat4::look_at_rh(light_pos, light_pos + vec3(-1.0, 0.0, 0.0), vec3(0.0, -1.0, 0.0)),
                    glam::Mat4::look_at_rh(light_pos, light_pos + vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0)),
                    glam::Mat4::look_at_rh(light_pos, light_pos + vec3(0.0, -1.0, 0.0), vec3(0.0, 0.0, -1.0)),
                    glam::Mat4::look_at_rh(light_pos, light_pos + vec3(0.0, 0.0, 1.0), vec3(0.0, -1.0, 0.0)),
                    glam::Mat4::look_at_rh(light_pos, light_pos + vec3(0.0, 0.0, -1.0), vec3(0.0, -1.0, 0.0)),
                ];
                for i in 0..light.shadow_data_count {
                    if let Some(mut shadow_map_data) = global_component.shadow_data_storage.get_shadow_data(light.shadow_data_offset as usize + i as usize) {
                        let view_proj = proj * views[i as usize];
                        shadow_map_data.shadow_data.light_view_proj = view_proj;
                        global_component.shadow_data_storage.set_shadow_data(light.shadow_data_offset as usize + i as usize, shadow_map_data).unwrap();
                    }
                }
                light.view_proj = glam::Mat4::IDENTITY;
            },
            LightType::Spot => {
                // Spotlight shadow mapping.
                let light = &mut light_comp.light;
                let light_pos = light.position;
                let light_dir = light.rotation.normalize();
                let fov = if light.spot_angle > 0.0 { light.spot_angle * 2.0 } else { std::f32::consts::FRAC_PI_4 };
                let aspect = 1.0;
                let near = 0.01;
                let far = light.range;
                let proj = glam::Mat4::perspective_rh(fov, aspect, near, far);
                let view = glam::Mat4::look_at_rh(light_pos, light_pos + light_dir, glam::Vec3::Y);
                let view_proj = proj * view;
                for i in 0..light.shadow_data_count {
                    if let Some(mut shadow_map_data) = global_component
                        .shadow_data_storage
                        .get_shadow_data((light.shadow_data_offset + i) as usize)
                    {
                        shadow_map_data.shadow_data.light_view_proj = view_proj;
                        global_component
                            .shadow_data_storage
                            .set_shadow_data((light.shadow_data_offset + i) as usize, shadow_map_data)
                            .unwrap();
                    }
                }
                light.view_proj = view_proj;
            },
            _ => {}
        }
        light_data.push(light_comp.light.clone());
    });
    global_component.light_storage.set_all_lights(light_data);
    global_component.light_storage.update();
    global_component.shadow_data_storage.update();
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
                let shadow_data = graphics.global_component.shadow_data_storage.get_all_shadow_data();
                let light_data = graphics.global_component.light_storage.get_all_lights();
                for (light_idx, light) in light_data.iter().enumerate() {
                    for i in 0..light.shadow_data_count {
                        let offset = light.shadow_data_offset as usize + i as usize;
                        let smc = &shadow_data[offset];
                        let rect = smc.tile.read().unwrap().rect;
                        pass.set_viewport(rect.x as f32, rect.y as f32, rect.width as f32, rect.height as f32, 0.0, 1.0);
                        pass.set_scissor_rect(rect.x as u32, rect.y as u32, rect.width as u32, rect.height as u32);
                        pass.set_bind_group(0, &*ctx.global_component.global_bind_group, &[]);
                        (&mesh_comps, &transform_comps, &shadow_cast_component).iter().for_each(|(mesh_comp, transform_comp, shadow_caster)| {
                            if shadow_caster.shadow_cast {
                                let model_matrix = [transform_comp.transform.matrix];
                                let mut push_data = Vec::with_capacity(32);
                                push_data.extend_from_slice(bytemuck::bytes_of(&model_matrix));
                                push_data.extend_from_slice(bytemuck::bytes_of(&smc.shadow_data.light_view_proj));
                                pass.set_push_constants(wgpu::ShaderStages::VERTEX_FRAGMENT, 0, &push_data);
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

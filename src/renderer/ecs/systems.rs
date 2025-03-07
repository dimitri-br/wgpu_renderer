use std::ops::Deref;
use glam::vec3;
use log::{error, info};
use rand::random;
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

    let post_processing_shader = asset_manager.get_or_create_shader("invert", "assets/shaders/post_process.wgsl");
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
        transform.rotate(glam::Quat::from_euler(glam::EulerRot::YXZ, random::<f32>() * 360.0, random::<f32>() * 360.0, random::<f32>() * 360.0));
        transform.scale(vec3(0.5, 0.5, 0.5));
        let transform_component = TransformComponent{
            transform
        };
        (mesh_component, material_component, transform_component)
    }));




    entities.bulk_add_entity((&mut lights, &mut transforms), (0..2).map(|n| {
        // Scatter a few lights around
        let mut light_transform = Transform::new();
        light_transform.translate(vec3(
            random::<f32>() * 25.0 - 12.5,
            random::<f32>() * 25.0 - 12.5,
            random::<f32>() * 25.0 - 12.5,
        ));

        let color = vec3(random::<f32>(), random::<f32>(), random::<f32>());
        // Random intensity
        let intensity = random::<f32>() * 10.0 + 10.0;

        let range = 25.0;
        let light = Light::new(light_transform.translation(), color, intensity, range);
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
    asset_manager.replace_screen_texture("output_texture", (width, height), TextureFormat::Bgra8UnormSrgb, false);
    // Resize the GBuffer textures
    asset_manager.replace_screen_texture("albedo_texture", (width, height), TextureFormat::Bgra8UnormSrgb, false);
    asset_manager.replace_screen_texture("position_texture", (width, height), TextureFormat::Bgra8UnormSrgb, false);
    asset_manager.replace_screen_texture("normal_texture", (width, height), TextureFormat::Bgra8UnormSrgb, false);
    asset_manager.replace_screen_texture("depth_texture", (width, height), TextureFormat::Depth32Float, false);
}

pub fn update_system(mut state: UniqueViewMut<State>, mut global_component: UniqueViewMut<GlobalComponent>, mut camera_component: UniqueViewMut<CameraComponent>) {
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

pub fn light_update_system(mut global_component: UniqueViewMut<GlobalComponent>, mut lights: ViewMut<LightComponent>) {
    let mut light_data = Vec::new();
    (&mut lights).iter().for_each(|light| {
        // Move the lights around (0,0,0)
        let mut position = &mut light.light.position;
        // Find the distance from the light to the origin
        let distance = position.length();
        // Move the light in a circle around the origin
        let angle = distance * 0.1 + 0.1;
        position.x = angle.cos() * distance;
        position.z = angle.sin() * distance;


        light_data.push(light.light);
    });
    global_component.light_storage.set_lights(light_data);

    if global_component.light_storage.needs_rebuild {
        info!("Rebuilding light storage buffer");
        global_component.reconstruct_bind_group();
        global_component.light_storage.needs_rebuild = false;
    }

    global_component.light_storage.update();


}

pub fn render_system(
    mut graphics: RenderGraphicsViewMut,
    asset_manager: UniqueView<AssetManager>,
    mesh_comps: View<MeshComponent>,
    mat_comps: View<MaterialComponent>,
    transform_comps: View<TransformComponent>
) {
    // Get textures once.
    let albedo_tex = asset_manager.get_texture_by_name("albedo_texture").unwrap();
    let position_tex = asset_manager.get_texture_by_name("position_texture").unwrap();
    let normal_tex = asset_manager.get_texture_by_name("normal_texture").unwrap();
    let depth_tex = asset_manager.get_texture_by_name("depth_texture").unwrap();
    let output_tex = asset_manager.get_texture_by_name("output_texture").unwrap();

    // Cache texture views and pipelines.
    let albedo_view = &albedo_tex.view;
    let position_view = &position_tex.view;
    let normal_view = &normal_tex.view;
    let depth_view = &depth_tex.view;
    let output_view = &output_tex.view;

    let gbuffer_material = asset_manager.get_material_by_name("gbuffer_mat").unwrap();
    let gbuffer_pipeline = gbuffer_material.get_pipeline();

    // Update textures for GBuffer material.
    gbuffer_material.set_texture("g_albedo", albedo_view.clone());
    gbuffer_material.set_texture("g_position", position_view.clone());
    gbuffer_material.set_texture("g_normal", normal_view.clone());
    gbuffer_material.set_texture("g_depth", depth_view.clone());

    let post_processing_material = asset_manager.get_material_by_name("invert_mat").unwrap();
    let post_processing_pipeline = post_processing_material.get_pipeline();
    post_processing_material.set_texture("u_texture", output_view.clone());

    // Define a clear color constant.

    // ---- Depth-Enabled Render Pass (GBuffer population) ----
    {
        let mut render_pass = graphics.encoder.as_mut().unwrap().begin_render_pass(&RenderPassDescriptor {
            label: Some("Depth Render Pass"),
            color_attachments: &[
                Some(generate_color_attachment(albedo_view)),
                Some(generate_color_attachment(normal_view)),
                Some(generate_color_attachment(position_view)),
            ],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: depth_view,
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

        // Render all mesh entities that write depth.
        (&mesh_comps, &mat_comps, &transform_comps).iter().for_each(|(mesh_comp, mat_comp, transform_comp)| {
            if !mat_comp.material.get_depth() {
                return;
            }

            render_pass.set_pipeline(&mat_comp.material.get_pipeline());
            mat_comp.material.bind(&mut render_pass);

            // Set per-object transform using push constants.
            render_pass.set_push_constants(wgpu::ShaderStages::VERTEX_FRAGMENT, 0, bytemuck::cast_slice(&[transform_comp.transform]));

            mesh_comp.mesh.draw(&mut render_pass);
        });
    }

    // ---- GBuffer Composite Pass ----
    {
        let mut render_pass = graphics.encoder.as_mut().unwrap().begin_render_pass(&RenderPassDescriptor {
            label: Some("GBuffer Pass"),
            color_attachments: &[Some(generate_color_attachment(output_view))],
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
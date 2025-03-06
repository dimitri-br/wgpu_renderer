use gltf::json::extensions::asset;
use log::error;
use shipyard::EntitiesViewMut;
use shipyard::{IntoIter, UniqueView, UniqueViewMut, View, ViewMut, World};
use wgpu::{Color, CommandEncoderDescriptor, LoadOp, Operations, RenderPass, RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor, StoreOp};
use winit::event::KeyEvent;
use crate::renderer::components::*;
use crate::renderer::State;
use crate::renderer::types::material::Material;

use super::asset_manager::AssetManager;
use super::types::sampler::SamplerParameters;
use super::types::transform::Transform;

pub fn load_assets( mut state: UniqueViewMut<State>, mut asset_manager: UniqueViewMut<AssetManager>){
    let mesh = asset_manager.get_or_create_mesh("assets/capsule.obj");
    // load a texture
    let texture = asset_manager.get_or_create_texture("capsule_tex", "assets/capsule0.jpg");

    // create a shader
    let shader = asset_manager.get_or_create_shader("main", "assets/shaders/shader.wgsl");
    // create a material referencing that shader
    let material = asset_manager.get_or_create_material("capsule_mat", "main");
    material.set_cull_mode(Some(wgpu::Face::Back));
    material.set_depth(true);
    material.set_transparent(false);
    material.set_texture("color_texture", texture.view.clone());
    material.set_sampler("color_sampler", SamplerParameters {
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        lod_min_clamp: 0.0,
        lod_max_clamp: 1.0,
        ..Default::default()
    });

    
}
pub fn add_entities(mut entities: EntitiesViewMut, asset_manager: UniqueView<AssetManager>, mut meshes: ViewMut<MeshComponent>, mut materials: ViewMut<MaterialComponent>, mut transforms: ViewMut<TransformComponent>){
    let mesh_component = MeshComponent{
        mesh: asset_manager.get_mesh_by_name("assets/capsule.obj").unwrap()
    };
    let material_component = MaterialComponent{
        material: asset_manager.get_material_by_name("capsule_mat").unwrap()
    };
    let transform_component = TransformComponent{
        transform: Transform::new()
    };
    
    entities.add_entity((&mut meshes, &mut materials, &mut transforms), (mesh_component, material_component, transform_component));

}

pub fn handle_keyboard_input(key_event: KeyEvent, mut state: UniqueViewMut<State>) {
    state.handle_keyboard(key_event);
}

pub fn handle_mouse_input(delta: (f64, f64), mut state: UniqueViewMut<State>) {
    state.handle_mouse(delta);
}

pub fn resize_system((width, height): (u32, u32), mut state: UniqueViewMut<State>) {
    state.resize(width, height);
}

pub fn update_system(mut state: UniqueViewMut<State>) {
    state.update();
}

pub fn render_system(world: &World, state: UniqueView<State>){
    // Acquire next swapchain frame
    let frame = match state.surface.get_current_texture() {
        Ok(frame) => frame,
        Err(e) => {
            error!("{:?}", e);
            return;
        }
    };

    let frame_view = frame.texture.create_view(&Default::default());

    let depth_view = state.depth_texture.as_ref().unwrap().view.clone();


    // Create a command encoder
    let mut encoder =
        state
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Main Command Encoder"),
            });


    {
        let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &frame_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color{
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0
                    }),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(
                RenderPassDepthStencilAttachment{
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


        world.run_with_data(render_objects_system, &mut rpass);
    }

    // Submit command buffers
    state.queue.submit(std::iter::once(encoder.finish()));
    frame.present();
}
pub fn render_objects_system<'a>(render_pass: &mut RenderPass<'a>, state: UniqueView<State>, mesh_comps: View<MeshComponent>, mat_comps: View<MaterialComponent>, transform_comps: View<TransformComponent>) {
    // Bind global resources first (group 0)
    render_pass.set_bind_group(0, &*state.global_bind_group, &[]);

    // Get the pipeline from the material.
    (&mesh_comps, &mat_comps, &transform_comps).iter().for_each(|(mesh_comp, mat_comp, transform_comp)| {
        let pipeline = mat_comp.material.get_pipeline();
        render_pass.set_pipeline(&pipeline);

        // Bind the material's bind group (typically group 1)
        // Use std::mem::transmute to convert the lifetime of material
        // to 'a, which is the lifetime of the render pass.
        unsafe {
            std::mem::transmute::<&MaterialComponent, &'a MaterialComponent>(mat_comp).material.bind(render_pass);
        }

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
        unsafe{
            std::mem::transmute::<&MeshComponent, &'a MeshComponent>(mesh_comp).mesh.draw(render_pass);
        }
    });

}

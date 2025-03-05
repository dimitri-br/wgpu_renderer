use log::error;
use shipyard::{IntoIter, UniqueView, UniqueViewMut, View, World};
use wgpu::{Color, CommandEncoderDescriptor, LoadOp, Operations, RenderPass, RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor, StoreOp};
use winit::event::KeyEvent;
use crate::renderer::components::*;
use crate::renderer::State;
use crate::renderer::types::material::Material;

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

use std::iter;
use std::sync::Arc;
use log::warn;
use shipyard::{AllStorages, SharedBorrow, TrackingTimestamp, UniqueView, UniqueViewMut};
use wgpu::SurfaceError;
use crate::renderer::ecs::camera_component::CameraComponent;
use crate::renderer::ecs::global_component::GlobalComponent;
use crate::renderer::ecs::instancing_component::InstancingComponent;
use crate::renderer::ecs::light_manager::LightManager;
use crate::renderer::render_batcher::RenderBatcher;
use crate::renderer::shadow_atlas::ShadowAtlas;
use crate::renderer::State;

pub struct RenderGraphicsViewMut<'v> {
    pub encoder: Option<wgpu::CommandEncoder>,
    pub view: Arc<wgpu::TextureView>,
    // New fields
    pub output: Option<wgpu::SurfaceTexture>,
    pub state: UniqueViewMut<'v, State>,
    pub global_component: UniqueView<'v, GlobalComponent>,
    pub light_manager: UniqueView<'v, LightManager>,
    pub camera_component: UniqueView<'v, CameraComponent>,
    pub shadow_atlas: UniqueViewMut<'v, ShadowAtlas>,
    pub instancing_component: UniqueView<'v, InstancingComponent>,
    pub render_batcher: UniqueView<'v, RenderBatcher>,
}

impl shipyard::Borrow for RenderGraphicsViewMut<'_> {
    type View<'v> = RenderGraphicsViewMut<'v>;

    fn borrow<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'a>, shipyard::error::GetStorage> {
        // Even if we don't use tracking for Graphics, it's good to build an habit of using last_run and current when creating custom views
        let mut state =
            UniqueViewMut::<State>::borrow(&all_storages, all_borrow.clone(), last_run, current)?;
        state.resize();

        let global_component = UniqueView::<GlobalComponent>::borrow(&all_storages, all_borrow.clone(), last_run, current)?;
        let light_manager = UniqueView::<LightManager>::borrow(&all_storages, all_borrow.clone(), last_run, current)?;
        let camera_component = UniqueView::<CameraComponent>::borrow(&all_storages, all_borrow.clone(), last_run, current)?;
        let shadow_atlas = UniqueViewMut::<ShadowAtlas>::borrow(&all_storages, all_borrow.clone(), last_run, current)?;
        let instancing_component = UniqueView::<InstancingComponent>::borrow(&all_storages, all_borrow.clone(), last_run, current)?;
        let render_batcher = UniqueView::<RenderBatcher>::borrow(&all_storages, all_borrow.clone(), last_run, current)?;

        // This error will now be reported as an error during the view creation process and not the system but is still bubbled up
        let output = try_get_texture(&state, state.surface.get_current_texture()).unwrap();


        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());


        let encoder = state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });


        Ok(RenderGraphicsViewMut {
            encoder: Some(encoder),
            view: Arc::new(view),
            output: Some(output),
            state,
            global_component,
            light_manager,
            camera_component,
            shadow_atlas,
            instancing_component,
            render_batcher,
        })
    }
}

impl Drop for RenderGraphicsViewMut<'_> {
    fn drop(&mut self) {
        self.state.queue.submit(iter::once(self.encoder.take().unwrap().finish()));

        // Present
        self.output.take().unwrap().present();
    }
}

fn try_get_texture(state: &UniqueViewMut<State>, texture: Result<wgpu::SurfaceTexture, SurfaceError>) -> Result<wgpu::SurfaceTexture, SurfaceError> {
    match texture {
        Ok(texture) => Ok(texture),
        Err(SurfaceError::Lost) | Err(SurfaceError::Outdated) => {
            warn!("Lost texture for {:?}", texture);
            state.surface.configure(&state.device, &state.surface_config);
            let surface = state.surface.get_current_texture();
            try_get_texture(state, surface)
        },
        Err(e) => Err(e),
    }
}
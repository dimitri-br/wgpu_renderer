// src/renderer/ecs/light_update_view.rs

use shipyard::{AllStorages, SharedBorrow, TrackingTimestamp, UniqueView, UniqueViewMut, Borrow};
use crate::renderer::ecs::camera_component::CameraComponent;
use crate::renderer::ecs::global_component::GlobalComponent;
use crate::renderer::ecs::light_manager::LightManager;

pub struct LightUpdateViewMut<'v> {
    pub global_component: UniqueViewMut<'v, GlobalComponent>,
    pub light_manager: UniqueViewMut<'v, LightManager>,
    pub camera_component: UniqueView<'v, CameraComponent>,
}

impl<'a> Borrow for LightUpdateViewMut<'a> {
    type View<'v> = LightUpdateViewMut<'v>;

    fn borrow<'v>(
        all_storages: &'v AllStorages,
        all_borrow: Option<SharedBorrow<'v>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Result<Self::View<'v>, shipyard::error::GetStorage> {
        let global_component = UniqueViewMut::<GlobalComponent>::borrow(
            all_storages,
            all_borrow.clone(),
            last_run,
            current,
        )?;
        let light_manager = UniqueViewMut::<LightManager>::borrow(
            all_storages,
            all_borrow.clone(),
            last_run,
            current,
        )?;
        let camera_component = UniqueView::<CameraComponent>::borrow(
            all_storages,
            all_borrow,
            last_run,
            current,
        )?;
        Ok(LightUpdateViewMut {
            global_component,
            light_manager,
            camera_component,
        })
    }
}

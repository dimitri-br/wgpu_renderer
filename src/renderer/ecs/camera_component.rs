use std::sync::Arc;
use shipyard::Unique;
use crate::renderer::types::camera::Camera;

// CameraComponent is unique as currently only one camera is supported
#[derive(Unique)]
pub struct CameraComponent{
    pub(crate) camera: Box<dyn Camera>,
}

impl CameraComponent{
    pub fn new<T: Camera + 'static>(camera: T) -> Self {
        Self {
            camera: camera.into(),
        }
    }
}

impl<T: Camera + 'static> From<T> for CameraComponent {
    fn from(camera: T) -> Self {
        Self::new(camera)
    }
}
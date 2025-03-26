use std::ops::Deref;
use std::sync::Arc;
use shipyard::Unique;
use crate::renderer::types::camera::Camera;
use crate::renderer::types::frustum::Frustum;

// CameraComponent is unique as currently only one camera is supported
#[derive(Unique)]
pub struct CameraComponent{
    pub(crate) camera: Box<dyn Camera>,
    pub(crate) frustum: Frustum,
}

impl CameraComponent{
    pub fn new<T: Camera + 'static>(camera: T) -> Self {
        let camera = camera.into();
        let frustum = Frustum::from_camera(&camera);
        Self {
            camera,
            frustum
        }
    }
}

impl<T: Camera + 'static> From<T> for CameraComponent {
    fn from(camera: T) -> Self {
        Self::new(camera)
    }
}

impl Deref for CameraComponent{
    type Target = dyn Camera;

    fn deref(&self) -> &Self::Target {
        &*self.camera
    }
}
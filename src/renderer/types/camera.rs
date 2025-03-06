use glam::Mat4;

pub trait Camera: Send + Sync {
    /// Get the camera's view-projection matrix.
    fn build_view_projection_matrix(&self) -> Mat4;
    /// Update the camera's aspect ratio.
    fn resize(&mut self, width: f32, height: f32);
    /// Update the camera's internal state.
    fn update(&mut self, dt: f32);
    /// Handle keyboard input.
    fn process_keyboard(&mut self, event: winit::event::KeyEvent);
    /// Handle mouse input.
    fn process_mouse(&mut self, delta_x: f32, delta_y: f32);
}

impl<T: Camera + 'static> From<T> for Box<dyn Camera> {
    fn from(camera: T) -> Self {
        Box::new(camera)
    }
}

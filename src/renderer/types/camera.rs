use glam::Mat4;

pub trait Camera {
    /// Get the camera's view-projection matrix.
    fn build_view_projection_matrix(&self) -> Mat4;
}

use glam::{Vec3, Vec4};
use crate::renderer::types::camera::Camera;
use crate::renderer::types::plane::Plane;

/// Stores six planes that define a camera frustum.
/// The typical order is: left, right, bottom, top, near, far.
#[derive(Clone, Debug)]
pub struct Frustum {
    pub planes: Vec<Plane>,
}

impl Frustum{
    pub fn from_camera(camera: &Box<dyn Camera>) -> Self{
        let view_proj = camera.build_view_projection_matrix();

        // Each plane can be extracted by taking certain combinations
        // of rows (or columns, depending on row-major vs. column-major).
        // For example, the left plane is (m[3] + m[0]), the right plane is (m[3] - m[0]), etc.
        // Then normalize each plane.
        let m0 = view_proj.row(0);
        let m1 = view_proj.row(1);
        let m2 = view_proj.row(2);
        let m3 = view_proj.row(3);

        // In glam, row(i) returns a Vec4. We can interpret (x,y,z,w) as plane normal + distance.
        // For example:
        let left  = (m3 + m0).into();
        let right = (m3 - m0).into();
        let bottom = (m3 + m1).into();
        let top   = (m3 - m1).into();
        let near_ = (m3 + m2).into();
        let far_  = (m3 - m2).into();

        let planes = vec![
            normalize_plane(left),
            normalize_plane(right),
            normalize_plane(bottom),
            normalize_plane(top),
            normalize_plane(near_),
            normalize_plane(far_)
            ];

        Self{
            planes
        }
    }
}


fn normalize_plane(v: Vec4) -> Plane {
    let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    Plane::new(
        Vec3::new(v.x / len, v.y / len, v.z / len),
        v.w / len
    )
}
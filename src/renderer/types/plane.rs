use glam::Vec3;

#[derive(Copy, Clone, Debug)]
pub struct Plane{
    pub(crate) normal: Vec3,
    distance: f32,
}

impl Plane {
    /// Create a plane from a normalized normal and a scalar distance.
    /// The plane equation is normal.dot(x) + d = 0.
    pub fn new(normal: Vec3, distance: f32) -> Self {
        Self { normal, distance }
    }

    /// Computes the signed distance from a point to this plane.
    pub fn distance(&self, point: Vec3) -> f32 {
        self.normal.dot(point) + self.distance
    }
}
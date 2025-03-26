use glam::Vec3;
use crate::renderer::types::frustum::Frustum;
use crate::renderer::types::transform::Transform;

#[derive(Copy, Clone, Debug)]
pub struct AABB{
    pub min:        Vec3,
    pub max:        Vec3,
}

impl AABB {
    /// Create an AABB from explicit `min` and `max` corners.
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Create an "empty" AABB with inverted bounds.
    /// Useful as a starting point for incremental expansion.
    pub fn empty() -> Self {
        Self {
            min: Vec3::splat(f32::MAX),
            max: Vec3::splat(f32::MIN),
        }
    }

    /// Build an AABB that encloses a set of points.
    pub fn from_points(points: &[Vec3]) -> Self {
        let mut aabb = AABB::empty();
        for &p in points {
            aabb.expand(p);
        }
        aabb
    }

    /// Expands this AABB to include the given point.
    pub fn expand(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }

    /// Returns the center of this AABB.
    pub fn center(&self) -> Vec3 {
        0.5 * (self.min + self.max)
    }

    /// Returns half the width/height/depth of this AABB.
    pub fn extents(&self) -> Vec3 {
        0.5 * (self.max - self.min)
    }

    /// Merges two AABBs into one that encloses both.
    /// Useful for building bounding volumes in a tree.
    pub fn union(&self, other: &AABB) -> AABB {
        AABB {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// Transform an AABB from a given transform
    /// Useful for converting from object space
    /// to world space
    /// Transform this AABB by the given Transform (pos, rot, scale).
    /// Returns a new, world-space AABB.
    pub fn transform_aabb(&self, transform: &Transform) -> AABB {
        // 1. Build a 4x4 matrix from Transform
        let mat = transform.matrix;

        // 2. Extract the 8 corners of the object-space AABB
        let corners = [
            Vec3::new(self.min.x, self.min.y, self.min.z),
            Vec3::new(self.min.x, self.min.y, self.max.z),
            Vec3::new(self.min.x, self.max.y, self.min.z),
            Vec3::new(self.min.x, self.max.y, self.max.z),
            Vec3::new(self.max.x, self.min.y, self.min.z),
            Vec3::new(self.max.x, self.min.y, self.max.z),
            Vec3::new(self.max.x, self.max.y, self.min.z),
            Vec3::new(self.max.x, self.max.y, self.max.z),
        ];

        // 3. Multiply each corner by the matrix
        let mut new_min = Vec3::splat(f32::MAX);
        let mut new_max = Vec3::splat(f32::MIN);

        for &corner in &corners {
            let world_pos = mat * corner.extend(1.0);
            let wp3 = world_pos.truncate();

            // 4. Track min/max
            new_min = new_min.min(wp3);
            new_max = new_max.max(wp3);
        }

        AABB {
            min: new_min,
            max: new_max,
        }
    }

    /// Tests if this AABB is *potentially* visible within the given frustum.
    /// Returns `false` if it is definitely outside, `true` if it might be inside.
    pub fn is_in_frustum(&self, frustum: &Frustum) -> bool {
        // For each plane in the frustum, test if the AABB is *behind* it.
        // We'll use the "most positive corner" trick (also called the p-vertex):
        // If the plane normal is positive, pick the box's max corner in that axis; else pick the min corner.
        //
        // If the signed distance of that corner is < 0, the entire box is behind the plane -> cull.

        for plane in &frustum.planes {
            // Build the "p-vertex" for this plane's normal.
            // i.e., the corner of AABB that's farthest in the direction of plane.normal.
            let px = if plane.normal.x >= 0.0 { self.max.x } else { self.min.x };
            let py = if plane.normal.y >= 0.0 { self.max.y } else { self.min.y };
            let pz = if plane.normal.z >= 0.0 { self.max.z } else { self.min.z };
            let p_vertex = Vec3::new(px, py, pz);

            // Distance from plane
            let dist = plane.distance(p_vertex);
            // If the p-vertex is behind this plane, then the box is completely out of this frustum.
            if dist < 0.0 {
                return false;
            }
        }
        // Passed all planes, so it's potentially inside.
        true
    }
}
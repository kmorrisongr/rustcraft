use bevy::prelude::*;

/// Represents a plane in 3D space using the equation: ax + by + cz + d = 0
/// The plane is normalized so that (a, b, c) is a unit vector.
#[derive(Debug, Clone, Copy)]
pub struct Plane {
    pub normal: Vec3,
    pub distance: f32,
}

impl Plane {
    /// Creates a new plane from the coefficients a, b, c, d
    /// The plane equation is: ax + by + cz + d = 0
    pub fn new(a: f32, b: f32, c: f32, d: f32) -> Self {
        let normal = Vec3::new(a, b, c);
        let length = normal.length();

        // Normalize the plane to avoid precision issues
        // This is critical to prevent chunks being culled at screen edges
        if length > 0.0 {
            Self {
                normal: normal / length,
                distance: d / length,
            }
        } else {
            Self {
                normal: Vec3::ZERO,
                distance: 0.0,
            }
        }
    }

    /// Computes the signed distance from a point to this plane
    /// Positive means the point is on the side the normal points to
    #[inline]
    pub fn distance_to_point(&self, point: Vec3) -> f32 {
        self.normal.dot(point) + self.distance
    }
}

/// Frustum planes enumeration for clarity
#[derive(Debug, Clone, Copy)]
pub enum FrustumPlane {
    Left = 0,
    Right = 1,
    Bottom = 2,
    Top = 3,
    Near = 4,
    Far = 5,
}

/// View frustum represented by 6 planes
/// Uses normalized planes to prevent floating-point precision issues
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    pub planes: [Plane; 6],
}

impl Frustum {
    /// Extracts frustum planes from a view-projection matrix using the Gribb-Hartmann method
    /// This method extracts planes directly from the combined view-projection matrix.
    ///
    /// Reference: "Fast Extraction of Viewing Frustum Planes from the World-View-Projection Matrix"
    /// by Gil Gribb and Klaus Hartmann
    pub fn from_view_projection_matrix(view_projection: &Mat4) -> Self {
        // Extract the rows of the matrix
        let m = view_projection.to_cols_array();

        // The Gribb-Hartmann method extracts planes by combining matrix rows:
        // Left:   row4 + row1
        // Right:  row4 - row1
        // Bottom: row4 + row2
        // Top:    row4 - row2
        // Near:   row4 + row3
        // Far:    row4 - row3
        //
        // Where row1, row2, row3, row4 are the first, second, third, and fourth rows

        let mut planes = [Plane::new(0.0, 0.0, 0.0, 0.0); 6];

        // Left plane: m[3] + m[0]
        planes[FrustumPlane::Left as usize] =
            Plane::new(m[3] + m[0], m[7] + m[4], m[11] + m[8], m[15] + m[12]);

        // Right plane: m[3] - m[0]
        planes[FrustumPlane::Right as usize] =
            Plane::new(m[3] - m[0], m[7] - m[4], m[11] - m[8], m[15] - m[12]);

        // Bottom plane: m[3] + m[1]
        planes[FrustumPlane::Bottom as usize] =
            Plane::new(m[3] + m[1], m[7] + m[5], m[11] + m[9], m[15] + m[13]);

        // Top plane: m[3] - m[1]
        planes[FrustumPlane::Top as usize] =
            Plane::new(m[3] - m[1], m[7] - m[5], m[11] - m[9], m[15] - m[13]);

        // Near plane: m[3] + m[2]
        planes[FrustumPlane::Near as usize] =
            Plane::new(m[3] + m[2], m[7] + m[6], m[11] + m[10], m[15] + m[14]);

        // Far plane: m[3] - m[2]
        planes[FrustumPlane::Far as usize] =
            Plane::new(m[3] - m[2], m[7] - m[6], m[11] - m[10], m[15] - m[14]);

        Self { planes }
    }

    /// Tests if an axis-aligned bounding box intersects or is inside the frustum
    /// Uses the P-vertex/N-vertex optimization for efficiency
    ///
    /// The P-vertex (positive vertex) is the corner of the AABB that is furthest along
    /// the plane's normal direction. The N-vertex (negative vertex) is the opposite corner.
    ///
    /// If the P-vertex is outside (negative side) of any plane, the entire AABB is outside.
    /// This allows for early rejection without testing all 8 corners.
    pub fn intersects_aabb(&self, min: Vec3, max: Vec3) -> bool {
        for plane in &self.planes {
            // For each plane, find the P-vertex (positive vertex)
            // The P-vertex is the corner of the AABB that is furthest in the
            // direction of the plane's normal
            let p_vertex = Vec3::new(
                if plane.normal.x >= 0.0 { max.x } else { min.x },
                if plane.normal.y >= 0.0 { max.y } else { min.y },
                if plane.normal.z >= 0.0 { max.z } else { min.z },
            );

            // If the P-vertex is on the negative side of the plane,
            // the entire AABB is outside the frustum
            if plane.distance_to_point(p_vertex) < 0.0 {
                return false;
            }

            // Note: We could also test the N-vertex for tighter bounds:
            // let n_vertex = Vec3::new(
            //     if plane.normal.x >= 0.0 { min.x } else { max.x },
            //     if plane.normal.y >= 0.0 { min.y } else { max.y },
            //     if plane.normal.z >= 0.0 { min.z } else { max.z },
            // );
            // if plane.distance_to_point(n_vertex) >= 0.0 {
            //     // AABB is completely inside this plane
            // }
            // But for culling, we only need to know if it's completely outside
        }

        // If we get here, the AABB is at least partially inside the frustum
        true
    }

    /// Convenience method to test a chunk's AABB given its position and size
    /// Uses double precision for chunk position to avoid floating-point errors
    /// with large world coordinates
    pub fn intersects_chunk(&self, chunk_pos: IVec3, chunk_size: i32) -> bool {
        // Convert chunk position to world space using doubles to avoid precision loss
        let chunk_world_x = chunk_pos.x as f64 * chunk_size as f64;
        let chunk_world_y = chunk_pos.y as f64 * chunk_size as f64;
        let chunk_world_z = chunk_pos.z as f64 * chunk_size as f64;

        // Calculate AABB bounds, then convert to f32 for the test
        // This approach minimizes floating-point error accumulation
        let min = Vec3::new(
            chunk_world_x as f32,
            chunk_world_y as f32,
            chunk_world_z as f32,
        );

        let max = Vec3::new(
            (chunk_world_x + chunk_size as f64) as f32,
            (chunk_world_y + chunk_size as f64) as f32,
            (chunk_world_z + chunk_size as f64) as f32,
        );

        self.intersects_aabb(min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plane_normalization() {
        let plane = Plane::new(3.0, 4.0, 0.0, 10.0);
        // Length of (3, 4, 0) is 5
        assert!((plane.normal.length() - 1.0).abs() < 0.001);
        assert!((plane.normal.x - 0.6).abs() < 0.001);
        assert!((plane.normal.y - 0.8).abs() < 0.001);
        assert!((plane.distance - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_frustum_culling_simple() {
        // Create a simple orthographic-like frustum for testing
        let frustum = Frustum {
            planes: [
                Plane::new(1.0, 0.0, 0.0, 10.0),   // Left
                Plane::new(-1.0, 0.0, 0.0, 10.0),  // Right
                Plane::new(0.0, 1.0, 0.0, 10.0),   // Bottom
                Plane::new(0.0, -1.0, 0.0, 10.0),  // Top
                Plane::new(0.0, 0.0, 1.0, 10.0),   // Near
                Plane::new(0.0, 0.0, -1.0, 100.0), // Far
            ],
        };

        // AABB inside frustum
        assert!(frustum.intersects_aabb(Vec3::ZERO, Vec3::ONE));

        // AABB far outside frustum (beyond far plane)
        assert!(!frustum.intersects_aabb(Vec3::new(0.0, 0.0, 200.0), Vec3::new(1.0, 1.0, 201.0)));
    }

    #[test]
    fn test_chunk_intersection() {
        // Create a simple frustum
        let frustum = Frustum {
            planes: [
                Plane::new(1.0, 0.0, 0.0, 100.0),   // Left
                Plane::new(-1.0, 0.0, 0.0, 100.0),  // Right
                Plane::new(0.0, 1.0, 0.0, 100.0),   // Bottom
                Plane::new(0.0, -1.0, 0.0, 100.0),  // Top
                Plane::new(0.0, 0.0, 1.0, 100.0),   // Near
                Plane::new(0.0, 0.0, -1.0, 1000.0), // Far
            ],
        };

        // Chunk at origin should be visible
        assert!(frustum.intersects_chunk(IVec3::ZERO, 16));

        // Chunk far away should not be visible
        assert!(!frustum.intersects_chunk(IVec3::new(0, 0, 2000), 16));
    }
}

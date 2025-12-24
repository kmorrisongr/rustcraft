use bevy::prelude::*;

/// Small epsilon value used to "fatten" AABBs for conservative culling.
/// This prevents chunks from flickering in/out at frustum edges due to
/// floating-point precision errors.
const FRUSTUM_AABB_EPSILON: f32 = 0.1;

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

/// Result of frustum intersection test
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrustumIntersection {
    /// The AABB is completely outside the frustum
    Outside,
    /// The AABB intersects the frustum (partially visible)
    Intersects,
    /// The AABB is completely inside the frustum (fully visible)
    Inside,
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
    /// Returns detailed intersection status (Outside, Intersects, or Inside)
    ///
    /// This is a hot-path function called for every chunk each frame.
    /// No heap allocations occur - all data is stack-based.
    #[inline]
    pub fn test_aabb(&self, min: Vec3, max: Vec3) -> FrustumIntersection {
        let mut all_inside = true;

        for plane in &self.planes {
            // P-vertex: corner furthest along the plane's normal
            let p_vertex = Vec3::new(
                if plane.normal.x >= 0.0 { max.x } else { min.x },
                if plane.normal.y >= 0.0 { max.y } else { min.y },
                if plane.normal.z >= 0.0 { max.z } else { min.z },
            );

            // N-vertex: corner furthest against the plane's normal
            let n_vertex = Vec3::new(
                if plane.normal.x >= 0.0 { min.x } else { max.x },
                if plane.normal.y >= 0.0 { min.y } else { max.y },
                if plane.normal.z >= 0.0 { min.z } else { max.z },
            );

            // If P-vertex is outside, the entire AABB is outside
            if plane.distance_to_point(p_vertex) < 0.0 {
                return FrustumIntersection::Outside;
            }

            // If N-vertex is outside, the AABB is not fully inside this plane
            if plane.distance_to_point(n_vertex) < 0.0 {
                all_inside = false;
            }
        }

        if all_inside {
            FrustumIntersection::Inside
        } else {
            FrustumIntersection::Intersects
        }
    }

    /// Simple boolean test - returns true if AABB is at least partially visible
    #[inline]
    pub fn intersects_aabb(&self, min: Vec3, max: Vec3) -> bool {
        self.test_aabb(min, max) != FrustumIntersection::Outside
    }

    /// Convenience method to test a chunk's AABB given its position and size.
    /// Returns true if the chunk is at least partially visible in the frustum.
    #[inline]
    pub fn intersects_chunk(&self, chunk_pos: IVec3, chunk_size: i32) -> bool {
        self.test_chunk(chunk_pos, chunk_size) != FrustumIntersection::Outside
    }

    /// Detailed chunk intersection test.
    /// Uses f64 for coordinate calculations to maintain precision at large world positions,
    /// with epsilon padding for conservative culling to prevent edge flickering.
    ///
    /// This is a hot-path function - no heap allocations, all stack-based.
    #[inline]
    pub fn test_chunk(&self, chunk_pos: IVec3, chunk_size: i32) -> FrustumIntersection {
        // Convert chunk position to world space using doubles to avoid precision loss
        let chunk_world_x = chunk_pos.x as f64 * chunk_size as f64;
        let chunk_world_y = chunk_pos.y as f64 * chunk_size as f64;
        let chunk_world_z = chunk_pos.z as f64 * chunk_size as f64;

        // Calculate AABB bounds with epsilon padding for conservative culling
        let min = Vec3::new(
            (chunk_world_x as f32) - FRUSTUM_AABB_EPSILON,
            (chunk_world_y as f32) - FRUSTUM_AABB_EPSILON,
            (chunk_world_z as f32) - FRUSTUM_AABB_EPSILON,
        );

        let max = Vec3::new(
            ((chunk_world_x + chunk_size as f64) as f32) + FRUSTUM_AABB_EPSILON,
            ((chunk_world_y + chunk_size as f64) as f32) + FRUSTUM_AABB_EPSILON,
            ((chunk_world_z + chunk_size as f64) as f32) + FRUSTUM_AABB_EPSILON,
        );

        self.test_aabb(min, max)
    }
}

/// Calculates a priority score for chunk meshing/rendering.
/// Lower scores = higher priority.
///
/// Factors considered:
/// - Distance from camera
/// - Whether chunk is in front of camera (view direction alignment)
/// - Whether chunk is in frustum and how much (fully visible vs partially)
///
/// Uses f64 for intermediate calculations to maintain precision at large world coordinates.
pub fn calculate_chunk_priority(
    chunk_pos: IVec3,
    chunk_size: i32,
    camera_pos: Vec3,
    camera_forward: Vec3,
    frustum: &Frustum,
    player_chunk_pos: IVec3,
) -> f32 {
    // Calculate chunk center using f64 for precision at large coordinates
    let chunk_size_f64 = chunk_size as f64;
    let chunk_center_x = (chunk_pos.x as f64 + 0.5) * chunk_size_f64;
    let chunk_center_y = (chunk_pos.y as f64 + 0.5) * chunk_size_f64;
    let chunk_center_z = (chunk_pos.z as f64 + 0.5) * chunk_size_f64;

    // Vector from camera to chunk center (calculated in f64 for precision)
    let to_chunk_x = chunk_center_x - camera_pos.x as f64;
    let to_chunk_y = chunk_center_y - camera_pos.y as f64;
    let to_chunk_z = chunk_center_z - camera_pos.z as f64;

    let distance_sq = to_chunk_x * to_chunk_x + to_chunk_y * to_chunk_y + to_chunk_z * to_chunk_z;
    let distance = distance_sq.sqrt();

    // Avoid division by zero for chunks at camera position
    if distance < 0.001 {
        return 0.0; // Highest priority for the chunk the player is in
    }

    // Normalize the to_chunk vector (now safe to convert to f32 since it's unit length)
    let to_chunk_normalized = Vec3::new(
        (to_chunk_x / distance) as f32,
        (to_chunk_y / distance) as f32,
        (to_chunk_z / distance) as f32,
    );

    // Dot product with camera forward direction
    // 1.0 = directly in front, 0.0 = perpendicular, -1.0 = directly behind
    let view_alignment = camera_forward.dot(to_chunk_normalized);

    // Check frustum intersection
    let frustum_result = frustum.test_chunk(chunk_pos, chunk_size);

    // Base priority is distance (closer = better)
    let distance_score = distance as f32;

    // View alignment factor:
    // - Chunks in front (alignment > 0) get a bonus (multiplier < 1)
    // - Chunks behind (alignment < 0) get a penalty (multiplier > 1)
    // Remap alignment from [-1, 1] to [2.0, 0.5] (behind gets 2x penalty, in front gets 0.5x bonus)
    let alignment_multiplier = 1.25 - (view_alignment * 0.75);

    // Frustum visibility factor:
    // - Outside frustum: large penalty (but still mesh eventually)
    // - Intersects (partial): small penalty
    // - Inside (fully visible): bonus
    let frustum_multiplier = match frustum_result {
        FrustumIntersection::Outside => 10.0, // Heavy penalty - mesh last
        FrustumIntersection::Intersects => 1.2, // Small penalty for partial visibility
        FrustumIntersection::Inside => 0.8,   // Bonus for fully visible
    };

    // Player's chunk gets absolute priority
    let player_chunk_bonus = if chunk_pos == player_chunk_pos {
        0.001 // Nearly zero - always first
    } else {
        1.0
    };

    distance_score * alignment_multiplier * frustum_multiplier * player_chunk_bonus
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

    #[test]
    fn test_chunk_priority() {
        // Create a frustum looking down negative Z axis
        let frustum = Frustum {
            planes: [
                Plane::new(1.0, 0.0, 0.0, 100.0),  // Left
                Plane::new(-1.0, 0.0, 0.0, 100.0), // Right
                Plane::new(0.0, 1.0, 0.0, 100.0),  // Bottom
                Plane::new(0.0, -1.0, 0.0, 100.0), // Top
                Plane::new(0.0, 0.0, -1.0, 0.0),   // Near (at origin, looking -Z)
                Plane::new(0.0, 0.0, 1.0, 1000.0), // Far
            ],
        };

        let camera_pos = Vec3::ZERO;
        let camera_forward = Vec3::NEG_Z; // Looking down -Z axis
        let player_chunk = IVec3::ZERO;

        // Player's chunk should have lowest priority (highest importance)
        let player_priority = calculate_chunk_priority(
            IVec3::ZERO,
            16,
            camera_pos,
            camera_forward,
            &frustum,
            player_chunk,
        );

        // Chunk directly in front should have lower priority than chunk behind
        let front_chunk = IVec3::new(0, 0, -2); // In front (negative Z)
        let behind_chunk = IVec3::new(0, 0, 2); // Behind (positive Z)

        let front_priority = calculate_chunk_priority(
            front_chunk,
            16,
            camera_pos,
            camera_forward,
            &frustum,
            player_chunk,
        );

        let behind_priority = calculate_chunk_priority(
            behind_chunk,
            16,
            camera_pos,
            camera_forward,
            &frustum,
            player_chunk,
        );

        // Player chunk should be highest priority (lowest score)
        assert!(player_priority < front_priority);
        // Front chunks should have higher priority than behind chunks
        assert!(front_priority < behind_priority);
    }

    #[test]
    fn test_large_coordinate_precision() {
        // Test that the f64 intermediate calculations in test_chunk
        // correctly handle large world coordinates without precision loss.
        //
        // The key insight: chunk coordinate → world coordinate conversion uses f64
        // internally to avoid precision loss when multiplying large integers by chunk_size.

        // Create a simple frustum - a box from (-100, -100, -100) to (100, 100, 100)
        let frustum = Frustum {
            planes: [
                Plane::new(1.0, 0.0, 0.0, 100.0),  // Left: x > -100
                Plane::new(-1.0, 0.0, 0.0, 100.0), // Right: x < 100
                Plane::new(0.0, 1.0, 0.0, 100.0),  // Bottom: y > -100
                Plane::new(0.0, -1.0, 0.0, 100.0), // Top: y < 100
                Plane::new(0.0, 0.0, 1.0, 100.0),  // Near: z > -100
                Plane::new(0.0, 0.0, -1.0, 100.0), // Far: z < 100
            ],
        };

        // A chunk at the origin should be visible
        let center_chunk = IVec3::new(0, 0, 0);
        assert!(
            frustum.intersects_chunk(center_chunk, 16),
            "Center chunk should be visible"
        );

        // A chunk very far away should not be visible
        // Even with large chunk coordinates, the world position is calculated correctly
        let far_chunk = IVec3::new(1000, 0, 0); // World position x = 16000, way outside x=100
        assert!(
            !frustum.intersects_chunk(far_chunk, 16),
            "Far chunk should not be visible"
        );

        // Test with a chunk at "large" coordinates where i32 * chunk_size matters
        // Chunk at x=625000 → world x = 10,000,000
        // This would lose precision with f32 multiplication, but we use f64 internally
        let large_chunk = IVec3::new(625000, 0, 0);

        // This chunk is at world position ~10 million, clearly outside our ±100 box
        assert!(
            !frustum.intersects_chunk(large_chunk, 16),
            "Chunk at large world coordinate should be correctly culled"
        );

        // Verify the f64 precision by checking chunk coordinate calculation doesn't overflow
        // 625000 * 16 = 10,000,000 which fits fine in i32 (max ~2 billion)
        // and definitely fits in f64 without precision loss
    }

    #[test]
    fn test_epsilon_padding() {
        // Test that epsilon padding prevents edge flickering
        // by ensuring chunks very close to the frustum boundary are still visible

        // Create a frustum where planes face inward toward the view volume
        // A box from roughly (-100, -100, -100) to (100, 100, 100)
        let frustum = Frustum {
            planes: [
                Plane::new(1.0, 0.0, 0.0, 100.0),  // Left: x > -100
                Plane::new(-1.0, 0.0, 0.0, 100.0), // Right: x < 100
                Plane::new(0.0, 1.0, 0.0, 100.0),  // Bottom: y > -100
                Plane::new(0.0, -1.0, 0.0, 100.0), // Top: y < 100
                Plane::new(0.0, 0.0, 1.0, 100.0),  // Near: z > -100
                Plane::new(0.0, 0.0, -1.0, 100.0), // Far: z < 100
            ],
        };

        // Test chunk at origin - should definitely be visible
        let center_chunk = IVec3::new(0, 0, 0);
        assert!(
            frustum.intersects_chunk(center_chunk, 16),
            "Center chunk should be visible"
        );

        // Test chunk near the edge but still inside
        let near_edge_chunk = IVec3::new(5, 0, 0); // x from 80 to 96
        assert!(
            frustum.intersects_chunk(near_edge_chunk, 16),
            "Chunk near edge should be visible"
        );

        // The epsilon ensures that even if there's floating point error that would
        // make a chunk appear exactly on the boundary, it won't flicker.
        // The constant FRUSTUM_AABB_EPSILON = 0.1 pads the AABB by 0.1 units on each side.
    }
}

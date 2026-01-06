/// Level of Detail (LOD) types and utilities for chunk rendering.
///
/// LOD reduces mesh complexity for distant chunks:
/// - LOD 0: Full detail, 1 block = 1 rendered block
/// - LOD 1: Reduced detail, 2×2×2 blocks = 1 rendered block (at 2× scale)

/// Level of Detail for chunk rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LodLevel {
    #[default]
    Lod0, // Full detail: 1 block = 1 rendered block
    Lod1, // Reduced: 2×2×2 blocks = 1 rendered block at 2× scale
}

impl LodLevel {
    /// Block scale factor (LOD 0 = 1, LOD 1 = 2)
    ///
    /// This is the factor by which blocks are scaled when rendered at this LOD level.
    /// For LOD 0, each block is rendered at its natural size.
    /// For LOD 1, each block represents a 2×2×2 region and is rendered at 2× size.
    pub fn block_scale(&self) -> i32 {
        match self {
            LodLevel::Lod0 => 1,
            LodLevel::Lod1 => 2,
        }
    }

    /// Determine LOD level from squared distance to chunk.
    ///
    /// All distance parameters are squared to avoid expensive sqrt operations.
    ///
    /// # Arguments
    /// * `chunk_distance_sq` - Squared distance from player to chunk
    /// * `lod0_threshold_sq` - Squared distance threshold for LOD 0 (full detail)
    ///
    /// # Returns
    /// * `LodLevel::Lod0` if within LOD 0 threshold
    /// * `LodLevel::Lod1` if beyond LOD 0 threshold (caller should cull beyond LOD 1 range)
    pub fn from_distance_squared(chunk_distance_sq: i32, lod0_threshold_sq: i32) -> Self {
        if chunk_distance_sq <= lod0_threshold_sq {
            LodLevel::Lod0
        } else {
            LodLevel::Lod1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_scale() {
        assert_eq!(LodLevel::Lod0.block_scale(), 1);
        assert_eq!(LodLevel::Lod1.block_scale(), 2);
    }

    #[test]
    fn test_from_distance_squared() {
        let lod0_threshold_sq = 64; // 8×8 = 64

        // Within LOD 0 range
        assert_eq!(
            LodLevel::from_distance_squared(0, lod0_threshold_sq),
            LodLevel::Lod0
        );
        assert_eq!(
            LodLevel::from_distance_squared(64, lod0_threshold_sq),
            LodLevel::Lod0
        );

        // Beyond LOD 0 range
        assert_eq!(
            LodLevel::from_distance_squared(65, lod0_threshold_sq),
            LodLevel::Lod1
        );
        assert_eq!(
            LodLevel::from_distance_squared(100, lod0_threshold_sq),
            LodLevel::Lod1
        );
    }
}

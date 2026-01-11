//! Wave scale calculation based on local water volume.
//!
//! This module computes wave amplitude scaling factors based on the local
//! water volume around each surface cell. The key insight is that wave size
//! should be a function of local water volume, which naturally produces:
//!
//! - Puddles → minor ripples (very low local volume)
//! - Ponds → small waves (moderate local volume)
//! - Lakes → medium waves (high local volume)
//! - Oceans → large waves (very high local volume, especially far from shore)
//!
//! ## Design Principles
//! - Local volume is sampled within a configurable radius
//! - Volume includes both horizontal extent AND depth (vertical stacking)
//! - The mapping from volume to wave scale is non-linear (smoothstep)
//! - Configuration allows tuning the "bucketing" behavior

use bevy::math::IVec3;

use super::ChunkWaterStorage;

// ============================================================================
// Configuration Constants
// ============================================================================

/// Default sampling radius for local volume calculation (in blocks).
/// A radius of 3 means sampling a 7x7 area horizontally (3 blocks in each direction).
pub const DEFAULT_SAMPLE_RADIUS_XZ: i32 = 3;

/// Default vertical sampling depth (number of blocks below surface to check).
/// With a value of 4, this samples 5 blocks total: the surface block plus 4 blocks below.
/// This allows deeper water to contribute more volume.
pub const DEFAULT_SAMPLE_DEPTH: i32 = 4;

/// Minimum wave scale factor (for very shallow/small water bodies).
/// This ensures there's always at least a tiny bit of surface movement.
pub const MIN_WAVE_SCALE: f32 = 0.05;

/// Maximum wave scale factor (for large deep water bodies).
pub const MAX_WAVE_SCALE: f32 = 1.0;

/// Volume threshold below which water is considered "puddle" (minimal waves).
/// This is the total sampled volume, not per-cell.
pub const VOLUME_THRESHOLD_PUDDLE: f32 = 2.0;

/// Volume threshold for "pond" size (small waves).
pub const VOLUME_THRESHOLD_POND: f32 = 8.0;

/// Volume threshold for "lake" size (medium waves).
pub const VOLUME_THRESHOLD_LAKE: f32 = 25.0;

/// Volume threshold for "ocean" size (full waves).
/// Above this threshold, wave scale is at maximum.
pub const VOLUME_THRESHOLD_OCEAN: f32 = 50.0;

// ============================================================================
// Configuration Struct
// ============================================================================

/// Configuration for wave scale calculation.
///
/// This allows tuning the volume sampling and wave scaling behavior.
#[derive(Debug, Clone, Copy)]
pub struct WaveScaleConfig {
    /// Horizontal sampling radius in blocks (samples (2*r+1)² area)
    pub sample_radius_xz: i32,
    /// Vertical sampling depth (number of blocks below surface to check).
    /// Total blocks sampled vertically is sample_depth + 1 (includes surface block).
    pub sample_depth: i32,
    /// Minimum wave scale (0.0 - 1.0)
    pub min_scale: f32,
    /// Maximum wave scale (0.0 - 1.0)
    pub max_scale: f32,
    /// Volume thresholds for wave scale interpolation [puddle, pond, lake, ocean]
    pub volume_thresholds: [f32; 4],
    /// Wave scales corresponding to thresholds [puddle, pond, lake, ocean]
    pub wave_scales: [f32; 4],
}

impl Default for WaveScaleConfig {
    fn default() -> Self {
        Self {
            sample_radius_xz: DEFAULT_SAMPLE_RADIUS_XZ,
            sample_depth: DEFAULT_SAMPLE_DEPTH,
            min_scale: MIN_WAVE_SCALE,
            max_scale: MAX_WAVE_SCALE,
            volume_thresholds: [
                VOLUME_THRESHOLD_PUDDLE,
                VOLUME_THRESHOLD_POND,
                VOLUME_THRESHOLD_LAKE,
                VOLUME_THRESHOLD_OCEAN,
            ],
            wave_scales: [
                0.1, // Puddle: 10% wave amplitude
                0.3, // Pond: 30% wave amplitude
                0.6, // Lake: 60% wave amplitude
                1.0, // Ocean: 100% wave amplitude
            ],
        }
    }
}

impl WaveScaleConfig {
    /// Creates a configuration for small, detailed wave scaling (smaller sampling area).
    pub fn detailed() -> Self {
        Self {
            sample_radius_xz: 2,
            sample_depth: 3,
            ..Default::default()
        }
    }

    /// Creates a configuration for broad wave scaling (larger sampling area).
    pub fn broad() -> Self {
        Self {
            sample_radius_xz: 5,
            sample_depth: 6,
            volume_thresholds: [
                3.0,  // Puddle
                15.0, // Pond
                40.0, // Lake
                80.0, // Ocean
            ],
            ..Default::default()
        }
    }
}

// ============================================================================
// Volume Sampling
// ============================================================================

/// Calculates the local water volume around a position within a single chunk.
///
/// This samples water cells in a configurable radius around the given position
/// and sums their volumes. The result represents the "local water mass" that
/// determines wave amplitude.
///
/// # Arguments
/// * `water` - Water storage for the chunk
/// * `local_pos` - Local position within the chunk (the surface cell)
/// * `config` - Configuration for sampling radius and depth
///
/// # Returns
/// Total volume of water in the sampled region (0.0 to theoretical max based on sample area)
pub fn calculate_local_volume(
    water: &ChunkWaterStorage,
    local_pos: &IVec3,
    config: &WaveScaleConfig,
) -> f32 {
    let mut total_volume = 0.0;
    let r = config.sample_radius_xz;
    let depth = config.sample_depth;

    // Sample horizontal area
    for dx in -r..=r {
        for dz in -r..=r {
            // Sample vertical column (including blocks below)
            for dy in -depth..=0 {
                let sample_pos = IVec3::new(local_pos.x + dx, local_pos.y + dy, local_pos.z + dz);

                // Only sample valid chunk positions (0..CHUNK_SIZE)
                if sample_pos.x >= 0
                    && sample_pos.x < crate::CHUNK_SIZE
                    && sample_pos.y >= 0
                    && sample_pos.y < crate::CHUNK_SIZE
                    && sample_pos.z >= 0
                    && sample_pos.z < crate::CHUNK_SIZE
                {
                    total_volume += water.volume_at(&sample_pos);
                }
            }
        }
    }

    total_volume
}

/// Extended local volume calculation that can sample across chunk boundaries.
///
/// This is more accurate for cells near chunk edges but requires access to
/// neighboring chunks. For efficiency, consider caching results per chunk.
///
/// # Arguments
/// * `get_volume` - Callback to get water volume at any global position
/// * `global_pos` - Global position of the surface cell
/// * `config` - Configuration for sampling
///
/// # Returns
/// Total volume in the sampled region
pub fn calculate_local_volume_global<F>(
    get_volume: F,
    global_pos: &IVec3,
    config: &WaveScaleConfig,
) -> f32
where
    F: Fn(&IVec3) -> f32,
{
    let mut total_volume = 0.0;
    let r = config.sample_radius_xz;
    let depth = config.sample_depth;

    for dx in -r..=r {
        for dz in -r..=r {
            for dy in -depth..=0 {
                let sample_pos =
                    IVec3::new(global_pos.x + dx, global_pos.y + dy, global_pos.z + dz);
                total_volume += get_volume(&sample_pos);
            }
        }
    }

    total_volume
}

// ============================================================================
// Wave Scale Mapping
// ============================================================================

/// Maps local water volume to a wave scale factor.
///
/// Uses the configuration's thresholds and scales to interpolate between
/// discrete wave scale levels. The result is a smooth curve from min_scale
/// to max_scale as volume increases.
///
/// # Arguments
/// * `local_volume` - Total water volume in the sampled region
/// * `config` - Configuration with thresholds and scales
///
/// # Returns
/// Wave scale factor (0.0 to 1.0, clamped to config min/max)
pub fn volume_to_wave_scale(local_volume: f32, config: &WaveScaleConfig) -> f32 {
    let thresholds = &config.volume_thresholds;
    let scales = &config.wave_scales;

    // Handle edge cases
    if local_volume <= thresholds[0] {
        // Below puddle threshold - interpolate from min_scale to puddle scale
        let t = local_volume / thresholds[0];
        return lerp(config.min_scale, scales[0], smoothstep(t));
    }

    // Find which bracket we're in and interpolate
    for i in 0..3 {
        if local_volume < thresholds[i + 1] {
            let t = (local_volume - thresholds[i]) / (thresholds[i + 1] - thresholds[i]);
            return lerp(scales[i], scales[i + 1], smoothstep(t));
        }
    }

    // Above ocean threshold - full scale
    config.max_scale
}

/// Convenience function to calculate wave scale directly from water storage.
///
/// # Arguments
/// * `water` - Chunk water storage
/// * `local_pos` - Position to calculate wave scale for
/// * `config` - Wave scale configuration
///
/// # Returns
/// Wave scale factor (0.0 to 1.0)
pub fn calculate_wave_scale(
    water: &ChunkWaterStorage,
    local_pos: &IVec3,
    config: &WaveScaleConfig,
) -> f32 {
    let volume = calculate_local_volume(water, local_pos, config);
    volume_to_wave_scale(volume, config)
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Linear interpolation between two values.
#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Smoothstep function for smooth interpolation.
/// Maps t from [0,1] to a smooth curve with zero derivatives at endpoints.
#[inline]
fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_to_wave_scale_thresholds() {
        let config = WaveScaleConfig::default();

        // Below puddle threshold
        let scale = volume_to_wave_scale(0.5, &config);
        assert!(scale < config.wave_scales[0]);
        assert!(scale > config.min_scale);

        // At puddle threshold
        let scale = volume_to_wave_scale(VOLUME_THRESHOLD_PUDDLE, &config);
        assert!((scale - config.wave_scales[0]).abs() < 0.01);

        // At pond threshold
        let scale = volume_to_wave_scale(VOLUME_THRESHOLD_POND, &config);
        assert!((scale - config.wave_scales[1]).abs() < 0.01);

        // At ocean threshold
        let scale = volume_to_wave_scale(VOLUME_THRESHOLD_OCEAN, &config);
        assert!((scale - config.wave_scales[3]).abs() < 0.01);

        // Above ocean threshold
        let scale = volume_to_wave_scale(VOLUME_THRESHOLD_OCEAN * 2.0, &config);
        assert!((scale - config.max_scale).abs() < 0.001);
    }

    #[test]
    fn test_volume_to_wave_scale_monotonic() {
        let config = WaveScaleConfig::default();

        // Wave scale should increase monotonically with volume
        let mut prev_scale = 0.0;
        for i in 0..100 {
            let volume = i as f32;
            let scale = volume_to_wave_scale(volume, &config);
            assert!(
                scale >= prev_scale,
                "Wave scale should increase with volume: {} -> {}",
                prev_scale,
                scale
            );
            prev_scale = scale;
        }
    }

    #[test]
    fn test_smoothstep() {
        // Smoothstep at endpoints
        assert!((smoothstep(0.0) - 0.0).abs() < 0.001);
        assert!((smoothstep(1.0) - 1.0).abs() < 0.001);

        // Smoothstep at midpoint
        assert!((smoothstep(0.5) - 0.5).abs() < 0.001);

        // Clamping
        assert!((smoothstep(-1.0) - 0.0).abs() < 0.001);
        assert!((smoothstep(2.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_local_volume_empty() {
        let water = ChunkWaterStorage::new();
        let config = WaveScaleConfig::default();
        let pos = IVec3::new(8, 8, 8);

        let volume = calculate_local_volume(&water, &pos, &config);
        assert!((volume - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_local_volume_single_cell() {
        let mut water = ChunkWaterStorage::new();
        let config = WaveScaleConfig::default();
        let pos = IVec3::new(8, 8, 8);

        water.set_full(pos);

        let volume = calculate_local_volume(&water, &pos, &config);
        assert!((volume - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_local_volume_deep_column() {
        let mut water = ChunkWaterStorage::new();
        let config = WaveScaleConfig::default();
        let pos = IVec3::new(8, 8, 8);

        // Create a 3-deep water column
        water.set_full(IVec3::new(8, 8, 8));
        water.set_full(IVec3::new(8, 7, 8));
        water.set_full(IVec3::new(8, 6, 8));

        let volume = calculate_local_volume(&water, &pos, &config);
        // Should include all 3 cells
        assert!((volume - 3.0).abs() < 0.001);
    }
}

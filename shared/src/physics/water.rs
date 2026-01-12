//! Water physics integration for player movement.
//!
//! This module handles player-water interactions including:
//! - Buoyancy forces
//! - Water drag
//! - Swimming mechanics
//!
//! This module works with the volume-based water system in `shared::world::water`.
//! Water submersion is calculated based on water volumes, not just block presence.

use crate::players::Player;
use crate::world::{BlockId, WorldMap, FULL_WATER_HEIGHT};

/// Constants for water physics
pub mod constants {
    /// Buoyancy force multiplier (upward force when in water)
    pub const BUOYANCY_FORCE: f32 = 15.0;
    /// Water drag coefficient (reduces movement speed in water)
    pub const WATER_DRAG: f32 = 0.6;
    /// Water resistance to vertical movement
    pub const WATER_VERTICAL_DRAG: f32 = 0.8;
    /// Maximum swim speed multiplier
    pub const SWIM_SPEED: f32 = 0.7;
    /// Swimming upward boost multiplier when jump is pressed
    pub const SWIM_JUMP_BOOST: f32 = 0.5;
    /// Minimum water submersion to enable swimming boost
    pub const SWIM_BOOST_THRESHOLD: f32 = 0.3;
    /// Maximum world height for water search
    pub const MAX_WATER_SEARCH_HEIGHT: i32 = 256;
}

/// Calculate how submerged a player is in water.
/// Returns a value from 0.0 (not in water) to 1.0 (fully submerged).
///
/// This function supports both the new volume-based water system (via WaterWorldMap)
/// and the legacy block-based water detection for backward compatibility.
pub fn calculate_water_submersion<T: WorldMap>(player: &Player, world_map: &T) -> f32 {
    let player_bottom = player.position.y - player.height / 2.0;
    let player_top = player.position.y + player.height / 2.0;

    // Sample water level at multiple points around the player
    let sample_positions = [
        (player.position.x as i32, player.position.z as i32),
        (
            (player.position.x + player.width * 0.4) as i32,
            player.position.z as i32,
        ),
        (
            (player.position.x - player.width * 0.4) as i32,
            player.position.z as i32,
        ),
        (
            player.position.x as i32,
            (player.position.z + player.width * 0.4) as i32,
        ),
        (
            player.position.x as i32,
            (player.position.z - player.width * 0.4) as i32,
        ),
    ];

    let mut max_submersion: f32 = 0.0;

    for (sample_x, sample_z) in &sample_positions {
        // Find the highest water surface at this position
        let water_height = find_water_surface_height(world_map, *sample_x, *sample_z);

        if water_height > player_bottom {
            let submersion = if water_height >= player_top {
                1.0
            } else {
                (water_height - player_bottom) / player.height
            };
            max_submersion = max_submersion.max(submersion);
        }
    }

    max_submersion.clamp(0.0, 1.0)
}

/// Find the water surface height at a given XZ position.
///
/// This function checks for water volumes first (new system), then falls back
/// to block-based detection (legacy BlockId::Water blocks).
fn find_water_surface_height<T: WorldMap>(world_map: &T, x: i32, z: i32) -> f32 {
    // Search downward from maximum height for water
    for y in (0..constants::MAX_WATER_SEARCH_HEIGHT).rev() {
        let pos = bevy::math::IVec3::new(x, y, z);

        // Check for water volume first (new volume-based system)
        if let Some(water_map) = world_map.as_water_world_map() {
            if let Some(volume) = water_map.get_water_volume(&pos) {
                if volume > 0.0 {
                    // Found water volume, calculate surface height
                    let surface_height = y as f32 + (volume * FULL_WATER_HEIGHT);
                    return surface_height;
                }
            }
        }

        // Fallback: Check for BlockId::Water (legacy compatibility)
        if let Some(block) = world_map.get_block_by_coordinates(&pos) {
            if block.id == BlockId::Water {
                // Found water block, search for surface
                for surface_y in y..constants::MAX_WATER_SEARCH_HEIGHT {
                    let surface_pos = bevy::math::IVec3::new(x, surface_y, z);
                    if let Some(above_block) = world_map.get_block_by_coordinates(&surface_pos) {
                        if above_block.id != BlockId::Water {
                            // Return surface with default full water height
                            return surface_y as f32;
                        }
                    } else {
                        return surface_y as f32;
                    }
                }
                return y as f32 + FULL_WATER_HEIGHT;
            }
        }
    }
    0.0 // No water found
}

/// Apply water physics to player movement
pub fn apply_water_physics<T: WorldMap>(player: &mut Player, world_map: &T, delta: f32) {
    // Calculate water submersion
    let submersion = calculate_water_submersion(player, world_map);

    // Update player water state
    player.in_water = submersion > 0.1;
    player.water_submersion = submersion;

    if !player.in_water || player.is_flying {
        return;
    }

    // Apply buoyancy force (upward)
    let buoyancy = constants::BUOYANCY_FORCE * submersion * delta;
    player.velocity.y += buoyancy;

    // Apply water drag to all velocities
    let drag_factor = 1.0 - (constants::WATER_DRAG * submersion * delta);
    player.velocity.x *= drag_factor;
    player.velocity.z *= drag_factor;
    player.velocity.y *= 1.0 - (constants::WATER_VERTICAL_DRAG * submersion * delta);

    // Limit vertical velocity in water
    const MAX_WATER_VELOCITY: f32 = 5.0;
    player.velocity.y = player
        .velocity
        .y
        .clamp(-MAX_WATER_VELOCITY, MAX_WATER_VELOCITY);
}

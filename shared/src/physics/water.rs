//! Water physics integration for player movement.
//!
//! This module handles player-water interactions including:
//! - Buoyancy forces
//! - Water drag
//! - Swimming mechanics
//!
//! Note: Wave motion is handled by bevy_water on the client side.
//! This module focuses on gameplay physics (buoyancy, drag, swimming).

use crate::players::Player;
use crate::world::{BlockId, WorldMap};

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

/// Calculate how submerged a player is in water
/// Returns a value from 0.0 (not in water) to 1.0 (fully submerged)
pub fn calculate_water_submersion(player: &Player, world_map: &impl WorldMap) -> f32 {
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
        // Find the highest water block at this position
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

/// Find the water surface height at a given XZ position
fn find_water_surface_height(world_map: &impl WorldMap, x: i32, z: i32) -> f32 {
    // Search downward from maximum height
    for y in (0..constants::MAX_WATER_SEARCH_HEIGHT).rev() {
        if let Some(block) = world_map.get_block_by_coordinates(&bevy::math::IVec3::new(x, y, z)) {
            if block.id == BlockId::Water {
                // Found water, now find the surface (first air block above water)
                for surface_y in y..constants::MAX_WATER_SEARCH_HEIGHT {
                    if let Some(above_block) =
                        world_map.get_block_by_coordinates(&bevy::math::IVec3::new(x, surface_y, z))
                    {
                        if above_block.id != BlockId::Water {
                            return surface_y as f32;
                        }
                    } else {
                        return surface_y as f32;
                    }
                }
                return y as f32 + 1.0; // Default to one block above water
            }
        }
    }
    0.0 // No water found
}

/// Apply water physics to player movement
pub fn apply_water_physics(player: &mut Player, world_map: &impl WorldMap, delta: f32) {
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

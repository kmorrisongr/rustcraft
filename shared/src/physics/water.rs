//! Water physics integration for player movement.
//!
//! This module handles player-water interactions including:
//! - Buoyancy forces
//! - Water drag
//! - Wave motion transfer
//! - Swimming mechanics

use bevy::math::{Vec2, Vec3};
use crate::players::Player;
use crate::water_physics::GerstnerWaveSystem;
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
    /// Wave push strength (how much waves move the player)
    pub const WAVE_PUSH_STRENGTH: f32 = 2.0;
    /// Swimming upward boost multiplier when jump is pressed
    pub const SWIM_JUMP_BOOST: f32 = 0.5;
    /// Minimum water submersion to enable swimming boost
    pub const SWIM_BOOST_THRESHOLD: f32 = 0.3;
    /// Maximum world height for water search
    pub const MAX_WATER_SEARCH_HEIGHT: i32 = 256;
}

/// Check if a player is in water at their current position
pub fn check_player_in_water(player: &Player, world_map: &impl WorldMap) -> bool {
    // Check blocks at player's position (bottom, middle, and head level)
    let positions = [
        player.position,                           // Center
        player.position + Vec3::new(0.0, player.height * 0.5, 0.0),  // Head
        player.position - Vec3::new(0.0, player.height * 0.3, 0.0),  // Feet
    ];

    for pos in &positions {
        if let Some(block) = world_map.get_block_by_coordinates(&pos.as_ivec3()) {
            if block.id == BlockId::Water {
                return true;
            }
        }
    }

    false
}

/// Calculate how submerged a player is in water
/// Returns a value from 0.0 (not in water) to 1.0 (fully submerged)
pub fn calculate_water_submersion(
    player: &Player,
    world_map: &impl WorldMap,
    wave_system: Option<&GerstnerWaveSystem>,
    time: f32,
) -> f32 {
    let player_bottom = player.position.y - player.height / 2.0;
    let player_top = player.position.y + player.height / 2.0;
    
    // Sample water level at multiple points around the player
    let sample_positions = [
        Vec2::new(player.position.x, player.position.z),
        Vec2::new(player.position.x + player.width * 0.4, player.position.z),
        Vec2::new(player.position.x - player.width * 0.4, player.position.z),
        Vec2::new(player.position.x, player.position.z + player.width * 0.4),
        Vec2::new(player.position.x, player.position.z - player.width * 0.4),
    ];

    let mut max_submersion: f32 = 0.0;

    for sample_pos in &sample_positions {
        // Get water surface height at this position
        let water_height = if let Some(waves) = wave_system {
            // Use Gerstner wave system for dynamic water surface
            waves.get_surface_height(*sample_pos, time)
        } else {
            // Fallback: find the highest water block
            find_water_surface_height(world_map, sample_pos.x as i32, sample_pos.y as i32)
        };

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

/// Find the water surface height at a given XZ position (fallback for when no wave system)
fn find_water_surface_height(world_map: &impl WorldMap, x: i32, z: i32) -> f32 {
    // Search downward from maximum height
    for y in (0..constants::MAX_WATER_SEARCH_HEIGHT).rev() {
        if let Some(block) = world_map.get_block_by_coordinates(&bevy::math::IVec3::new(x, y, z)) {
            if block.id == BlockId::Water {
                // Found water, now find the surface (first air block above water)
                for surface_y in y..constants::MAX_WATER_SEARCH_HEIGHT {
                    if let Some(above_block) = world_map.get_block_by_coordinates(&bevy::math::IVec3::new(x, surface_y, z)) {
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
pub fn apply_water_physics(
    player: &mut Player,
    world_map: &impl WorldMap,
    wave_system: Option<&GerstnerWaveSystem>,
    time: f32,
    delta: f32,
) {
    // Calculate water submersion
    let submersion = calculate_water_submersion(player, world_map, wave_system, time);
    
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

    // Apply wave motion to player
    if let Some(waves) = wave_system {
        let player_pos_2d = Vec2::new(player.position.x, player.position.z);
        let wave_velocity = waves.get_flow_velocity(player_pos_2d, time);
        
        // Push player with waves
        let wave_push = wave_velocity * constants::WAVE_PUSH_STRENGTH * submersion * delta;
        player.velocity.x += wave_push.x;
        player.velocity.z += wave_push.y;
    }

    // Limit vertical velocity in water
    const MAX_WATER_VELOCITY: f32 = 5.0;
    player.velocity.y = player.velocity.y.clamp(-MAX_WATER_VELOCITY, MAX_WATER_VELOCITY);
}

/// Check if player should be able to "stand" on water surface
/// This prevents player from sinking through calm water surfaces
pub fn check_water_surface_support(
    player: &Player,
    world_map: &impl WorldMap,
    wave_system: Option<&GerstnerWaveSystem>,
    time: f32,
) -> bool {
    if player.is_flying {
        return false;
    }

    let player_bottom = player.position.y - player.height / 2.0;
    let player_pos_2d = Vec2::new(player.position.x, player.position.z);

    let water_surface = if let Some(waves) = wave_system {
        waves.get_surface_height(player_pos_2d, time)
    } else {
        find_water_surface_height(world_map, player.position.x as i32, player.position.z as i32)
    };

    // Check if player is just above water surface and moving downward
    let distance_above_water = player_bottom - water_surface;
    distance_above_water < 0.5 && distance_above_water > -0.1 && player.velocity.y <= 0.0
}

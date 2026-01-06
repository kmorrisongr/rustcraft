use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    physics::{apply_gravity, resolve_vertical_movement, try_move, PhysicsBody},
    players::constants::{FLY_SPEED_MULTIPLIER, GRAVITY, JUMP_VELOCITY, SPEED},
    world::{world_position_to_chunk_position, WorldMap},
};
use bevy::prelude::*;

use super::Player;

/// Update the cached gravity_enabled state.
/// Only caches when gravity is enabled - if disabled, we recheck each frame
/// until chunks load to avoid the player being stuck floating.
fn update_gravity_state(player: &mut Player, world_map: &impl WorldMap) {
    let current_chunk = world_position_to_chunk_position(player.position);

    // If gravity is already enabled and we're in the same chunk, no need to recheck
    if player.gravity_enabled && player.last_gravity_check_chunk == Some(current_chunk) {
        return;
    }

    // Recompute: either chunk changed, or gravity was disabled (chunks may have loaded)
    let chunk_below = current_chunk - IVec3::Y;
    player.gravity_enabled =
        world_map.has_chunk(&current_chunk) && world_map.has_chunk(&chunk_below);
    player.last_gravity_check_chunk = Some(current_chunk);
}

pub fn simulate_player_movement(
    player: &mut Player,
    world_map: &impl WorldMap,
    action: &PlayerFrameInput,
) {
    // let's check if the 9 chunks around the player are loaded
    let chunks = world_map.get_surrounding_chunks(player.position, 1);
    if chunks.len() < 9 {
        log::debug!("Not enough chunks loaded, skipping movement simulation");
        return;
    }

    let delta = action.delta_ms as f32 / 1000.0;

    let mut direction = Vec3::ZERO;

    if action.is_pressed(NetworkAction::ToggleFlyMode) {
        player.is_flying = !player.is_flying;
    }

    player.camera_transform = action.camera;

    let is_jumping = action.is_pressed(NetworkAction::JumpOrFlyUp);

    // Calculate movement directions relative to the camera
    let forward = player
        .camera_transform
        .forward()
        .xyz()
        .with_y(0.0)
        .normalize();

    let right = player
        .camera_transform
        .right()
        .xyz()
        .with_y(0.0)
        .normalize();

    // Adjust direction based on key presses
    if action.is_pressed(NetworkAction::MoveBackward) {
        direction -= forward;
    }
    if action.is_pressed(NetworkAction::MoveForward) {
        direction += forward;
    }
    if action.is_pressed(NetworkAction::MoveLeft) {
        direction -= right;
    }
    if action.is_pressed(NetworkAction::MoveRight) {
        direction += right;
    }

    // Normalize direction to prevent faster movement with diagonals
    if direction != Vec3::ZERO {
        direction = direction.normalize();
    }

    if action.is_pressed(NetworkAction::JumpOrFlyUp) {
        direction += Vec3::Y;
    }
    if action.is_pressed(NetworkAction::SneakOrFlyDown) {
        direction -= Vec3::Y;
    }

    let mut body = PhysicsBody::new(
        player.position,
        player.velocity,
        player.on_ground,
        Vec3::new(player.width, player.height, player.width),
    );

    // Update cached gravity state (only recomputes when chunk changes)
    update_gravity_state(player, world_map);

    // Handle jumping (if on the ground) and gravity, only if not flying
    if !player.is_flying {
        if body.on_ground && is_jumping {
            // Player can jump only when grounded
            body.velocity.y = JUMP_VELOCITY * delta;
            body.on_ground = false;
        } else if player.gravity_enabled {
            apply_gravity(&mut body, GRAVITY, delta);
        }
    } else {
        body.velocity.y = 0.0;
        body.on_ground = false;
    }

    let max_velocity = 0.9;

    resolve_vertical_movement(&mut body, world_map, max_velocity, !player.is_flying);

    let speed = if player.is_flying {
        SPEED * FLY_SPEED_MULTIPLIER
    } else {
        SPEED
    };
    let speed = speed * delta;
    let displacement = Vec3::new(direction.x * speed, 0.0, direction.z * speed);

    if player.is_flying {
        try_move(
            &mut body,
            world_map,
            displacement + Vec3::new(0.0, direction.y * speed, 0.0),
            false,
        );
    } else {
        try_move(
            &mut body,
            world_map,
            Vec3::new(displacement.x, 0.0, 0.0),
            true,
        );
        try_move(
            &mut body,
            world_map,
            Vec3::new(0.0, 0.0, displacement.z),
            true,
        );
    }

    player.position = body.position;
    player.velocity = body.velocity;
    player.on_ground = body.on_ground;
}

trait IsPressed {
    fn is_pressed(&self, action: NetworkAction) -> bool;
}

impl IsPressed for PlayerFrameInput {
    fn is_pressed(&self, action: NetworkAction) -> bool {
        self.inputs.contains(&action)
    }
}

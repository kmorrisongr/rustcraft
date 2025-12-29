use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    physics::{apply_gravity, resolve_vertical_movement, try_move, PhysicsBody},
    players::constants::{FLY_SPEED_MULTIPLIER, GRAVITY, JUMP_VELOCITY, SPEED},
    world::WorldMap,
};
use bevy::prelude::*;

use super::Player;

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

    // Handle jumping (if on the ground) and gravity, only if not flying
    if !player.is_flying {
        if body.on_ground && is_jumping {
            // Player can jump only when grounded
            body.velocity.y = JUMP_VELOCITY * delta;
            body.on_ground = false;
        } else {
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

    // If the player is below the world, reset their position
    const FALL_LIMIT: f32 = -50.0;
    if body.position.y < FALL_LIMIT {
        body.position = Vec3::new(0.0, 100.0, 0.0);
        body.velocity.y = 0.0;
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

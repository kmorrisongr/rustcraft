//! Rapier-based player movement system.
//!
//! This module provides player movement using Rapier physics with
//! voxel world collision detection.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    physics::{
        constants::{FLY_SPEED_MULTIPLIER, GRAVITY, JUMP_VELOCITY, PLAYER_SPEED},
        RustcraftPhysicsBody,
    },
    players::Player,
    world::{world_position_to_chunk_position, WorldMap},
};

/// Recompute gravity_enabled based on whether required chunks are loaded.
fn compute_gravity_enabled(player: &Player, world_map: &impl WorldMap) -> bool {
    let current_chunk = world_position_to_chunk_position(player.position);
    let chunk_below = current_chunk - IVec3::Y;
    let chunk_above = current_chunk + IVec3::Y;
    world_map.has_chunk(&current_chunk)
        && world_map.has_chunk(&chunk_below)
        && world_map.has_chunk(&chunk_above)
}

/// Check if gravity state needs to be updated and update it if so.
fn maybe_update_gravity_state(player: &mut Player, world_map: &impl WorldMap) {
    let current_chunk = world_position_to_chunk_position(player.position);
    let chunk_changed = player.last_gravity_check_chunk != Some(current_chunk);

    if chunk_changed {
        player.last_gravity_check_chunk = Some(current_chunk);
        player.gravity_enabled = compute_gravity_enabled(player, world_map);
    } else if !player.gravity_enabled {
        player.gravity_enabled = compute_gravity_enabled(player, world_map);
    }
}

/// Simulate player movement using Rapier-compatible physics.
///
/// This function processes player input and updates the player's position,
/// velocity, and state using physics simulation compatible with Rapier.
///
/// # Arguments
/// * `player` - The player to update
/// * `world_map` - The world map for collision detection
/// * `action` - The player's input for this frame
/// * `rapier_context` - Optional Rapier context for advanced physics queries
pub fn simulate_player_movement_rapier<W: WorldMap>(
    player: &mut Player,
    world_map: &W,
    action: &PlayerFrameInput,
) {
    // Check if enough chunks are loaded
    let chunks = world_map.get_surrounding_chunks(player.position, 1);
    if chunks.len() < 9 {
        log::debug!("Not enough chunks loaded, skipping movement simulation");
        return;
    }

    let delta = action.delta_ms as f32 / 1000.0;
    if delta <= 0.0 {
        return;
    }

    // Handle fly mode toggle
    if action.inputs.contains(&NetworkAction::ToggleFlyMode) {
        player.is_flying = !player.is_flying;
        player.velocity = Vec3::ZERO;
    }

    player.camera_transform = action.camera;

    // Calculate movement direction
    let mut direction = calculate_movement_direction(player, action);

    // Update gravity state
    maybe_update_gravity_state(player, world_map);

    // Apply physics based on flying state
    if player.is_flying {
        apply_flying_physics(player, &direction, delta);
    } else {
        apply_ground_physics(player, world_map, &mut direction, action, delta);
    }

    // Apply movement with collision
    apply_movement_with_collision(player, world_map, direction, delta);

    // Safety net
    apply_safety_net(player);
}

/// Calculate the movement direction based on player input.
fn calculate_movement_direction(player: &Player, action: &PlayerFrameInput) -> Vec3 {
    let forward = player
        .camera_transform
        .forward()
        .xyz()
        .with_y(0.0)
        .normalize_or_zero();

    let right = player
        .camera_transform
        .right()
        .xyz()
        .with_y(0.0)
        .normalize_or_zero();

    let mut direction = Vec3::ZERO;

    if action.inputs.contains(&NetworkAction::MoveBackward) {
        direction -= forward;
    }
    if action.inputs.contains(&NetworkAction::MoveForward) {
        direction += forward;
    }
    if action.inputs.contains(&NetworkAction::MoveLeft) {
        direction -= right;
    }
    if action.inputs.contains(&NetworkAction::MoveRight) {
        direction += right;
    }

    // Normalize horizontal direction to prevent faster diagonal movement
    if direction != Vec3::ZERO {
        direction = direction.normalize();
    }

    // Add vertical movement for flying
    if action.inputs.contains(&NetworkAction::JumpOrFlyUp) {
        direction.y += 1.0;
    }
    if action.inputs.contains(&NetworkAction::SneakOrFlyDown) {
        direction.y -= 1.0;
    }

    direction
}

/// Apply flying physics (no gravity, direct position control).
fn apply_flying_physics(player: &mut Player, direction: &Vec3, delta: f32) {
    let speed = PLAYER_SPEED * FLY_SPEED_MULTIPLIER * delta;
    player.velocity = *direction * speed / delta; // Store velocity for smooth movement
    player.on_ground = false;
}

/// Apply ground physics (gravity, jumping, ground detection).
fn apply_ground_physics<W: WorldMap>(
    player: &mut Player,
    _world_map: &W,
    direction: &mut Vec3,
    action: &PlayerFrameInput,
    delta: f32,
) {
    let is_jumping = action.inputs.contains(&NetworkAction::JumpOrFlyUp);

    // Apply gravity if enabled
    if player.gravity_enabled && !player.on_ground {
        player.velocity.y += GRAVITY * delta;
    }

    // Handle jumping
    if player.on_ground && is_jumping {
        player.velocity.y = JUMP_VELOCITY;
        player.on_ground = false;
    }

    // Clamp vertical velocity
    const MAX_FALL_SPEED: f32 = 50.0;
    player.velocity.y = player.velocity.y.clamp(-MAX_FALL_SPEED, MAX_FALL_SPEED);

    // Remove vertical component from direction when not flying
    direction.y = 0.0;
}

/// Apply movement with voxel collision detection.
fn apply_movement_with_collision<W: WorldMap>(
    player: &mut Player,
    world_map: &W,
    direction: Vec3,
    delta: f32,
) {
    use bevy::math::bounding::Aabb3d;

    let speed = if player.is_flying {
        PLAYER_SPEED * FLY_SPEED_MULTIPLIER
    } else {
        PLAYER_SPEED
    };

    let horizontal_displacement = Vec3::new(
        direction.x * speed * delta,
        0.0,
        direction.z * speed * delta,
    );
    let vertical_displacement = Vec3::new(0.0, player.velocity.y * delta, 0.0);

    let half_extents = Vec3::new(player.width / 2.0, player.height / 2.0, player.width / 2.0);

    // Try horizontal movement (X axis)
    let candidate_x = player.position + Vec3::new(horizontal_displacement.x, 0.0, 0.0);
    if !world_map.check_collision_box(&Aabb3d::new(candidate_x, half_extents)) {
        player.position.x = candidate_x.x;
    }

    // Try horizontal movement (Z axis)
    let candidate_z = player.position + Vec3::new(0.0, 0.0, horizontal_displacement.z);
    if !world_map.check_collision_box(&Aabb3d::new(candidate_z, half_extents)) {
        player.position.z = candidate_z.z;
    }

    // Try vertical movement
    if player.is_flying {
        // In fly mode, use direction for vertical movement
        let fly_vertical = Vec3::new(0.0, direction.y * speed * delta, 0.0);
        let candidate_y = player.position + fly_vertical;
        if !world_map.check_collision_box(&Aabb3d::new(candidate_y, half_extents)) {
            player.position.y = candidate_y.y;
        }
    } else {
        // Normal gravity-based vertical movement
        let candidate_y = player.position + vertical_displacement;
        if world_map.check_collision_box(&Aabb3d::new(candidate_y, half_extents)) {
            // Collision detected
            if player.velocity.y <= 0.0 {
                player.on_ground = true;
            }
            player.velocity.y = 0.0;
        } else {
            player.position.y = candidate_y.y;
            player.on_ground = false;
        }
    }
}

/// Apply safety net to prevent falling through the world.
fn apply_safety_net(player: &mut Player) {
    const FALL_RESET_Y: f32 = -100.0;
    if player.position.y < FALL_RESET_Y {
        log::warn!(
            "Player {:?} fell below safety threshold (y = {}). Resetting position.",
            player.id,
            player.position.y
        );
        player.position.y = 64.0; // Reset to reasonable height
        player.velocity = Vec3::ZERO;
    }
}

/// Component for entities that use the Rapier movement controller.
#[derive(Component, Default)]
pub struct RapierMovementController {
    /// Desired movement direction (normalized)
    pub movement_direction: Vec3,
    /// Whether the entity wants to jump
    pub wants_jump: bool,
    /// Whether the entity is flying
    pub is_flying: bool,
    /// Custom movement speed (overrides default)
    pub speed_override: Option<f32>,
}

impl RapierMovementController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_direction(mut self, direction: Vec3) -> Self {
        self.movement_direction = direction.normalize_or_zero();
        self
    }

    pub fn with_jump(mut self, wants_jump: bool) -> Self {
        self.wants_jump = wants_jump;
        self
    }

    pub fn with_flying(mut self, is_flying: bool) -> Self {
        self.is_flying = is_flying;
        self
    }

    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed_override = Some(speed);
        self
    }
}

/// System that processes Rapier movement controllers.
///
/// This system reads `RapierMovementController` components and applies
/// movement using Rapier's kinematic character controller.
pub fn rapier_movement_system(
    time: Res<Time>,
    mut query: Query<(
        &mut Transform,
        &mut Velocity,
        &RapierMovementController,
        &mut RustcraftPhysicsBody,
        &Collider,
    )>,
    rapier_context: Query<(
        &RapierContextColliders,
        &RapierRigidBodySet,
        &RapierQueryPipeline,
    )>,
) {
    let delta = time.delta_secs();
    if delta <= 0.0 {
        return;
    }

    let Ok((colliders, rigidbody_set, query_pipeline)) = rapier_context.single() else {
        return;
    };

    for (mut transform, mut velocity, controller, mut physics_body, collider) in query.iter_mut() {
        let speed = controller.speed_override.unwrap_or(PLAYER_SPEED);

        if controller.is_flying {
            // Flying: direct position control
            let movement = controller.movement_direction * speed * delta;
            transform.translation += movement;
            velocity.linvel = controller.movement_direction * speed;
            physics_body.on_ground = false;
        } else {
            // Ground movement with gravity
            if physics_body.gravity_enabled && !physics_body.on_ground {
                velocity.linvel.y += GRAVITY * delta;
            }

            // Jump
            if physics_body.on_ground && controller.wants_jump {
                velocity.linvel.y = JUMP_VELOCITY;
                physics_body.on_ground = false;
            }

            // Horizontal movement
            let horizontal_dir = Vec3::new(
                controller.movement_direction.x,
                0.0,
                controller.movement_direction.z,
            )
            .normalize_or_zero();

            velocity.linvel.x = horizontal_dir.x * speed;
            velocity.linvel.z = horizontal_dir.z * speed;

            // Clamp fall speed
            velocity.linvel.y = velocity.linvel.y.clamp(-50.0, 50.0);

            // Apply velocity to position
            let movement = velocity.linvel * delta;
            transform.translation += movement;

            // Ground check using shape cast
            let ground_check = query_pipeline.cast_shape(
                colliders,
                rigidbody_set,
                transform.translation,
                Quat::IDENTITY,
                Vec3::NEG_Y,
                collider,
                ShapeCastOptions {
                    max_time_of_impact: 0.1,
                    ..default()
                },
                QueryFilter::default(),
            );

            physics_body.on_ground = ground_check.is_some();
        }
    }
}

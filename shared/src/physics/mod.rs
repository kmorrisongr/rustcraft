pub mod movement;
pub mod rapier;

use bevy::math::{bounding::Aabb3d, Vec3};

use crate::world::WorldMap;

// Re-export Rapier integration
pub use movement::{
    rapier_movement_system, simulate_player_movement_rapier, RapierMovementController,
};
pub use rapier::*;

/// Physics representation shared by movable entities (players, mobs, etc.).
/// `dimensions` is the full size of the entity's hitbox (width, height, depth).
///
/// NOTE: This is the legacy physics body struct. New code should use
/// `RustcraftPhysicsBody` with Rapier components instead.
#[derive(Clone, Copy, Debug)]
pub struct PhysicsBody {
    /// Current world position of the entity (center of the hitbox).
    pub position: Vec3,
    /// Current velocity applied to the body.
    pub velocity: Vec3,
    /// Whether the body is resting on a surface.
    pub on_ground: bool,
    /// Full dimensions of the hitbox (width, height, depth).
    pub dimensions: Vec3,
}

impl PhysicsBody {
    /// Create a new physics body with explicit state and hitbox size.
    pub fn new(position: Vec3, velocity: Vec3, on_ground: bool, dimensions: Vec3) -> Self {
        Self {
            position,
            velocity,
            on_ground,
            dimensions,
        }
    }

    fn aabb_at(&self, position: Vec3) -> Aabb3d {
        Aabb3d::new(position, self.dimensions / 2.0)
    }
}

/// Apply gravity to the body if it is not grounded.
pub fn apply_gravity(body: &mut PhysicsBody, gravity: f32, delta: f32) {
    if !body.on_ground {
        body.velocity.y += gravity * delta;
    }
}

/// Resolve vertical movement, clamping vertical speed and handling collisions when `collide` is true.
/// When `collide` is false, the body moves freely without collision checks and is marked airborne.
pub fn resolve_vertical_movement(
    body: &mut PhysicsBody,
    world_map: &impl WorldMap,
    max_velocity: f32,
    collide: bool,
) {
    if body.velocity.y < -max_velocity {
        body.velocity.y = -max_velocity;
    } else if body.velocity.y > max_velocity {
        body.velocity.y = max_velocity;
    }

    let new_y = body.position.y + body.velocity.y;

    if !collide {
        body.position.y = new_y;
        body.on_ground = false;
        return;
    }

    let candidate = body.position.with_y(new_y);
    if world_map.check_collision_box(&body.aabb_at(candidate)) {
        if body.velocity.y <= 0.0 {
            body.on_ground = true;
        } else {
            body.on_ground = false;
        }
        body.velocity.y = 0.0;
    } else {
        body.position.y = new_y;
        body.on_ground = false;
    }
}

/// Attempt to move the body by `displacement`.
/// If `collide` is false, the body moves freely and returns `false`.
/// If `collide` is true, performs collision checks; returns `true` when the
/// movement is blocked by a collision (position is not updated) and `false` otherwise.
pub fn try_move(
    body: &mut PhysicsBody,
    world_map: &impl WorldMap,
    displacement: Vec3,
    collide: bool,
) -> bool {
    if !collide {
        body.position += displacement;
        return false;
    }

    let candidate = body.position + displacement;
    if world_map.check_collision_box(&body.aabb_at(candidate)) {
        true
    } else {
        body.position = candidate;
        false
    }
}

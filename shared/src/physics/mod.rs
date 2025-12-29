use bevy::math::{bounding::Aabb3d, Vec3};

use crate::world::WorldMap;

#[derive(Clone, Copy, Debug)]
pub struct PhysicsBody {
    pub position: Vec3,
    pub velocity: Vec3,
    pub on_ground: bool,
    pub dimensions: Vec3,
}

impl PhysicsBody {
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

pub fn apply_gravity(body: &mut PhysicsBody, gravity: f32, delta: f32) {
    if !body.on_ground {
        body.velocity.y += gravity * delta;
    }
}

pub fn resolve_vertical_movement(
    body: &mut PhysicsBody,
    world_map: &impl WorldMap,
    max_velocity: f32,
    collide: bool,
) {
    if body.velocity.y > max_velocity {
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
        body.on_ground = true;
        body.velocity.y = 0.0;
    } else {
        body.position.y = new_y;
        body.on_ground = false;
    }
}

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

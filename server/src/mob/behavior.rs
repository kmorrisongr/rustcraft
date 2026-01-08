use bevy::{
    math::{bounding::Aabb3d, ops::atan2, Quat, Vec3},
    time::{Fixed, Time},
};
use bevy_ecs::system::{Res, ResMut};
use log::error as log_error;
use shared::{
    physics::constants::{GRAVITY, JUMP_VELOCITY, PLAYER_SPEED, TERMINAL_VELOCITY},
    world::{MobAction, MobTarget, ServerWorldMap, WorldMap},
};

/// Mob movement speed as a fraction of player speed
const MOB_WALK_SPEED_MULTIPLIER: f32 = 0.7;
/// Mob flee speed as a fraction of player speed
const MOB_FLEE_SPEED_MULTIPLIER: f32 = 0.5;

/// Calculates half extents from mob dimensions for AABB collision detection.
#[inline]
fn calculate_half_extents(dimensions: Vec3) -> Vec3 {
    Vec3::new(dimensions.x / 2.0, dimensions.y / 2.0, dimensions.z / 2.0)
}

/// Applies Rapier-style physics to a mob, including gravity and velocity clamping.
///
/// # Arguments
/// * `position` - Current mob position
/// * `velocity` - Current mob velocity (will be modified)
/// * `on_ground` - Whether the mob is on the ground (will be modified)
/// * `world_map` - World map for collision detection
/// * `dimensions` - Mob dimensions (width, height, depth)
/// * `delta` - Time step in seconds
fn apply_mob_physics(
    position: &mut Vec3,
    velocity: &mut Vec3,
    on_ground: &mut bool,
    world_map: &ServerWorldMap,
    dimensions: Vec3,
    delta: f32,
) {
    // Apply gravity if not on ground
    if !*on_ground {
        velocity.y += GRAVITY * delta;
    }

    // Clamp vertical velocity to terminal velocity
    velocity.y = velocity.y.clamp(-TERMINAL_VELOCITY, TERMINAL_VELOCITY);

    // Calculate vertical displacement
    let vertical_displacement = Vec3::new(0.0, velocity.y * delta, 0.0);
    let half_extents = calculate_half_extents(dimensions);

    // Try vertical movement
    let candidate_y = *position + vertical_displacement;
    if world_map
        .chunks
        .check_collision_box(&Aabb3d::new(candidate_y, half_extents))
    {
        // Collision detected
        if velocity.y <= 0.0 {
            *on_ground = true;
        }
        velocity.y = 0.0;
    } else {
        position.y = candidate_y.y;
        *on_ground = false;
    }
}

/// Applies horizontal movement with collision detection and obstacle avoidance.
///
/// Uses per-axis collision detection similar to Rapier's approach.
/// If blocked, the mob will attempt to jump over the obstacle.
///
/// # Arguments
/// * `position` - Current mob position (will be modified)
/// * `velocity` - Current mob velocity (used for jump logic)
/// * `on_ground` - Whether the mob is on the ground
/// * `world_map` - World map for collision detection
/// * `dimensions` - Mob dimensions (width, height, depth)
/// * `direction` - Normalized movement direction
/// * `speed` - Movement speed
fn apply_horizontal_movement(
    position: &mut Vec3,
    velocity: &mut Vec3,
    on_ground: bool,
    world_map: &ServerWorldMap,
    dimensions: Vec3,
    direction: Vec3,
    speed: f32,
    delta: f32,
) {
    let horizontal_displacement = Vec3::new(
        direction.x * speed * delta,
        0.0,
        direction.z * speed * delta,
    );
    let half_extents = calculate_half_extents(dimensions);

    // Try X-axis movement
    let candidate_x = *position + Vec3::new(horizontal_displacement.x, 0.0, 0.0);
    let mut blocked = false;
    if !world_map
        .chunks
        .check_collision_box(&Aabb3d::new(candidate_x, half_extents))
    {
        position.x = candidate_x.x;
    } else {
        blocked = true;
    }

    // Try Z-axis movement
    let candidate_z = *position + Vec3::new(0.0, 0.0, horizontal_displacement.z);
    if !world_map
        .chunks
        .check_collision_box(&Aabb3d::new(candidate_z, half_extents))
    {
        position.z = candidate_z.z;
    } else {
        blocked = true;
    }

    // If blocked and on ground, try jumping to clear obstacle
    if blocked && on_ground {
        velocity.y = JUMP_VELOCITY;
    }
}

pub fn mob_behavior_system(mut world_map: ResMut<ServerWorldMap>, delta: Res<Time<Fixed>>) {
    let mut mobs = world_map.mobs.clone();

    for (_mob_id, mob) in mobs.iter_mut() {
        // Validate mob state
        if (mob.position.x.is_nan() || mob.position.y.is_nan() || mob.position.z.is_nan())
            || (mob.velocity.x.is_nan() || mob.velocity.y.is_nan() || mob.velocity.z.is_nan())
        {
            log_error!("Mob has NaN position or velocity, skipping");
            continue;
        }

        let target = match mob.target {
            MobTarget::Position(pos) => pos,
            MobTarget::None => continue,
            MobTarget::Player(id) => {
                if let Some(player) = world_map.players.get(&id) {
                    player.position
                } else {
                    mob.position
                }
            }
            MobTarget::Mob(id) => {
                if let Some(target_mob) = world_map.mobs.get(&id) {
                    target_mob.position
                } else {
                    continue;
                }
            }
        };

        let delta = delta.delta_secs();
        if delta <= 0.0 {
            continue;
        }

        let dimensions = Vec3::new(mob.width, mob.height, mob.depth);

        // Apply physics (gravity and vertical movement)
        apply_mob_physics(
            &mut mob.position,
            &mut mob.velocity,
            &mut mob.on_ground,
            &world_map,
            dimensions,
            delta,
        );

        // Calculate direction to target
        let dir = (target - mob.position).normalize_or_zero();

        match mob.action {
            MobAction::Walk | MobAction::Attack => {
                // Use a slower speed for mobs
                let speed = PLAYER_SPEED * MOB_WALK_SPEED_MULTIPLIER;

                // Apply horizontal movement with obstacle avoidance
                apply_horizontal_movement(
                    &mut mob.position,
                    &mut mob.velocity,
                    mob.on_ground,
                    &world_map,
                    dimensions,
                    dir,
                    speed,
                    delta,
                );

                mob.rotation = Quat::from_rotation_y(atan2(dir.x, dir.z));

                // If reached destination, start fleeing
                if mob.position.distance(target) < 0.5 {
                    mob.action = MobAction::Flee;
                }
            }
            MobAction::Flee => {
                if mob.position.distance(target) < 15.0 {
                    let flee_speed = PLAYER_SPEED * MOB_FLEE_SPEED_MULTIPLIER;
                    let flee_dir = -dir;

                    // Apply horizontal movement while fleeing
                    apply_horizontal_movement(
                        &mut mob.position,
                        &mut mob.velocity,
                        mob.on_ground,
                        &world_map,
                        dimensions,
                        flee_dir,
                        flee_speed,
                        delta,
                    );

                    mob.rotation = Quat::from_rotation_y(atan2(flee_dir.x, flee_dir.z));
                }
            }
            _ => {}
        }
    }

    world_map.mobs = mobs;
}

use bevy::{
    math::{ops::atan2, Quat, Vec3},
    time::{Fixed, Time},
};
use bevy_ecs::system::{Res, ResMut};
use shared::{
    physics::{apply_gravity, resolve_vertical_movement, try_move, PhysicsBody},
    players::constants::{GRAVITY, JUMP_VELOCITY, SPEED},
    world::{MobAction, MobTarget, ServerChunkWorldMap, ServerWorldMap},
};

/// Attempts to move a mob toward `displacement` while avoiding obstacles.
///
/// Strategy:
/// 1. Try the full (typically diagonal) displacement; if blocked, fall back.
/// 2. If grounded with prior horizontal velocity, jump to clear the obstacle.
/// 3. Otherwise, try per-axis moves (x first, then z) before performing a jump fallback.
///
/// - `body`: Mutable physics state to mutate with movement/jump outcomes.
/// - `chunks`: World chunk map used by `try_move` for collision checks.
/// - `displacement`: Intended horizontal step (already scaled by speed).
/// - `delta`: Fixed timestep seconds; used when adding jump velocity.
///
/// `try_move` returns `true` when movement is blocked and `false` when it succeeds;
/// this helper does not return a value but mutates `body` to reflect the final action taken.
fn attempt_movement_with_avoidance(
    body: &mut PhysicsBody,
    chunks: &ServerChunkWorldMap,
    displacement: Vec3,
    delta: f32,
) {
    if !try_move(body, chunks, displacement, true) {
        body.velocity.x = displacement.x;
        body.velocity.z = displacement.z;
        return;
    }

    if !body.on_ground {
        return;
    }

    if body.velocity.x != 0.0 && body.velocity.z != 0.0 {
        body.velocity.y += JUMP_VELOCITY * delta;
        body.on_ground = false;
        body.velocity.x = 0.0;
        body.velocity.z = 0.0;
        return;
    }

    if !try_move(body, chunks, Vec3::new(displacement.x, 0.0, 0.0), true) {
        body.velocity.x = displacement.x;
    } else if !try_move(body, chunks, Vec3::new(0.0, 0.0, displacement.z), true) {
        body.velocity.z = displacement.z;
    } else {
        body.velocity.y += JUMP_VELOCITY * delta;
        body.on_ground = false;
        body.velocity.x = 0.0;
        body.velocity.z = 0.0;
    }
}

pub fn mob_behavior_system(mut world_map: ResMut<ServerWorldMap>, delta: Res<Time<Fixed>>) {
    let mut mobs = world_map.mobs.clone();

    for (_mob_id, mob) in mobs.iter_mut() {
        //log::info!("Mob is at position: {:?}", mob.position);
        if (mob.position.x.is_nan() || mob.position.y.is_nan() || mob.position.z.is_nan())
            || (mob.velocity.x.is_nan() || mob.velocity.y.is_nan() || mob.velocity.z.is_nan())
        {
            //log::error!("Mob has NaN position or velocity");
            // TODO: FIX mob position
            return;
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
            MobTarget::Mob(id) => world_map.mobs.get(&id).unwrap().position,
        };

        // same gravity management as the player
        let dir = (target - mob.position).normalize();
        let delta = delta.delta_secs();

        let mut body = PhysicsBody::new(
            mob.position,
            mob.velocity,
            mob.on_ground,
            Vec3::new(mob.width, mob.height, mob.depth),
        );

        apply_gravity(&mut body, GRAVITY, delta);

        let max_velocity = 0.9;
        resolve_vertical_movement(&mut body, &world_map.chunks, max_velocity, true);

        match mob.action {
            MobAction::Walk | MobAction::Attack => {
                let speed = SPEED * delta;
                let displacement = Vec3::new(dir.x * speed, 0.0, dir.z * speed);
                attempt_movement_with_avoidance(&mut body, &world_map.chunks, displacement, delta);

                mob.rotation = Quat::from_rotation_y(atan2(dir.x, dir.z));

                // If reached destination, start idling
                if body.position.distance(target) < 0.5 {
                    mob.action = MobAction::Flee;
                }
            }
            MobAction::Flee => {
                if body.position.distance(target) < 15.0 {
                    body.position -= dir * delta;
                    mob.rotation = Quat::from_rotation_y(atan2(-dir.x, -dir.z));
                }
            }
            _ => {}
        }

        mob.position = body.position;
        mob.velocity = body.velocity;
        mob.on_ground = body.on_ground;
    }

    world_map.mobs = mobs;
}

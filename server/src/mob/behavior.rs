use bevy::{
    math::{ops::atan2, Quat, Vec3},
    time::{Fixed, Time},
};
use bevy_ecs::system::{Res, ResMut};
use shared::{
    physics::{apply_gravity, resolve_vertical_movement, try_move, PhysicsBody},
    players::constants::{GRAVITY, JUMP_VELOCITY, SPEED},
    world::{MobAction, MobTarget, ServerWorldMap},
};

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
                if !try_move(&mut body, &world_map.chunks, displacement, true) {
                    body.velocity.x = dir.x * speed;
                    body.velocity.z = dir.z * speed;
                }
                // If it can't move, try to jump (only if on ground and if it moved before)
                else if body.on_ground && (body.velocity.x != 0.0 && body.velocity.z != 0.0) {
                    body.velocity.y += JUMP_VELOCITY * delta;
                    body.on_ground = false;
                    body.velocity.x = 0.0;
                    body.velocity.z = 0.0;
                } else if body.on_ground {
                    // Try to move in the other direction
                    if !try_move(
                        &mut body,
                        &world_map.chunks,
                        Vec3::new(displacement.x, 0.0, 0.0),
                        true,
                    ) {
                        body.velocity.x = dir.x * speed;
                    } else if !try_move(
                        &mut body,
                        &world_map.chunks,
                        Vec3::new(0.0, 0.0, displacement.z),
                        true,
                    ) {
                        body.velocity.z = dir.z * speed;
                    //Try to jump (can improve this)
                    } else {
                        body.velocity.y += JUMP_VELOCITY * delta;
                        body.on_ground = false;
                        body.velocity.x = 0.0;
                        body.velocity.z = 0.0;
                    }
                }

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

use crate::init::ServerTime;
use crate::network::extensions::SendGameMessageExtension;
use bevy::math::IVec3;
use bevy::prelude::*;
use bevy_ecs::system::ResMut;
use bevy_renet::renet::RenetServer;
use shared::messages::mob::MobUpdateEvent;
use shared::messages::{ItemStackUpdateEvent, PlayerId, ServerToClientMessage, WorldUpdate};
use shared::players::Player;
use shared::world::{
    world_position_to_chunk_position, ServerChunk, ServerChunkWorldMap, ServerWorldMap,
};
use shared::{GameServerConfig, CHUNK_SIZE};
use std::collections::HashMap;

/// Maximum number of chunks to send to a client per update
const MAX_CHUNKS_PER_UPDATE: usize = 50;

// Chunk prioritization constants used by get_all_active_chunks and get_player_chunks_prioritized
/// Dot product threshold for considering a chunk as "in front" of the player.
/// -0.3 allows a wider viewing angle (~108° from center vs 90° for 0.0).
/// This ensures chunks slightly behind the player are still prioritized.
const FORWARD_DOT_THRESHOLD: f32 = -0.3;

/// Multiplier for view direction bias when chunks are in front of the player.
/// A value of 500.0 creates a smooth falloff for peripheral chunks,
/// balancing between distance and view direction importance.
const VIEW_DIRECTION_MULTIPLIER: f32 = 500.0;

/// Penalty added to chunks behind the player to deprioritize them.
/// 5000.0 creates a noticeable but not extreme deprioritization,
/// allowing chunks behind to still be loaded but with lower priority.
const BEHIND_PLAYER_PENALTY: f32 = 5000.0;

pub fn broadcast_world_state(
    mut server: ResMut<RenetServer>,
    time: Res<ServerTime>,
    mut world_map: ResMut<ServerWorldMap>,
    config: Res<GameServerConfig>,
) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let world_map = world_map.as_mut();

    let mobs = world_map.mobs.clone();
    let players = &mut world_map.players;
    let chunks = &mut world_map.chunks;

    for client in server.clients_id().iter_mut() {
        let player = players.get_mut(client);
        let player = match player {
            Some(p) => p.clone(),
            None => continue,
        };

        for (id, mob) in mobs.iter() {
            if mob.position.distance(player.position)
                < (config.broadcast_render_distance * CHUNK_SIZE) as f32
            {
                server.send_game_message(
                    *client,
                    ServerToClientMessage::MobUpdate(MobUpdateEvent {
                        id: *id,
                        mob: mob.clone(),
                    }),
                );
            }
        }

        let msg = WorldUpdate {
            tick: time.0,
            time: ts,
            new_map: get_world_map_chunks_to_send(
                chunks,
                players,
                &player,
                config.broadcast_render_distance,
            ),
            mobs: mobs.clone(),
            item_stacks: get_items_stacks(),
        };

        if msg.new_map.is_empty() {
            continue;
        }

        let message = ServerToClientMessage::WorldUpdate(msg);

        server.send_game_message(*client, message);
    }

    // Clear the list of chunks that needed updates after broadcasting to all clients
    chunks.chunks_to_update.clear();
}

fn get_world_map_chunks_to_send(
    chunks: &mut ServerChunkWorldMap,
    _players: &HashMap<PlayerId, Player>,
    player: &Player,
    broadcast_render_distance: i32,
) -> HashMap<IVec3, ServerChunk> {
    // Send only chunks in render distance
    let mut map: HashMap<IVec3, ServerChunk> = HashMap::new();

    let active_chunks = get_player_chunks_prioritized(player, broadcast_render_distance);

    // First, handle chunks that need to be updated (re-sent due to modifications)
    for &chunk_pos in &chunks.chunks_to_update {
        if active_chunks.contains(&chunk_pos) {
            if let Some(chunk) = chunks.map.get_mut(&chunk_pos) {
                // Clear sent_to_clients list so the chunk will be re-sent to all players
                chunk.sent_to_clients.clear();
            }
        }
    }

    for c in active_chunks {
        if map.len() >= MAX_CHUNKS_PER_UPDATE {
            break;
        }

        let chunk = chunks.map.get_mut(&c);

        // If chunk already exists, transmit it to client
        if let Some(chunk) = chunk {
            if chunk.sent_to_clients.contains(&player.id) {
                continue;
            }

            map.insert(c, chunk.clone());
            chunk.sent_to_clients.push(player.id);
        }
    }

    map
}

fn get_items_stacks() -> Vec<ItemStackUpdateEvent> {
    // TODO: Update later by requiring less data (does not need to borrow a full ServerWorldMap)
    vec![]
    // world_map
    //     .item_stacks
    //     .iter()
    //     .map(|stack| ItemStackUpdateEvent {
    //         id: stack.id,
    //         data: if stack.despawned {
    //             None
    //         } else {
    //             Some((stack.stack, stack.pos))
    //         },
    //     })
    //     .collect()
}

fn get_player_chunks_prioritized(player: &Player, radius: i32) -> Vec<IVec3> {
    let player_chunk_pos = world_position_to_chunk_position(player.position);
    let mut chunks = get_player_nearby_chunks_coords(player_chunk_pos, radius);

    // Prioritize chunks based on player's view direction
    let forward = player.camera_transform.forward();

    chunks.sort_by(|&a, &b| {
        let dir_a = (a - player_chunk_pos).as_vec3().normalize_or_zero();
        let dir_b = (b - player_chunk_pos).as_vec3().normalize_or_zero();

        // Calculate dot product with forward vector (higher = more in front)
        let dot_a = forward.dot(dir_a);
        let dot_b = forward.dot(dir_b);

        // Distance from player
        let dist_a = (a - player_chunk_pos).length_squared();
        let dist_b = (b - player_chunk_pos).length_squared();

        // Prioritize: closer chunks first, but favor chunks in view direction
        let score_a = if dot_a > FORWARD_DOT_THRESHOLD {
            dist_a as f32 - (dot_a * VIEW_DIRECTION_MULTIPLIER) // In/near view: closer = lower score
        } else {
            dist_a as f32 + BEHIND_PLAYER_PENALTY // Behind: higher score but not extreme
        };

        let score_b = if dot_b > FORWARD_DOT_THRESHOLD {
            dist_b as f32 - (dot_b * VIEW_DIRECTION_MULTIPLIER)
        } else {
            dist_b as f32 + BEHIND_PLAYER_PENALTY
        };

        score_a
            .partial_cmp(&score_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    chunks
}

pub fn get_all_active_chunks(
    players: &HashMap<PlayerId, Player>,
    radius: i32,
    requesting_player: &Player,
) -> Vec<IVec3> {
    let player_chunks: Vec<IVec3> = players
        .values()
        .map(|v| world_position_to_chunk_position(v.position))
        .flat_map(|v| get_player_nearby_chunks_coords(v, radius))
        .collect();

    let mut chunks: Vec<IVec3> = Vec::new();

    for c in player_chunks {
        if !chunks.contains(&c) {
            chunks.push(c);
        }
    }

    // Prioritize chunks based on requesting player's view direction
    let player_chunk_pos = world_position_to_chunk_position(requesting_player.position);
    let forward = requesting_player.camera_transform.forward();

    // Only partially sort the chunks we'll actually use
    // This significantly improves performance when there are many chunks
    let sort_count = chunks.len().min(MAX_CHUNKS_PER_UPDATE);

    if chunks.len() > 1 {
        chunks.select_nth_unstable_by(sort_count - 1, |&a, &b| {
            let dir_a = (a - player_chunk_pos).as_vec3().normalize_or_zero();
            let dir_b = (b - player_chunk_pos).as_vec3().normalize_or_zero();

            // Calculate dot product with forward vector (higher = more in front)
            let dot_a = forward.dot(dir_a);
            let dot_b = forward.dot(dir_b);

            // Distance from player
            let dist_a = (a - player_chunk_pos).length_squared();
            let dist_b = (b - player_chunk_pos).length_squared();

            // Prioritize: closer chunks first, but favor chunks in view direction
            let score_a = if dot_a > FORWARD_DOT_THRESHOLD {
                dist_a as f32 - (dot_a * VIEW_DIRECTION_MULTIPLIER) // In/near view: closer = lower score
            } else {
                dist_a as f32 + BEHIND_PLAYER_PENALTY // Behind: higher score but not extreme
            };

            let score_b = if dot_b > FORWARD_DOT_THRESHOLD {
                dist_b as f32 - (dot_b * VIEW_DIRECTION_MULTIPLIER)
            } else {
                dist_b as f32 + BEHIND_PLAYER_PENALTY
            };

            score_a
                .partial_cmp(&score_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    chunks
}

fn get_player_nearby_chunks_coords(
    player_chunk_position: IVec3,
    render_distance: i32,
) -> Vec<IVec3> {
    let mut chunks: Vec<IVec3> = Vec::new();
    let radius_squared = render_distance * render_distance;

    for x in -render_distance..=render_distance {
        for y in -render_distance..=render_distance {
            for z in -render_distance..=render_distance {
                let offset = IVec3::new(x, y, z);
                // Only include chunks within spherical distance
                if offset.length_squared() <= radius_squared {
                    chunks.push(player_chunk_position + offset);
                }
            }
        }
    }

    // let's sort by distance to player
    chunks.sort_by_key(|&c| (c - player_chunk_position).length_squared());

    chunks
}

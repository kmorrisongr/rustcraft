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

// Scaling factor for chunk limit based on render distance
// With the default render distance of 8, this gives 48 chunks per tick
// The factor of 6 provides a good balance between initial load speed and bandwidth usage
const CHUNKS_PER_RENDER_DISTANCE: i32 = 6;

// Chunk prioritization constants for get_all_active_chunks
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

/// Multiplier for vertical distance penalty when prioritizing chunks.
/// A value of 100.0 ensures chunks at the player's Y level are prioritized over
/// chunks far above or below, preventing underground chunks from rendering first
/// when the player is above ground. This creates a top-down rendering preference
/// relative to the player's vertical position.
const VERTICAL_DISTANCE_MULTIPLIER: f32 = 100.0;

/// Calculate a score for chunk prioritization based on distance and view direction.
/// # Arguments
/// * `chunk_pos` - Position of the chunk being evaluated.
/// * `player_chunk_pos` - Chunk the player is currently in.
/// * `forward` - Player's forward view direction.
fn get_chunk_render_score(chunk_pos: IVec3, player_chunk_pos: IVec3, forward: Vec3) -> f32 {
    let direction_from_player = (chunk_pos - player_chunk_pos).as_vec3().normalize_or_zero();
    let direction_dot_product = forward.dot(direction_from_player);
    let distance_from_player = (chunk_pos - player_chunk_pos).length_squared();

    // Add vertical distance penalty to prioritize chunks at player's Y level
    let y_distance = (chunk_pos.y - player_chunk_pos.y).abs();
    let vertical_penalty = y_distance as f32 * VERTICAL_DISTANCE_MULTIPLIER;

    if direction_dot_product > FORWARD_DOT_THRESHOLD {
        distance_from_player as f32 - (direction_dot_product * VIEW_DIRECTION_MULTIPLIER)
            + vertical_penalty
    } else {
        distance_from_player as f32 + BEHIND_PLAYER_PENALTY + vertical_penalty
    }
}

fn order_chunks_by_render_score(
    a: &IVec3,
    b: &IVec3,
    player_chunk_pos: IVec3,
    forward: Vec3,
) -> std::cmp::Ordering {
    let score_a = get_chunk_render_score(*a, player_chunk_pos, forward);
    let score_b = get_chunk_render_score(*b, player_chunk_pos, forward);

    score_a
        .partial_cmp(&score_b)
        .unwrap_or(std::cmp::Ordering::Equal)
}

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
    player: &Player,
    broadcast_render_distance: i32,
) -> HashMap<IVec3, ServerChunk> {
    // Send only chunks in render distance
    let mut map: HashMap<IVec3, ServerChunk> = HashMap::new();

    // Scale chunk limit based on render distance to prevent bandwidth issues
    // with larger render distances while maintaining good performance
    // Use saturating multiplication to prevent overflow with very large render distances
    let chunk_limit = broadcast_render_distance
        .saturating_mul(CHUNKS_PER_RENDER_DISTANCE)
        .min(MAX_CHUNKS_PER_UPDATE as i32) as usize;

    let active_chunks =
        get_player_chunks_prioritized(player, broadcast_render_distance, chunk_limit);

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
        // Should not be necessary due to prior generation, but double-check
        if map.len() >= chunk_limit {
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

/// Get chunk coordinates around a player prioritized by view direction
///
/// Resulting vector is partially sorted to prioritize chunks in front of the player
/// up to max_chunks.
fn get_player_chunks_prioritized(player: &Player, radius: i32, max_chunks: usize) -> Vec<IVec3> {
    let player_chunk_pos = world_position_to_chunk_position(player.position);
    let mut chunks = get_player_nearby_chunks_coords(player_chunk_pos, radius);

    // Prioritize chunks based on player's view direction
    let forward = player.camera_transform.forward();

    let sort_count = chunks.len().min(max_chunks);
    if chunks.len() > 1 {
        chunks.select_nth_unstable_by(sort_count - 1, |a, b| {
            order_chunks_by_render_score(a, b, player_chunk_pos, *forward)
        });
    }

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
        chunks.select_nth_unstable_by(sort_count - 1, |a, b| {
            order_chunks_by_render_score(a, b, player_chunk_pos, *forward)
        });
    }

    chunks
}

/// Get all chunk coordinates within a spherical radius around the player's chunk position
///
/// Resulting vector is not sorted in any way.
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

    chunks
}

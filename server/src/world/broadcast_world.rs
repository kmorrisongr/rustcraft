use crate::init::ServerTime;
use crate::network::extensions::SendGameMessageExtension;
use bevy::prelude::*;
use bevy_ecs::system::ResMut;
use bevy_renet::renet::RenetServer;
use shared::messages::mob::MobUpdateEvent;
use shared::messages::{ItemStackUpdateEvent, ServerToClientMessage, WorldUpdate};
use shared::players::Player;
use shared::world::{
    ChunkPos, ServerChunk, ServerChunkWorldMap, ServerWorldMap 
};
use shared::{GameServerConfig, CHUNK_SIZE, LOD1_MULTIPLIER};
use std::collections::HashMap;

/// Maximum number of chunks to send to a client per update
const MAX_CHUNKS_PER_UPDATE: usize = 50;

// Scaling factor for chunk limit based on render distance
// With the default render distance of 8, this gives 48 chunks per tick
// The factor of 6 provides a good balance between initial load speed and bandwidth usage
const CHUNKS_PER_RENDER_DISTANCE: i32 = 6;


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

        // Use extended render distance to support LOD 1 chunks on the client
        let effective_render_distance =
            (config.broadcast_render_distance as f32 * LOD1_MULTIPLIER) as i32;

        let msg = WorldUpdate {
            tick: time.0,
            time: ts,
            new_map: get_world_map_chunks_to_send(chunks, &player, effective_render_distance),
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
) -> HashMap<ChunkPos, ServerChunk> {
    // Send only chunks in render distance
    let mut map: HashMap<ChunkPos, ServerChunk> = HashMap::new();

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
            chunk.sent_to_clients.insert(player.id);
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

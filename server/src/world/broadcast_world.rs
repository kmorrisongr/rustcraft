use std::collections::{HashMap, HashSet};

use crate::init::ServerTime;
use crate::network::extensions::SendGameMessageExtension;
use crate::world::riverbed::{ChunkChangesReceiver, VoxelWorld};
use bevy::prelude::*;
use bevy_ecs::system::ResMut;
use bevy_renet::renet::RenetServer;
use shared::messages::mob::MobUpdateEvent;
use shared::messages::{ItemStackUpdateEvent, PlayerId, ServerToClientMessage, WorldUpdate};
use shared::players::Player;
use shared::world::{
    ChunkPos, SerdablePackedUints, ServerChunk, ServerWorldMap,
};
use shared::{GameServerConfig, CHUNK_SIZE, LOD1_MULTIPLIER};

/// Maximum number of chunks to send to a client per update
const MAX_CHUNKS_PER_UPDATE: usize = 50;

/// Tracks which chunks have been sent to which clients
#[derive(Resource, Default)]
pub struct ChunkSendTracker {
    pub sent_to_clients: HashMap<ChunkPos, HashSet<PlayerId>>,
}

/// System to process chunk change notifications and invalidate send tracker
pub fn process_chunk_changes(
    chunk_changes: Res<ChunkChangesReceiver>,
    mut send_tracker: ResMut<ChunkSendTracker>,
) {
    // Drain all pending chunk changes and clear their sent status
    while let Ok(chunk_pos) = chunk_changes.0.try_recv() {
        send_tracker.sent_to_clients.remove(&chunk_pos);
    }
}


pub fn broadcast_world_state(
    mut server: ResMut<RenetServer>,
    time: Res<ServerTime>,
    mut world_map: ResMut<ServerWorldMap>,
    voxel_world: Res<VoxelWorld>,
    mut send_tracker: ResMut<ChunkSendTracker>,
    config: Res<GameServerConfig>,
) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let world_map = world_map.as_mut();

    let mobs = world_map.mobs.clone();
    let players = &mut world_map.players;

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
            new_map: get_world_map_chunks_to_send(&voxel_world, &mut *send_tracker, &player, effective_render_distance),
            mobs: mobs.clone(),
            item_stacks: get_items_stacks(),
        };

        if msg.new_map.is_empty() {
            continue;
        }

        let message = ServerToClientMessage::WorldUpdate(msg);

        server.send_game_message(*client, message);
    }
}

fn get_world_map_chunks_to_send(
    voxel_world: &VoxelWorld,
    send_tracker: &mut ChunkSendTracker,
    player: &Player,
    broadcast_render_distance: i32,
) -> HashMap<ChunkPos, ServerChunk> {
    let mut map: HashMap<ChunkPos, ServerChunk> = HashMap::new();
    
    let player_chunk_x = (player.position.x / CHUNK_SIZE as f32).floor() as i32;
    let player_chunk_z = (player.position.z / CHUNK_SIZE as f32).floor() as i32;
    
    // Collect chunks within render distance, sorted by distance to player
    let mut candidate_chunks: Vec<(ChunkPos, i32)> = Vec::new();
    
    for entry in voxel_world.chunks.iter() {
        let chunk_pos = *entry.key();
        
        // Check if chunk is within render distance (horizontal only)
        let dx = chunk_pos.x - player_chunk_x;
        let dz = chunk_pos.z - player_chunk_z;
        let dist_sq = dx * dx + dz * dz;
        
        if dist_sq <= broadcast_render_distance * broadcast_render_distance {
            candidate_chunks.push((chunk_pos, dist_sq));
        }
    }
    
    // Sort by distance (closest first)
    candidate_chunks.sort_by_key(|(_, dist)| *dist);
    
    // Send chunks that haven't been sent to this player yet
    for (chunk_pos, _) in candidate_chunks {
        if map.len() >= MAX_CHUNKS_PER_UPDATE {
            break;
        }
        
        // Check if already sent to this player
        let sent_set = send_tracker.sent_to_clients.entry(chunk_pos).or_default();
        if sent_set.contains(&player.id) {
            continue;
        }
        
        // Get chunk and convert to ServerChunk
        if let Some(chunk_entry) = voxel_world.chunks.get(&chunk_pos) {
            let chunk = chunk_entry.value().read();
            let server_chunk = ServerChunk {
                data: SerdablePackedUints(chunk.data.clone()),
                palette: chunk.palette.clone(),
                ts: 0,
                sent_to_clients: HashSet::new(),
            };
            
            map.insert(chunk_pos, server_chunk);
            sent_set.insert(player.id);
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

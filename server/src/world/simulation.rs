use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use bevy_renet::renet::{ClientId, RenetServer};
use shared::{
    messages::{NetworkAction, PlayerFrameInput, PlayerUpdateEvent},
    players::{simulation::simulate_player_actions, blocks::CallerType},
    world::{ServerWorldMap, WorldSeed},
};

use crate::{
    network::extensions::SendGameMessageExtension,
    world::generation::{apply_pending_blocks, generate_chunk},
};

use super::broadcast_world::get_all_active_chunks;

#[derive(Event, Debug)]
pub struct PlayerInputsEvent {
    pub client_id: ClientId,
    pub input: PlayerFrameInput,
}

pub fn handle_player_inputs_system(
    mut events: EventReader<PlayerInputsEvent>,
    mut world_map: ResMut<ServerWorldMap>,
    mut server: ResMut<RenetServer>,
    seed: Res<WorldSeed>,
) {
    let world_map = world_map.as_mut();
    let players = &mut world_map.players;
    let chunks = &mut world_map.chunks;

    let active_chunks = get_all_active_chunks(players, 1);
    for c in active_chunks {
        let chunk = chunks.map.get(&c);

        if chunk.is_none() {
            let mut chunk = generate_chunk(c, seed.0);
            
            // Apply pending blocks from neighboring chunks
            apply_pending_blocks(&mut chunk, c, &chunks.map);
            
            info!("Generated chunk: {:?}", c);
            
            // Extract pending blocks before moving chunk into map
            let pending_blocks = chunk.pending_blocks.clone();
            
            // Insert chunk into map
            chunks.map.insert(c, chunk);
            
            // Push pending blocks to existing neighbors
            for (offset, blocks) in pending_blocks.iter() {
                let neighbor_pos = c + *offset;
                if let Some(neighbor_chunk) = chunks.map.get_mut(&neighbor_pos) {
                    for (local_pos, block_data) in blocks.iter() {
                        neighbor_chunk.map.entry(*local_pos).or_insert(*block_data);
                    }
                }
            }
        }
    }

    let mut player_actions = HashMap::<u64, HashSet<NetworkAction>>::new();
    for client_id in players.keys() {
        player_actions.insert(*client_id, HashSet::new());
    }

    for ev in events.read() {
        let player = players.get_mut(&ev.client_id).unwrap();

        simulate_player_actions(player, chunks, &ev.input.clone(), CallerType::Server);

        player.last_input_processed = ev.input.time_ms;
    }

    for player in players.values() {
        server.broadcast_game_message(shared::messages::ServerToClientMessage::PlayerUpdate(
            PlayerUpdateEvent {
                id: player.id,
                position: player.position,
                orientation: player.camera_transform.rotation,
                last_ack_time: player.last_input_processed,
                inventory: player.inventory.clone(),
            },
        ));
    }
}

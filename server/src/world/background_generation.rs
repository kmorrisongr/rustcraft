use bevy::prelude::*;
use log::info;
use shared::world::{ServerWorldMap, WorldSeed};

use crate::world::generation::generate_chunk;

use super::broadcast_world::get_all_active_chunks;
use shared::GameServerConfig;

pub fn background_world_generation_system(
    mut world_map: ResMut<ServerWorldMap>,
    seed: Res<WorldSeed>,
    config: Res<GameServerConfig>,
) {
    // Get first player for chunk prioritization (or default if no players)
    let first_player = world_map.players.values().next();
    if first_player.is_none() {
        return; // No players, no need to generate chunks
    }

    let all_chunks = get_all_active_chunks(
        &world_map.players,
        config.broadcast_render_distance,
        first_player.unwrap(),
    );
    let mut generated = 0;
    for c in all_chunks {
        let chunk = world_map.chunks.map.get(&c);

        if chunk.is_none() {
            // Check for any pending generation requests for this chunk
            let pending_requests = world_map.chunks.generation_requests.remove(&c);

            // Generate the chunk with any pending requests
            let result = generate_chunk(c, seed.0, pending_requests);
            info!("Generated chunk: {:?}", c);
            world_map.chunks.map.insert(c, result.chunk);

            // Store any generation requests for the chunk above
            if !result.requests_for_chunk_above.is_empty() {
                let chunk_above = IVec3::new(c.x, c.y + 1, c.z);
                world_map
                    .chunks
                    .generation_requests
                    .entry(chunk_above)
                    .or_default()
                    .extend(result.requests_for_chunk_above);
            }

            generated += 1;
        }

        if generated >= 1 {
            break;
        }
    }
}

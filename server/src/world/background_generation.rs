use bevy::prelude::*;
use shared::world::{ServerWorldMap, WorldSeed};

use crate::world::generation::{apply_pending_blocks, generate_chunk};

use super::broadcast_world::{get_all_active_chunks, BROADCAST_RENDER_DISTANCE};

pub fn background_world_generation_system(
    mut world_map: ResMut<ServerWorldMap>,
    seed: Res<WorldSeed>,
) {
    let all_chunks = get_all_active_chunks(&world_map.players, BROADCAST_RENDER_DISTANCE);
    let mut generated = 0;
    for c in all_chunks {
        let chunk = world_map.chunks.map.get(&c);

        if chunk.is_none() {
            let mut chunk = generate_chunk(c, seed.0);
            
            // Apply pending blocks from neighboring chunks
            apply_pending_blocks(&mut chunk, c, &world_map.chunks.map);
            
            info!("Generated chunk: {:?}", c);
            
            // Extract pending blocks before moving chunk into map
            let pending_blocks = chunk.pending_blocks.clone();
            
            // Insert chunk into map
            world_map.chunks.map.insert(c, chunk);
            
            // Push pending blocks to existing neighbors
            for (offset, blocks) in pending_blocks.iter() {
                let neighbor_pos = c + *offset;
                if let Some(neighbor_chunk) = world_map.chunks.map.get_mut(&neighbor_pos) {
                    for (local_pos, block_data) in blocks.iter() {
                        neighbor_chunk.map.entry(*local_pos).or_insert(*block_data);
                    }
                }
            }
            
            generated += 1;
        }

        if generated >= 1 {
            break;
        }
    }
}

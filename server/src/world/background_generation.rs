use bevy::prelude::*;
use shared::world::{ServerWorldMap, WorldSeed};

use crate::world::generation::generate_chunk;

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
            
            // Process pending blocks from neighboring chunks
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        if dx == 0 && dy == 0 && dz == 0 {
                            continue;
                        }
                        
                        let neighbor_pos = c + IVec3::new(dx, dy, dz);
                        let inverse_offset = IVec3::new(-dx, -dy, -dz);
                        
                        if let Some(neighbor_chunk) = world_map.chunks.map.get(&neighbor_pos) {
                            if let Some(pending_blocks) = neighbor_chunk.pending_blocks.get(&inverse_offset) {
                                for (local_pos, block_data) in pending_blocks.iter() {
                                    chunk.map.insert(*local_pos, *block_data);
                                }
                            }
                        }
                    }
                }
            }
            
            info!("Generated chunk: {:?}", c);
            world_map.chunks.map.insert(c, chunk);
            generated += 1;
        }

        if generated >= 1 {
            break;
        }
    }
}

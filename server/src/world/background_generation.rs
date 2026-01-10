use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use futures_lite::future;
use log::info;
use shared::world::{FloraRequest, ServerWorldMap, WorldSeed};
use shared::LOD1_MULTIPLIER;
use std::collections::HashSet;

use crate::world::generation::{
    generate_chunk, generate_debug_water_chunk, ChunkGenerationResult, DebugWaterWorld,
};
use crate::world::water_simulation::WaterSurfaceUpdateQueue;

use super::broadcast_world::get_all_active_chunks;
use shared::GameServerConfig;

const MAX_CONCURRENT_GENERATION_TASKS: usize = 4;

/// Resource to track in-progress chunk generation tasks.
///
/// The `in_progress` HashSet duplicates position information from `tasks`, but provides
/// O(1) lookup vs O(n) linear scan of the tasks vec. With a small MAX_CONCURRENT_GENERATION_TASKS
/// (currently 4), the memory overhead is negligible and the O(1) lookup is worthwhile since we
/// check membership for every candidate chunk each frame.
#[derive(Resource, Default)]
pub struct ChunkGenerationTasks {
    /// Active generation tasks with their chunk positions
    pub tasks: Vec<(IVec3, Task<ChunkGenerationResult>)>,
    /// Chunk positions currently being generated (for O(1) duplicate checking)
    pub in_progress: HashSet<IVec3>,
}

/// System to spawn async chunk generation tasks and collect completed results.
///
/// Spawns up to MAX_CONCURRENT_GENERATION_TASKS parallel chunk generation tasks
/// using Bevy's AsyncComputeTaskPool, then polls for completed tasks and integrates
/// the results into the world map.
pub fn background_chunk_generation_system(
    mut world_map: ResMut<ServerWorldMap>,
    seed: Res<WorldSeed>,
    config: Res<GameServerConfig>,
    mut generation_tasks: ResMut<ChunkGenerationTasks>,
    mut surface_queue: ResMut<WaterSurfaceUpdateQueue>,
) {
    // === Phase 1: Collect completed tasks ===
    let mut completed: Vec<(usize, IVec3, ChunkGenerationResult)> = Vec::new();

    for (index, (chunk_pos, task)) in generation_tasks.tasks.iter_mut().enumerate() {
        if let Some(result) = future::block_on(future::poll_once(task)) {
            completed.push((index, *chunk_pos, result));
        }
    }

    // Sort completed indices in descending order to safely remove without invalidating indices
    completed.sort_by(|a, b| b.0.cmp(&a.0));

    // Process completed results
    for (index, chunk_pos, result) in completed {
        info!("Generated chunk: {:?}", chunk_pos);

        // Queue for surface detection if the chunk has water
        if !result.chunk.water.is_empty() {
            surface_queue.queue(chunk_pos);
        }

        world_map.chunks.map.insert(chunk_pos, result.chunk);

        if !result.requests_for_chunk_above.is_empty() {
            let chunk_above = IVec3::new(chunk_pos.x, chunk_pos.y + 1, chunk_pos.z);
            world_map
                .chunks
                .generation_requests
                .entry(chunk_above)
                .or_default()
                .extend(result.requests_for_chunk_above);
        }

        // Remove from tasks first, then from in_progress to keep structures in sync
        let _ = generation_tasks.tasks.swap_remove(index);
        generation_tasks.in_progress.remove(&chunk_pos);
    }

    // === Phase 2: Spawn new tasks ===
    let first_player = match world_map.players.values().next() {
        Some(player) => player,
        None => return, // No players, no need to generate chunks
    };

    // Use extended render distance to generate chunks for LOD 1 rendering
    let effective_render_distance =
        (config.broadcast_render_distance as f32 * LOD1_MULTIPLIER) as i32;

    let all_chunks =
        get_all_active_chunks(&world_map.players, effective_render_distance, first_player);

    let task_pool = AsyncComputeTaskPool::get();
    let seed_value = seed.0;
    let is_debug_water = config.debug_water_world;

    for chunk_pos in all_chunks {
        if generation_tasks.tasks.len() >= MAX_CONCURRENT_GENERATION_TASKS {
            break;
        }

        if world_map.chunks.map.contains_key(&chunk_pos)
            || generation_tasks.in_progress.contains(&chunk_pos)
        {
            continue;
        }

        let pending_requests: Option<Vec<FloraRequest>> =
            world_map.chunks.generation_requests.remove(&chunk_pos);

        let task = if is_debug_water {
            // In debug mode, generate flat terrain with water test pools
            task_pool.spawn(async move {
                let config = DebugWaterWorld::default();
                let chunk = generate_debug_water_chunk(chunk_pos, &config);
                ChunkGenerationResult {
                    chunk,
                    requests_for_chunk_above: Vec::new(),
                }
            })
        } else {
            // Normal world generation
            task_pool.spawn(async move { generate_chunk(chunk_pos, seed_value, pending_requests) })
        };

        generation_tasks.tasks.push((chunk_pos, task));
        generation_tasks.in_progress.insert(chunk_pos);
    }
}

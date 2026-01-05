use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use futures_lite::future;
use log::info;
use shared::world::{FloraRequest, ServerWorldMap, WorldSeed};
use std::collections::HashSet;

use crate::world::generation::{generate_chunk, ChunkGenerationResult};

use super::broadcast_world::get_all_active_chunks;
use shared::GameServerConfig;

const MAX_CONCURRENT_GENERATION_TASKS: usize = 10;

/// Resource to track in-progress chunk generation tasks
#[derive(Resource, Default)]
pub struct ChunkGenerationTasks {
    /// Active generation tasks, keyed by chunk position
    pub tasks: Vec<(IVec3, Task<ChunkGenerationResult>)>,
    /// Chunks currently being generated (to avoid duplicate tasks)
    pub in_progress: HashSet<IVec3>,
}

pub fn spawn_chunk_generation_tasks(
    mut world_map: ResMut<ServerWorldMap>,
    seed: Res<WorldSeed>,
    config: Res<GameServerConfig>,
    mut generation_tasks: ResMut<ChunkGenerationTasks>,
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

    let task_pool = AsyncComputeTaskPool::get();
    let seed_value = seed.0;

    // Spawn new tasks up to the limit
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

        generation_tasks.in_progress.insert(chunk_pos);

        let task =
            task_pool.spawn(async move { generate_chunk(chunk_pos, seed_value, pending_requests) });

        generation_tasks.tasks.push((chunk_pos, task));
    }
}

/// System to poll and collect completed chunk generation tasks
pub fn collect_chunk_generation_tasks(
    mut world_map: ResMut<ServerWorldMap>,
    mut generation_tasks: ResMut<ChunkGenerationTasks>,
) {
    // Process completed tasks - collect results first to avoid borrow issues
    let mut completed: Vec<(usize, IVec3, ChunkGenerationResult)> = Vec::new();

    for (index, (chunk_pos, task)) in generation_tasks.tasks.iter_mut().enumerate() {
        // Poll the task to see if it's complete
        if let Some(result) = future::block_on(future::poll_once(task)) {
            completed.push((index, *chunk_pos, result));
        }
    }

    // Process completed results (in reverse order to preserve indices during removal)
    for (index, chunk_pos, result) in completed.into_iter().rev() {
        info!("Generated chunk: {:?}", chunk_pos);

        world_map.chunks.map.insert(chunk_pos, result.chunk);

        // Store any generation requests for the chunk above
        if !result.requests_for_chunk_above.is_empty() {
            let chunk_above = IVec3::new(chunk_pos.x, chunk_pos.y + 1, chunk_pos.z);
            world_map
                .chunks
                .generation_requests
                .entry(chunk_above)
                .or_default()
                .extend(result.requests_for_chunk_above);
        }

        generation_tasks.in_progress.remove(&chunk_pos);
        let _ = generation_tasks.tasks.swap_remove(index);
    }
}

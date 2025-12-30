use crate::world::time::ClientTime;
use crate::world::{ClientChunk, ClientWorldMap};
use bevy::{math::IVec3, prelude::*};
use shared::world::BlockData;
use std::mem::size_of;

#[derive(Component)]
pub struct BlocksNumberText;

#[derive(Component)]
pub struct TimeText;

#[derive(Component)]
pub struct ChunksNumberText;

pub fn total_blocks_text_update_system(
    query_blocks: Query<Entity, With<BlocksNumberText>>,
    query_chunks: Query<Entity, (With<ChunksNumberText>, Without<BlocksNumberText>)>,
    mut writer: TextUiWriter,
    world_map: Res<ClientWorldMap>,
) {
    for entity in query_blocks.iter() {
        *writer.text(entity, 0) = format!("Loaded blocks: {}", world_map.total_blocks_count);
    }
    for entity in query_chunks.iter() {
        let chunk_count = world_map.map.len();
        // Rough, implementation-agnostic approximation of per-slot HashMap overhead.
        // Note: std::collections::HashMap uses control bytes plus contiguous key/value storage; this
        // is an estimated lower bound for debug display only, not an exact memory model.
        const HASHMAP_ENTRY_OVERHEAD_USIZE: usize = 2; // approx. two usize words per slot (lower bound)
        let slot_overhead_bytes = size_of::<usize>() * HASHMAP_ENTRY_OVERHEAD_USIZE;
        let control_overhead_bytes = size_of::<usize>(); // approximate control/bucket array cost per slot

        let chunk_table_bytes: usize = world_map
            .map
            .iter()
            .map(|(_, chunk)| {
                let entry_bytes = chunk.map.len()
                    * (size_of::<IVec3>() + size_of::<BlockData>() + slot_overhead_bytes);
                let control_bytes = chunk.map.capacity() * control_overhead_bytes;
                entry_bytes + control_bytes
            })
            .sum();

        let world_entry_bytes = world_map.map.len()
            * (size_of::<IVec3>() + size_of::<ClientChunk>() + slot_overhead_bytes);
        let world_control_bytes = world_map.map.capacity() * control_overhead_bytes;
        let world_table_bytes = world_entry_bytes + world_control_bytes;

        let estimated_bytes = chunk_table_bytes + world_table_bytes;
        let estimated_mb = estimated_bytes as f32 / (1024.0 * 1024.0);
        *writer.text(entity, 0) =
            format!("Loaded chunks: {} (~{estimated_mb:.2} MiB)", chunk_count);
    }
}

pub fn time_text_update_system(
    query: Query<Entity, With<TimeText>>,
    mut writer: TextUiWriter,
    time_resource: Res<ClientTime>,
) {
    for entity in query.iter() {
        *writer.text(entity, 0) = format!("Time: {}", time_resource.0);
    }
}

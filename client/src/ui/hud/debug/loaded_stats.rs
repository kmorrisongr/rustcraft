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
        let block_entries: usize = world_map.map.values().map(|chunk| chunk.map.len()).sum();
        // HashMap entry overhead varies by implementation and platform; this is a rough
        // approximation using two usize words of metadata (hash/next) per entry
        const HASHMAP_ENTRY_OVERHEAD_USIZE: usize = 2;
        let block_entry_bytes =
            size_of::<IVec3>() + size_of::<BlockData>() + size_of::<usize>() * HASHMAP_ENTRY_OVERHEAD_USIZE;
        let chunk_entry_bytes =
            size_of::<IVec3>() + size_of::<ClientChunk>() + size_of::<usize>() * HASHMAP_ENTRY_OVERHEAD_USIZE;
        let estimated_bytes = block_entries * block_entry_bytes + chunk_count * chunk_entry_bytes;
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

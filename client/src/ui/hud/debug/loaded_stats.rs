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
        let estimated_bytes = block_entries * size_of::<(IVec3, BlockData)>()
            + chunk_count * size_of::<(IVec3, ClientChunk)>();
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

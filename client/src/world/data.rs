use bevy::prelude::*;
use shared::world::BlockData;
use shared::world::LodLevel;
use shared::world::WorldMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Instant;

use bevy::math::IVec3;
use bevy::prelude::Resource;
use shared::world::global_to_chunk_local;
use std::collections::HashMap;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub enum GlobalMaterial {
    Sun,
    Moon,
    Blocks,
    Items,
}

#[derive(Clone, Debug)]
pub struct ClientChunk {
    pub map: HashMap<IVec3, BlockData>, // Maps block positions within a chunk to block IDs
    pub entity: Option<Entity>,
    pub last_mesh_ts: Instant, // When was the last time a mesh was created for this chunk ?
    pub current_lod: LodLevel, // Current LOD level of this chunk's mesh
}

impl Default for ClientChunk {
    fn default() -> Self {
        Self {
            map: HashMap::new(),
            entity: None,
            last_mesh_ts: Instant::now(),
            current_lod: LodLevel::default(),
        }
    }
}

#[derive(Resource, Clone)]
pub struct ClientWorldMap {
    pub name: String,
    pub map: HashMap<IVec3, Arc<ClientChunk>>, // Maps global chunk positions to chunks (Arc for cheap cloning)
    pub total_blocks_count: u64,
    pub total_chunks_count: u64,
    pub dirty: bool,
}

impl Default for ClientWorldMap {
    fn default() -> Self {
        Self {
            name: String::new(),
            map: HashMap::new(),
            total_blocks_count: 0,
            total_chunks_count: 0,
            dirty: true,
        }
    }
}

impl ClientWorldMap {
    #[inline]
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}

impl WorldMap for ClientWorldMap {
    fn has_chunk(&self, chunk_pos: &IVec3) -> bool {
        self.map.contains_key(chunk_pos)
    }

    fn get_block_by_coordinates(&self, position: &IVec3) -> Option<&BlockData> {
        let (chunk_pos, local_pos) = global_to_chunk_local(position);
        let chunk = self.map.get(&chunk_pos)?;
        chunk.map.get(&local_pos)
    }

    fn get_block_mut_by_coordinates(&mut self, position: &IVec3) -> Option<&mut BlockData> {
        let (chunk_pos, local_pos) = global_to_chunk_local(position);
        let chunk = Arc::make_mut(self.map.get_mut(&chunk_pos)?);
        chunk.map.get_mut(&local_pos)
    }

    fn remove_block_by_coordinates(&mut self, global_block_pos: &IVec3) -> Option<BlockData> {
        let block: &BlockData = self.get_block_by_coordinates(global_block_pos)?;
        let kind: BlockData = *block;

        let (chunk_pos, local_block_pos) = global_to_chunk_local(global_block_pos);
        let chunk_arc = self.map.get_mut(&chunk_pos)?;
        let chunk_map: &mut ClientChunk = Arc::make_mut(chunk_arc);

        chunk_map.map.remove(&local_block_pos);
        self.mark_dirty();

        Some(kind)
    }

    fn set_block(&mut self, position: &IVec3, block: BlockData) {
        let (chunk_pos, local_pos) = global_to_chunk_local(position);
        let chunk: &mut ClientChunk = Arc::make_mut(
            self.map
                .entry(chunk_pos)
                .or_insert_with(|| Arc::new(ClientChunk::default())),
        );

        chunk.map.insert(local_pos, block);
        self.mark_dirty();
    }

    fn mark_block_for_update(&mut self, _block_pos: &IVec3) {
        // Useless in client
    }
}

#[derive(Default, Debug)]
pub struct QueuedEvents {
    pub events: HashSet<WorldRenderRequestUpdateEvent>, // Set of events for rendering updates
}

#[derive(Event, Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum WorldRenderRequestUpdateEvent {
    ChunkToReload(IVec3),
}

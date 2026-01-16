use crate::messages::PlayerId;
use crate::players::Player;
use crate::world::utils::Palette;
use crate::world::{BlockPos, ChunkPos};
use crate::world::blocks::blocks::{BlockData, BlockHitbox, BlockId};
use bevy::math::bounding::Aabb3d;
use bevy::math::{Vec3};
use bevy_ecs::resource::Resource;
use packed_uints::PackedUints;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

use super::{ItemId, ItemType, MobId, ServerMob};

// Biome generation constants - shared between client and server
/// Scale factor for biome noise generation
pub const BIOME_SCALE: f32 = 0.01;
/// Seed offset for temperature noise generation
pub const TEMP_SEED_OFFSET: u32 = 1;
/// Seed offset for humidity noise generation
pub const HUMIDITY_SEED_OFFSET: u32 = 2;

/// Represents a type of flora that can be requested for generation in the chunk above.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FloraType {
    /// A flower (Dandelion or Poppy)
    Flower,
    /// Tall grass
    TallGrass,
    /// A standard tree
    Tree,
    /// A big tree (Forest biome)
    BigTree,
    /// A cactus
    Cactus,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ServerItemStack {
    pub id: u128,
    pub despawned: bool,
    pub stack: ItemStack,
    pub pos: Vec3,
    pub timestamp: u64,
}

#[derive(Clone, Debug)]
pub struct SerdablePackedUints(pub PackedUints);

impl Default for SerdablePackedUints {
    fn default() -> Self {
        Self(PackedUints::new(0))
    }
}

impl Serialize for SerdablePackedUints {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as u32 values (portable, sufficient for block IDs)
        // Format: [length as u32, then all values as u32]
        let length = self.0.length as u32;
        let values: Vec<u32> = self.0.unpack_u32();
        
        // Build the byte buffer: 4 bytes for length + 4 bytes per value
        let mut bytes: Vec<u8> = Vec::with_capacity(4 + values.len() * 4);
        bytes.extend_from_slice(&length.to_le_bytes());
        for val in values.iter().take(self.0.length) {
            bytes.extend_from_slice(&val.to_le_bytes());
        }
        
        serializer.serialize_bytes(&bytes)
    }
}

impl<'de> Deserialize<'de> for SerdablePackedUints {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct PackedUintsVisitor;

        impl<'de> serde::de::Visitor<'de> for PackedUintsVisitor {
            type Value = SerdablePackedUints;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a byte array representing PackedUints")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v.len() < 4 {
                    return Err(E::custom("PackedUints data too short"));
                }
                
                // Read length (first 4 bytes)
                let length = u32::from_le_bytes(v[0..4].try_into().unwrap()) as usize;
                let data_bytes = &v[4..];
                
                if data_bytes.len() != length * 4 {
                    return Err(E::custom(format!(
                        "expected {} bytes for {} values, got {}",
                        length * 4,
                        length,
                        data_bytes.len()
                    )));
                }
                
                let values: Vec<usize> = data_bytes
                    .chunks_exact(4)
                    .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()) as usize)
                    .collect();
                
                Ok(SerdablePackedUints(PackedUints::from(values.as_slice())))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                // Handle formats that deserialize bytes as a sequence
                let mut bytes = Vec::new();
                while let Some(byte) = seq.next_element::<u8>()? {
                    bytes.push(byte);
                }
                self.visit_bytes(&bytes)
            }
        }

        deserializer.deserialize_bytes(PackedUintsVisitor)
    }
}

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct ServerChunk {
    pub data: SerdablePackedUints,
    pub palette: Palette<BlockId>,
    /// Timestamp marking the last update this chunk has received
    pub ts: u64,
    pub sent_to_clients: HashSet<PlayerId>,
}

// #[derive(Resource)]
// pub struct PlayerInventories(HashMap<PlayerId, Inventory>);

#[derive(Resource, Default, Clone, Serialize, Deserialize, Debug)]
pub struct ServerWorldMap {
    pub name: String,
    pub players: HashMap<PlayerId, Player>,
    pub mobs: HashMap<MobId, ServerMob>,
    pub item_stacks: Vec<ServerItemStack>,
    pub time: u64,
}

#[derive(Resource, Clone, Copy, Serialize, Deserialize, Default)]
pub struct WorldSeed(pub u32);

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Default, PartialEq)]
pub struct ItemStack {
    pub item_id: ItemId,
    pub item_type: ItemType,
    pub nb: u32,
}

pub trait WorldMap {
    fn has_chunk(&self, chunk_pos: &ChunkPos) -> bool;
    fn get_block_mut_by_coordinates(&mut self, position: &BlockPos) -> Option<&mut BlockData>;
    fn get_block_by_coordinates(&self, position: &BlockPos) -> Option<&BlockId>;
    fn remove_block_by_coordinates(&mut self, global_block_pos: &BlockPos) -> bool;
    fn set_block(&mut self, position: &BlockPos, block: BlockData);

    fn get_height_ground(&self, position: Vec3) -> i32 {
        for y in (0..256).rev() {
            if self
                .get_block_by_coordinates(&BlockPos::overworld(position.x as i32, y, position.z as i32))
                .is_some()
            {
                return y;
            }
        }
        0
    }

    /// Check if a bounding box collides with the world.
    ///
    /// By default, this checks for solid block collisions only.
    /// Override check_collision_box_with_water for dynamic water surface collision.
    fn check_collision_box(&self, hitbox: &Aabb3d) -> bool {
        self.check_collision_box_with_water(hitbox, None)
    }

    /// Check collision with optional water surface height.
    ///
    /// # Arguments
    /// * `hitbox` - The bounding box to check for collisions
    /// * `water_height_fn` - Optional function to get dynamic water surface height at a position
    ///
    /// Default implementation checks only solid blocks. Can be overridden for dynamic water.
    fn check_collision_box_with_water(
        &self,
        hitbox: &Aabb3d,
        water_height_fn: Option<&dyn Fn(f32, f32) -> f32>,
    ) -> bool {
        // Check all blocks inside the hitbox
        // Manual flooring is needed for negative coordinates
        for x in (hitbox.min.x.floor() as i32)..=(hitbox.max.x.floor() as i32) {
            for y in (hitbox.min.y.floor() as i32)..=(hitbox.max.y.floor() as i32) {
                for z in (hitbox.min.z.floor() as i32)..=(hitbox.max.z.floor() as i32) {
                    if let Some(block) = self.get_block_by_coordinates(&BlockPos::overworld(x, y, z)) {
                        match block.get_hitbox() {
                            BlockHitbox::FullBlock => return true,
                            BlockHitbox::None => {
                                // Check if this is a water block and if we should check dynamic surface
                                if matches!(block, BlockId::Water) {
                                    if let Some(get_height) = water_height_fn {
                                        // Check if hitbox intersects with dynamic water surface
                                        let water_height =
                                            get_height(x as f32 + 0.5, z as f32 + 0.5);
                                        if hitbox.min.y <= water_height
                                            && hitbox.max.y >= water_height
                                        {
                                            // Consider this a collision if the object is moving downward into water
                                            return true;
                                        }
                                    }
                                }
                                continue;
                            }
                            BlockHitbox::Aabb(block_hitbox) => {
                                let min = hitbox.min.max(block_hitbox.min);
                                let max = hitbox.max.min(block_hitbox.max);

                                if min == max.min(min) {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }
        false
    }

    fn mark_block_for_update(&mut self, position: &BlockPos);
}

/// Global trait for all numerical enums serving as unique IDs for certain
/// types of elements in the game. Example : ItemId, BlockId...
/// Used in texture atlases and such
pub trait GameElementId: std::hash::Hash + Eq + PartialEq + Copy + Clone + Default + Debug {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sent_to_clients_deduplicates_players() {
        let mut chunk = ServerChunk::default();
        chunk.sent_to_clients.insert(1);
        chunk.sent_to_clients.insert(1);

        assert_eq!(chunk.sent_to_clients.len(), 1);
    }
}
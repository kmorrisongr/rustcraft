use crate::messages::PlayerId;
use crate::players::Player;
use crate::world::{block_to_chunk_coord, global_to_chunk_local, BlockHitbox, BlockId};
use bevy::math::{bounding::Aabb3d, IVec3, Vec2, Vec3};
use bevy_ecs::resource::Resource;
use bevy_log::info;
use noiz::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

use super::{BlockData, ItemId, ItemType, MobId, ServerMob};

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

/// Represents a request for flora generation to be fulfilled in a target chunk.
/// This is created when a chunk's top layer (y = CHUNK_SIZE - 1) has a valid
/// surface block and rolls successfully for flora generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloraRequest {
    /// The local x position within the target chunk.
    /// Valid range: 0 to CHUNK_SIZE - 1 (uses i32 for consistency with chunk coordinate types)
    pub local_x: i32,
    /// The local z position within the target chunk.
    /// Valid range: 0 to CHUNK_SIZE - 1 (uses i32 for consistency with chunk coordinate types)
    pub local_z: i32,
    /// The type of flora to generate
    pub flora_type: FloraType,
    /// The biome type where this flora should be generated
    pub biome_type: BiomeType,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ServerItemStack {
    pub id: u128,
    pub despawned: bool,
    pub stack: ItemStack,
    pub pos: Vec3,
    pub timestamp: u64,
}

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct ServerChunk {
    pub map: HashMap<IVec3, BlockData>,
    /// Timestamp marking the last update this chunk has received
    pub ts: u64,
    pub sent_to_clients: HashSet<PlayerId>,
}

// #[derive(Resource)]
// pub struct PlayerInventories(HashMap<PlayerId, Inventory>);

#[derive(Resource, Default, Clone, Serialize, Deserialize, Debug)]
pub struct ServerWorldMap {
    pub name: String,
    pub chunks: ServerChunkWorldMap,
    pub players: HashMap<PlayerId, Player>,
    pub mobs: HashMap<MobId, ServerMob>,
    pub item_stacks: Vec<ServerItemStack>,
    pub time: u64,
}

#[derive(Default, Clone, Serialize, Deserialize, Debug)]
pub struct ServerChunkWorldMap {
    pub map: HashMap<IVec3, ServerChunk>,
    pub chunks_to_update: Vec<IVec3>,
    /// Pending flora generation requests, keyed by the target chunk position.
    /// When a chunk is generated, it checks this map for any pending requests
    /// and processes them before generating its own flora.
    pub generation_requests: HashMap<IVec3, Vec<FloraRequest>>,
}

#[derive(Resource, Clone, Copy, Serialize, Deserialize, Default)]
pub struct WorldSeed(pub u32);

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Default, PartialEq)]
pub struct ItemStack {
    pub item_id: ItemId,
    pub item_type: ItemType,
    pub nb: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BiomeType {
    Plains,
    Forest,
    MediumMountain,
    HighMountainGrass,
    Desert,
    IcePlain,
    FlowerPlains,
    ShallowOcean,
    Ocean,
    DeepOcean,
}

impl BiomeType {
    /// Returns the human-readable name of the biome
    pub fn name(&self) -> &'static str {
        match self {
            BiomeType::Plains => "Plains",
            BiomeType::Forest => "Forest",
            BiomeType::MediumMountain => "Medium Mountain",
            BiomeType::HighMountainGrass => "High Mountain Grass",
            BiomeType::Desert => "Desert",
            BiomeType::IcePlain => "Ice Plain",
            BiomeType::FlowerPlains => "Flower Plains",
            BiomeType::ShallowOcean => "Shallow Ocean",
            BiomeType::Ocean => "Ocean",
            BiomeType::DeepOcean => "Deep Ocean",
        }
    }

    /// Determines the biome type from climate data (temperature and humidity).
    /// This function is used by both server (for world generation) and client (for biome display).
    ///
    /// # Arguments
    /// * `climate` - BiomeClimate struct containing temperature and humidity values (both 0.0 to 1.0)
    ///
    /// # Returns
    /// The biome type corresponding to the given climate
    pub fn from_climate(climate: BiomeClimate) -> Self {
        const OCEAN_PERCENTAGE: f64 = 0.33;
        const DEEP_OCEAN_THRESHOLD: f64 = 1.0 - (OCEAN_PERCENTAGE / 3.0);
        const OCEAN_THRESHOLD: f64 = 1.0 - 2.0 * (OCEAN_PERCENTAGE / 3.0);
        const SHALLOW_OCEAN_THRESHOLD: f64 = 1.0 - OCEAN_PERCENTAGE;
        const LAND_HUMID_THRESHOLD: f64 = SHALLOW_OCEAN_THRESHOLD / 2.0;
        const LAND_HIGH_HUMID_THRESHOLD: f64 = 2.0 * SHALLOW_OCEAN_THRESHOLD / 3.0;
        const LAND_MID_HUMID_THRESHOLD: f64 = SHALLOW_OCEAN_THRESHOLD / 3.0;

        match (climate.temperature, climate.humidity) {
            // Ocean biomes (determined primarily by humidity)
            (_, h) if h > DEEP_OCEAN_THRESHOLD => BiomeType::DeepOcean,
            (_, h) if h > OCEAN_THRESHOLD => BiomeType::Ocean,
            (_, h) if h > SHALLOW_OCEAN_THRESHOLD => BiomeType::ShallowOcean,

            // Land biomes - Hot climate (temperature > 0.6)
            (t, h) if t > 0.6 && h > LAND_HUMID_THRESHOLD => BiomeType::Forest,
            (t, _) if t > 0.6 => BiomeType::Desert,

            // Land biomes - Temperate climate (0.3 < temperature <= 0.6)
            (t, h) if t > 0.3 && h > LAND_HIGH_HUMID_THRESHOLD => BiomeType::FlowerPlains,
            (t, h) if t > 0.3 && h > LAND_MID_HUMID_THRESHOLD => BiomeType::Plains,
            (t, _) if t > 0.3 => BiomeType::MediumMountain,

            // Land biomes - Cold climate (temperature <= 0.3)
            (t, h) if t >= 0.0 && h > LAND_HUMID_THRESHOLD => BiomeType::IcePlain,
            (t, _) if t >= 0.0 => BiomeType::HighMountainGrass,

            _ => panic!(
                "Invalid climate values: temperature={}, humidity={}",
                climate.temperature, climate.humidity
            ),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Biome {
    pub biome_type: BiomeType,
    pub base_height: i32,
    pub height_variation: i32,
    pub surface_block: BlockId,
    pub sub_surface_block: BlockId,
}

pub fn get_biome_data(biome_type: BiomeType) -> Biome {
    match biome_type {
        BiomeType::Plains => Biome {
            biome_type: BiomeType::Plains,
            base_height: 64,
            height_variation: 1,
            surface_block: BlockId::Grass,
            sub_surface_block: BlockId::Dirt,
        },
        BiomeType::Forest => Biome {
            biome_type: BiomeType::Forest,
            base_height: 64,
            height_variation: 2,
            surface_block: BlockId::Grass,
            sub_surface_block: BlockId::Dirt,
        },
        BiomeType::MediumMountain => Biome {
            biome_type: BiomeType::MediumMountain,
            base_height: 70,
            height_variation: 4,
            surface_block: BlockId::Grass,
            sub_surface_block: BlockId::Dirt,
        },
        BiomeType::HighMountainGrass => Biome {
            biome_type: BiomeType::HighMountainGrass,
            base_height: 75,
            height_variation: 7,
            surface_block: BlockId::Grass,
            sub_surface_block: BlockId::Dirt,
        },
        BiomeType::Desert => Biome {
            biome_type: BiomeType::Desert,
            base_height: 64,
            height_variation: 1,
            surface_block: BlockId::Sand,
            sub_surface_block: BlockId::Sand,
        },
        BiomeType::IcePlain => Biome {
            biome_type: BiomeType::IcePlain,
            base_height: 64,
            height_variation: 1,
            surface_block: BlockId::Snow,
            sub_surface_block: BlockId::Ice,
        },
        BiomeType::FlowerPlains => Biome {
            biome_type: BiomeType::FlowerPlains,
            base_height: 64,
            height_variation: 1,
            surface_block: BlockId::Grass,
            sub_surface_block: BlockId::Dirt,
        },
        BiomeType::ShallowOcean => Biome {
            biome_type: BiomeType::ShallowOcean,
            base_height: 60,
            height_variation: 1,
            surface_block: BlockId::Sand,
            sub_surface_block: BlockId::Sand,
        },
        BiomeType::Ocean => Biome {
            biome_type: BiomeType::DeepOcean,
            base_height: 55,
            height_variation: 2,
            surface_block: BlockId::Sand,
            sub_surface_block: BlockId::Sand,
        },
        BiomeType::DeepOcean => Biome {
            biome_type: BiomeType::DeepOcean,
            base_height: 50,
            height_variation: 3,
            surface_block: BlockId::Sand,
            sub_surface_block: BlockId::Sand,
        },
    }
}

/// Temperature and humidity values for biome calculation
#[derive(Debug, Clone, Copy)]
pub struct BiomeClimate {
    /// Temperature value between 0.0 and 1.0
    pub temperature: f64,
    /// Humidity value between 0.0 and 1.0
    pub humidity: f64,
}

/// Calculates the temperature and humidity at a given world position using Perlin noise.
/// This ensures the client and server use identical noise generation parameters.
///
/// # Arguments
/// * `x` - World x coordinate
/// * `z` - World z coordinate
/// * `seed` - World seed
///
/// # Returns
/// A BiomeClimate struct with temperature and humidity values, both between 0.0 and 1.0
#[derive(Clone)]
pub struct ClimateNoises {
    temp: Noise<common_noise::Perlin>,
    humidity: Noise<common_noise::Perlin>,
}

impl ClimateNoises {
    pub fn new(seed: u32) -> Self {
        let mut temp = Noise::<common_noise::Perlin>::default();
        temp.set_seed(seed + TEMP_SEED_OFFSET);

        let mut humidity = Noise::<common_noise::Perlin>::default();
        humidity.set_seed(seed + HUMIDITY_SEED_OFFSET);

        Self { temp, humidity }
    }
}

pub fn calculate_temperature_humidity_with_noises(
    x: i32,
    z: i32,
    noises: &mut ClimateNoises,
) -> BiomeClimate {
    let sample_position = Vec2::new(x as f32 * BIOME_SCALE, z as f32 * BIOME_SCALE);

    let temperature = (noises.temp.sample_for::<f64>(sample_position) + 1.0) / 2.0;
    let humidity = (noises.humidity.sample_for::<f64>(sample_position) + 1.0) / 2.0;

    BiomeClimate {
        temperature,
        humidity,
    }
}

pub fn calculate_temperature_humidity(x: i32, z: i32, seed: u32) -> BiomeClimate {
    let mut noises = ClimateNoises::new(seed);
    calculate_temperature_humidity_with_noises(x, z, &mut noises)
}

/// Calculates the biome at a given world position.
/// This is a convenience function that combines temperature/humidity calculation with biome determination.
///
/// # Arguments
/// * `x` - World x coordinate
/// * `z` - World z coordinate
/// * `seed` - World seed
///
/// # Returns
/// The biome type at the given position
pub fn calculate_biome_at_position(x: i32, z: i32, seed: u32) -> BiomeType {
    let climate = calculate_temperature_humidity(x, z, seed);
    BiomeType::from_climate(climate)
}

pub trait WorldMap {
    fn get_block_mut_by_coordinates(&mut self, position: &IVec3) -> Option<&mut BlockData>;
    fn get_block_by_coordinates(&self, position: &IVec3) -> Option<&BlockData>;
    fn remove_block_by_coordinates(&mut self, global_block_pos: &IVec3) -> Option<BlockData>;
    fn set_block(&mut self, position: &IVec3, block: BlockData);

    /// Check if a chunk at the given chunk position is loaded
    fn has_chunk(&self, chunk_pos: &IVec3) -> bool;

    fn get_height_ground(&self, position: Vec3) -> i32 {
        for y in (0..256).rev() {
            if self
                .get_block_by_coordinates(&IVec3::new(position.x as i32, y, position.z as i32))
                .is_some()
            {
                return y;
            }
        }
        0
    }

    fn check_collision_box(&self, hitbox: &Aabb3d) -> bool {
        // Check all blocks inside the hitbox
        // Manual flooring is needed for negative coordinates
        for x in (hitbox.min.x.floor() as i32)..=(hitbox.max.x.floor() as i32) {
            for y in (hitbox.min.y.floor() as i32)..=(hitbox.max.y.floor() as i32) {
                for z in (hitbox.min.z.floor() as i32)..=(hitbox.max.z.floor() as i32) {
                    if let Some(block) = self.get_block_by_coordinates(&IVec3::new(x, y, z)) {
                        match block.id.get_hitbox() {
                            BlockHitbox::FullBlock => return true,
                            BlockHitbox::None => continue,
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

    fn get_surrounding_chunks(&self, position: Vec3, radius: i32) -> Vec<IVec3> {
        let mut chunks = Vec::new();
        let x = position.x as i32;
        let y = position.y as i32;
        let z = position.z as i32;
        let cx = block_to_chunk_coord(x);
        let cy = block_to_chunk_coord(y);
        let cz = block_to_chunk_coord(z);
        for i in -radius..=radius {
            for j in -radius..=radius {
                for k in -radius..=radius {
                    chunks.push(IVec3::new(cx + i, cy + j, cz + k));
                }
            }
        }
        chunks
    }

    fn mark_block_for_update(&mut self, position: &IVec3);
}

impl WorldMap for ServerChunkWorldMap {
    fn has_chunk(&self, chunk_pos: &IVec3) -> bool {
        self.map.contains_key(chunk_pos)
    }

    fn get_block_mut_by_coordinates(&mut self, position: &IVec3) -> Option<&mut BlockData> {
        let (chunk_pos, local_pos) = global_to_chunk_local(position);
        let chunk = self.map.get_mut(&chunk_pos)?;
        chunk.map.get_mut(&local_pos)
    }

    fn get_block_by_coordinates(&self, position: &IVec3) -> Option<&BlockData> {
        let (chunk_pos, local_pos) = global_to_chunk_local(position);
        let chunk = self.map.get(&chunk_pos)?;
        chunk.map.get(&local_pos)
    }

    fn remove_block_by_coordinates(&mut self, global_block_pos: &IVec3) -> Option<BlockData> {
        info!("Trying to remove block at pos {:?}", global_block_pos);
        let block: &BlockData = self.get_block_by_coordinates(global_block_pos)?;
        let kind: BlockData = *block;

        let (chunk_pos, local_block_pos) = global_to_chunk_local(global_block_pos);
        let chunk_map: &mut ServerChunk = self.map.get_mut(&chunk_pos)?;

        chunk_map.map.remove(&local_block_pos);
        self.chunks_to_update.push(chunk_pos);

        Some(kind)
    }

    fn set_block(&mut self, position: &IVec3, block: BlockData) {
        let (chunk_pos, local_pos) = global_to_chunk_local(position);
        let chunk: &mut ServerChunk = self.map.entry(chunk_pos).or_default();

        chunk.map.insert(local_pos, block);
        self.chunks_to_update.push(chunk_pos);
    }

    fn mark_block_for_update(&mut self, position: &IVec3) {
        let x: i32 = position.x;
        let y: i32 = position.y;
        let z: i32 = position.z;
        let cx: i32 = block_to_chunk_coord(x);
        let cy: i32 = block_to_chunk_coord(y);
        let cz: i32 = block_to_chunk_coord(z);
        self.chunks_to_update.push(IVec3::new(cx, cy, cz));
    }
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

    #[test]
    fn calculate_temperature_humidity_is_deterministic_and_bounded() {
        let first = calculate_temperature_humidity(10, -5, 123);
        let second = calculate_temperature_humidity(10, -5, 123);

        assert!((0.0..=1.0).contains(&first.temperature));
        assert!((0.0..=1.0).contains(&first.humidity));

        assert!((first.temperature - second.temperature).abs() < f32::EPSILON as f64);
        assert!((first.humidity - second.humidity).abs() < f32::EPSILON as f64);
    }
}

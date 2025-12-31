use std::collections::HashMap;

use crate::HALF_BLOCK;

use super::{GameElementId, ItemId};
use bevy::math::{bounding::Aabb3d, IVec3, Vec3, Vec3A};
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialOrd)]
pub struct RayHitboxArgs {
    center: [f32; 3],
    half_size: [f32; 3],
}

impl RayHitboxArgs {
    pub fn short_flower() -> Self {
        RayHitboxArgs {
            center: [0.5, 0.3, 0.5],
            half_size: [0.3, 0.3, 0.3],
        }
    }
}

impl PartialEq for RayHitboxArgs {
    fn eq(&self, other: &Self) -> bool {
        self.center == other.center && self.half_size == other.half_size
    }
}

impl Eq for RayHitboxArgs {}

impl Ord for RayHitboxArgs {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        for i in 0..3 {
            match self.center[i].total_cmp(&other.center[i]) {
                std::cmp::Ordering::Equal => continue,
                ord => return ord,
            }
        }
        for i in 0..3 {
            match self.half_size[i].total_cmp(&other.half_size[i]) {
                std::cmp::Ordering::Equal => continue,
                ord => return ord,
            }
        }
        std::cmp::Ordering::Equal
    }
}

impl std::hash::Hash for RayHitboxArgs {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for f in self.center.iter().chain(self.half_size.iter()) {
            f.to_bits().hash(state);
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DropStatistics {
    relative_chance: u32,
    corresponding_item: ItemId,
    base_number: u32,
}

impl DropStatistics {
    pub fn with_base_chance(corresponding_item: ItemId) -> Self {
        DropStatistics {
            relative_chance: 1,
            corresponding_item,
            base_number: 1,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, PartialOrd, Ord)]
pub struct BlockProperties {
    break_time: u8,
    hitbox: InternalBlockHitbox,
    ray_hitbox_args: Option<RayHitboxArgs>,
    visibility: BlockTransparency,
    drop_table: DropStatistics,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, PartialOrd, Ord)]
pub struct UnbreakableBlockProperties {
    hitbox: InternalBlockHitbox,
    ray_hitbox_args: Option<RayHitboxArgs>,
    visibility: BlockTransparency,
}

impl BlockProperties {
    pub fn full_solid_block(break_time: u8, drop_table: DropStatistics) -> Self {
        BlockProperties {
            break_time,
            hitbox: InternalBlockHitbox::FullBlock,
            ray_hitbox_args: None,
            visibility: BlockTransparency::Solid,
            drop_table,
        }
    }

    pub fn full_solid_base_block(break_time: u8, corresponding_item: ItemId) -> Self {
        BlockProperties::full_solid_block(
            break_time,
            DropStatistics::with_base_chance(corresponding_item),
        )
    }

    pub fn decoration_base_block(
        break_time: u8,
        ray_hitbox_args: RayHitboxArgs,
        corresponding_item: ItemId,
    ) -> Self {
        BlockProperties {
            break_time,
            hitbox: InternalBlockHitbox::None,
            ray_hitbox_args: Some(ray_hitbox_args),
            visibility: BlockTransparency::Decoration,
            drop_table: DropStatistics::with_base_chance(corresponding_item),
        }
    }

    pub fn full_transparent_block(break_time: u8, drop_table: DropStatistics) -> Self {
        BlockProperties {
            break_time,
            hitbox: InternalBlockHitbox::FullBlock,
            ray_hitbox_args: None,
            visibility: BlockTransparency::Transparent,
            drop_table,
        }
    }

    pub fn full_transparent_base_block(break_time: u8, corresponding_item: ItemId) -> Self {
        BlockProperties::full_transparent_block(
            break_time,
            DropStatistics::with_base_chance(corresponding_item),
        )
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    Hash,
    Default,
)]
pub enum BlockId {
    #[default]
    Dirt,
    Debug,
    Grass,
    Stone,
    OakLog,
    OakPlanks,
    OakLeaves,
    Sand,
    Cactus,
    Ice,
    Glass,
    Bedrock,
    Dandelion,
    Poppy,
    TallGrass,
    Cobblestone,
    Snow,
    SpruceLeaves,
    SpruceLog,
    Water,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize, Hash,
)]
pub enum BlockDefinition {
    Debug,
    Dirt(BlockProperties),
    Grass(BlockProperties),
    Stone(BlockProperties),
    OakLog(BlockProperties),
    OakPlanks(BlockProperties),
    OakLeaves(BlockProperties),
    Sand(BlockProperties),
    Cactus(BlockProperties),
    Ice(BlockProperties),
    Glass(BlockProperties),
    Bedrock(BlockProperties),
    Dandelion(BlockProperties),
    Poppy(BlockProperties),
    TallGrass(BlockProperties),
    Cobblestone(BlockProperties),
    Snow(BlockProperties),
    SpruceLeaves(BlockProperties),
    SpruceLog(BlockProperties),
    Water(UnbreakableBlockProperties),
}

pub enum GetPropertiesResult {
    Breakable(BlockProperties),
    Unbreakable(UnbreakableBlockProperties),
    None,
}

impl BlockDefinition {
    pub fn properties(&self) -> GetPropertiesResult {
        match self {
            BlockDefinition::Debug => GetPropertiesResult::None,
            BlockDefinition::Dirt(props)
            | BlockDefinition::Grass(props)
            | BlockDefinition::Stone(props)
            | BlockDefinition::OakLog(props)
            | BlockDefinition::OakPlanks(props)
            | BlockDefinition::OakLeaves(props)
            | BlockDefinition::Sand(props)
            | BlockDefinition::Cactus(props)
            | BlockDefinition::Ice(props)
            | BlockDefinition::Glass(props)
            | BlockDefinition::Bedrock(props)
            | BlockDefinition::Dandelion(props)
            | BlockDefinition::Poppy(props)
            | BlockDefinition::TallGrass(props)
            | BlockDefinition::Cobblestone(props)
            | BlockDefinition::Snow(props)
            | BlockDefinition::SpruceLeaves(props)
            | BlockDefinition::SpruceLog(props) => GetPropertiesResult::Breakable(*props),
            BlockDefinition::Water(props) => GetPropertiesResult::Unbreakable(*props),
        }
    }

    pub fn from_block_id(id: BlockId) -> Self {
        match id {
            BlockId::Debug => BlockDefinition::debug(),
            BlockId::Dirt => BlockDefinition::dirt(),
            BlockId::Grass => BlockDefinition::grass(),
            BlockId::Stone => BlockDefinition::stone(),
            BlockId::OakLog => BlockDefinition::oak_log(),
            BlockId::OakPlanks => BlockDefinition::oak_planks(),
            BlockId::OakLeaves => BlockDefinition::oak_leaves(),
            BlockId::Sand => BlockDefinition::sand(),
            BlockId::Cactus => BlockDefinition::cactus(),
            BlockId::Ice => BlockDefinition::ice(),
            BlockId::Glass => BlockDefinition::glass(),
            BlockId::Bedrock => BlockDefinition::bedrock(),
            BlockId::Dandelion => BlockDefinition::dandelion(),
            BlockId::Poppy => BlockDefinition::poppy(),
            BlockId::TallGrass => BlockDefinition::tall_grass(),
            BlockId::Cobblestone => BlockDefinition::cobblestone(),
            BlockId::Snow => BlockDefinition::snow(),
            BlockId::SpruceLeaves => BlockDefinition::spruce_leaves(),
            BlockId::SpruceLog => BlockDefinition::spruce_log(),
            BlockId::Water => BlockDefinition::water(),
        }
    }

    pub fn debug() -> Self {
        BlockDefinition::Debug
    }

    pub fn dirt() -> Self {
        BlockDefinition::Dirt(BlockProperties::full_solid_base_block(30, ItemId::Dirt))
    }

    pub fn grass() -> Self {
        BlockDefinition::Grass(BlockProperties::full_solid_base_block(36, ItemId::Dirt))
    }

    pub fn stone() -> Self {
        BlockDefinition::Stone(BlockProperties::full_solid_base_block(
            60,
            ItemId::Cobblestone,
        ))
    }

    pub fn oak_log() -> Self {
        BlockDefinition::OakLog(BlockProperties::full_solid_base_block(60, ItemId::OakLog))
    }

    pub fn oak_planks() -> Self {
        BlockDefinition::OakPlanks(BlockProperties::full_solid_base_block(
            60,
            ItemId::OakPlanks,
        ))
    }

    pub fn oak_leaves() -> Self {
        BlockDefinition::OakLeaves(BlockProperties::full_transparent_base_block(
            12,
            ItemId::OakLeaves,
        ))
    }

    pub fn sand() -> Self {
        BlockDefinition::Sand(BlockProperties::full_solid_base_block(30, ItemId::Sand))
    }

    pub fn cactus() -> Self {
        BlockDefinition::Cactus(BlockProperties::full_solid_base_block(24, ItemId::Cactus))
    }

    pub fn ice() -> Self {
        BlockDefinition::Ice(BlockProperties::full_solid_base_block(30, ItemId::Ice))
    }

    pub fn glass() -> Self {
        BlockDefinition::Glass(BlockProperties::full_transparent_base_block(
            18,
            ItemId::Glass,
        ))
    }

    pub fn bedrock() -> Self {
        BlockDefinition::Bedrock(BlockProperties::full_solid_base_block(255, ItemId::Bedrock))
    }

    pub fn dandelion() -> Self {
        BlockDefinition::Dandelion(BlockProperties::decoration_base_block(
            6,
            RayHitboxArgs::short_flower(),
            ItemId::Dandelion,
        ))
    }

    pub fn poppy() -> Self {
        BlockDefinition::Poppy(BlockProperties::decoration_base_block(
            6,
            RayHitboxArgs::short_flower(),
            ItemId::Poppy,
        ))
    }

    pub fn tall_grass() -> Self {
        BlockDefinition::TallGrass(BlockProperties::decoration_base_block(
            6,
            RayHitboxArgs::short_flower(),
            ItemId::TallGrass,
        ))
    }

    pub fn cobblestone() -> Self {
        BlockDefinition::Cobblestone(BlockProperties::full_solid_base_block(
            12,
            ItemId::Cobblestone,
        ))
    }

    pub fn snow() -> Self {
        BlockDefinition::Snow(BlockProperties::full_solid_base_block(54, ItemId::Snowball))
    }

    pub fn spruce_leaves() -> Self {
        BlockDefinition::SpruceLeaves(BlockProperties::full_transparent_base_block(
            12,
            // TODO: spruce leaves item
            ItemId::OakLeaves,
        ))
    }

    pub fn spruce_log() -> Self {
        BlockDefinition::SpruceLog(BlockProperties::full_solid_base_block(
            60,
            ItemId::SpruceLog,
        ))
    }

    pub fn water() -> Self {
        BlockDefinition::Water(UnbreakableBlockProperties {
            hitbox: InternalBlockHitbox::None,
            ray_hitbox_args: None,
            visibility: BlockTransparency::Liquid,
        })
    }
}

impl Default for BlockDefinition {
    fn default() -> Self {
        BlockDefinition::dirt()
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BlockDirection {
    Front,
    Right,
    Back,
    Left,
}

/// Data associated with a given `BlockId`
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockData {
    pub id: BlockId,
    pub direction: BlockDirection,
    pub breaking_progress: u8,
}

impl BlockData {
    pub fn new(id: BlockId, direction: BlockDirection) -> Self {
        BlockData {
            id,
            direction,
            breaking_progress: 0,
        }
    }

    pub fn get_breaking_level(&self) -> u8 {
        ((self.breaking_progress as u16 * 10) / self.id.get_break_time() as u16) as u8
    }
}

pub enum BlockTags {
    Solid,
    Stone,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy, Serialize, Deserialize, Hash, PartialOrd, Ord)]
pub enum BlockTransparency {
    Transparent,
    Liquid,
    Solid,
    Decoration,
}

// Backwards compatability
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, PartialOrd, Ord)]
enum InternalBlockHitbox {
    FullBlock,
    None,
}

impl InternalBlockHitbox {
    pub fn to_public(&self) -> BlockHitbox {
        match *self {
            Self::FullBlock => BlockHitbox::FullBlock,
            Self::None => BlockHitbox::None,
        }
    }
}

pub enum BlockHitbox {
    FullBlock,
    Aabb(Aabb3d),
    None,
}

impl BlockId {
    fn properties(&self) -> GetPropertiesResult {
        BlockDefinition::from_block_id(*self).properties()
    }

    pub fn get_hitbox(&self) -> BlockHitbox {
        match *self {
            Self::Debug => BlockHitbox::FullBlock,
            _ => match self.properties() {
                GetPropertiesResult::Breakable(props) => props.hitbox.to_public(),
                GetPropertiesResult::Unbreakable(props) => props.hitbox.to_public(),
                GetPropertiesResult::None => BlockHitbox::FullBlock,
            },
        }
    }

    pub fn get_ray_hitbox(&self) -> BlockHitbox {
        match *self {
            Self::Debug => BlockHitbox::FullBlock,
            _ => match self.properties() {
                GetPropertiesResult::Breakable(props) => match props.ray_hitbox_args {
                    Some(args) => BlockHitbox::Aabb(Aabb3d::new(
                        Vec3A::from_slice(&args.center),
                        Vec3A::from_slice(&args.half_size),
                    )),
                    None => props.hitbox.to_public(),
                },
                GetPropertiesResult::Unbreakable(_) => BlockHitbox::None,
                GetPropertiesResult::None => BlockHitbox::FullBlock,
            },
        }
    }

    pub fn get_break_time(&self) -> u8 {
        match *self {
            Self::Debug => 42,
            _ => match self.properties() {
                GetPropertiesResult::Breakable(props) => props.break_time,
                GetPropertiesResult::Unbreakable(_) => 255,
                GetPropertiesResult::None => 100,
            },
        }
    }

    pub fn get_color(&self) -> [f32; 4] {
        match *self {
            Self::Grass => [0.1, 1.0, 0.25, 1.],
            _ => [1., 1., 1., 1.],
        }
    }

    pub fn get_drops(&self, nb_drops: u32) -> HashMap<ItemId, u32> {
        let mut drops = HashMap::new();
        let table = self.get_drop_table();

        if table.is_empty() {
            return drops;
        }

        let total = table
            .clone()
            .into_iter()
            .reduce(|a, b| (a.0 + b.0, a.1, a.2))
            .unwrap()
            .0;

        // Choose drop items
        for _ in 0..nb_drops {
            let mut nb = rand::thread_rng().gen_range(0..total);
            for item in table.iter() {
                if nb < item.0 {
                    drops.insert(item.1, *drops.get(&item.1).unwrap_or(&0) + item.2);
                } else {
                    nb -= item.0;
                }
            }
        }
        drops
    }

    pub fn get_drop_table(&self) -> Vec<(u32, ItemId, u32)> {
        match self.properties() {
            GetPropertiesResult::Breakable(props) => vec![(
                props.drop_table.relative_chance,
                props.drop_table.corresponding_item,
                props.drop_table.base_number,
            )],
            _ => vec![],
        }
    }

    pub fn get_tags(&self) -> Vec<BlockTags> {
        match *self {
            BlockId::Stone => vec![BlockTags::Stone, BlockTags::Solid],
            _ => vec![BlockTags::Solid],
        }
    }

    pub fn get_visibility(&self) -> BlockTransparency {
        match *self {
            Self::Debug => BlockTransparency::Solid,
            _ => match self.properties() {
                GetPropertiesResult::Breakable(props) => props.visibility,
                GetPropertiesResult::Unbreakable(props) => props.visibility,
                GetPropertiesResult::None => BlockTransparency::Solid,
            },
        }
    }

    pub fn get_interaction_box(&self, position: &IVec3) -> Aabb3d {
        let pos = Vec3::new(position.x as f32, position.y as f32, position.z as f32);
        match *self {
            Self::Dandelion | Self::Poppy | Self::TallGrass => {
                Aabb3d::new(pos - Vec3::new(0f32, 0.25, 0f32), HALF_BLOCK / 2.0)
            }
            _ => Aabb3d::new(pos, HALF_BLOCK),
        }
    }
}

impl GameElementId for BlockId {}

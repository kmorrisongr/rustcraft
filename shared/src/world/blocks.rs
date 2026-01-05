use std::collections::HashMap;

use crate::HALF_BLOCK;

use super::{GameElementId, ItemId};
use bevy::math::{bounding::Aabb3d, IVec3, Vec3, Vec3A};
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone)]
struct RayHitboxArgs {
    center: [f32; 3],
    half_size: [f32; 3],
}

impl RayHitboxArgs {
    fn short_flower() -> Self {
        RayHitboxArgs {
            center: [0.5, 0.3, 0.5],
            half_size: [0.3, 0.3, 0.3],
        }
    }
}

#[derive(Copy, Clone)]
struct DropStatistics {
    relative_chance: u32,
    corresponding_item: ItemId,
    base_number: u32,
}

impl DropStatistics {
    fn with_base_chance(corresponding_item: ItemId) -> Self {
        DropStatistics {
            relative_chance: 1,
            corresponding_item,
            base_number: 1,
        }
    }
}

#[derive(Copy, Clone)]
struct BlockProperties {
    break_time: u8,
    hitbox: InternalBlockHitbox,
    ray_hitbox_args: Option<RayHitboxArgs>,
    visibility: BlockTransparency,
    drop_table: Option<DropStatistics>,
}

#[derive(Copy, Clone)]
struct UnbreakableBlockProperties {
    hitbox: InternalBlockHitbox,
    ray_hitbox_args: Option<RayHitboxArgs>,
    visibility: BlockTransparency,
}

enum Block {
    Breakable(BlockProperties),
    Unbreakable(UnbreakableBlockProperties),
}

impl Block {
    fn full_solid_block(break_time: u8, drop_table: Option<DropStatistics>) -> Self {
        Block::Breakable(BlockProperties {
            break_time,
            hitbox: InternalBlockHitbox::FullBlock,
            ray_hitbox_args: None,
            visibility: BlockTransparency::Solid,
            drop_table,
        })
    }

    fn full_solid_block_with_multiple_drops(
        break_time: u8,
        corresponding_item: ItemId,
        number_of_items: u32,
    ) -> Self {
        Block::full_solid_block(
            break_time,
            Some(DropStatistics {
                relative_chance: 1,
                corresponding_item,
                base_number: number_of_items,
            }),
        )
    }

    fn full_solid_base_block(break_time: u8, corresponding_item: ItemId) -> Self {
        Block::full_solid_block(
            break_time,
            Some(DropStatistics::with_base_chance(corresponding_item)),
        )
    }

    fn decoration_base_block(
        break_time: u8,
        ray_hitbox_args: RayHitboxArgs,
        corresponding_item: ItemId,
    ) -> Self {
        Block::Breakable(BlockProperties {
            break_time,
            hitbox: InternalBlockHitbox::None,
            ray_hitbox_args: Some(ray_hitbox_args),
            visibility: BlockTransparency::Decoration,
            drop_table: Some(DropStatistics::with_base_chance(corresponding_item)),
        })
    }

    fn full_transparent_block(break_time: u8, drop_table: Option<DropStatistics>) -> Self {
        Block::Breakable(BlockProperties {
            break_time,
            hitbox: InternalBlockHitbox::FullBlock,
            ray_hitbox_args: None,
            visibility: BlockTransparency::Transparent,
            drop_table,
        })
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

static BLOCK_PROPERTIES: once_cell::sync::Lazy<HashMap<BlockId, Block>> =
    once_cell::sync::Lazy::new(|| {
        HashMap::from([
            (
                BlockId::Dirt,
                Block::full_solid_base_block(30, ItemId::Dirt),
            ),
            (
                BlockId::Grass,
                Block::full_solid_base_block(36, ItemId::Dirt),
            ),
            (
                BlockId::Stone,
                Block::full_solid_base_block(60, ItemId::Cobblestone),
            ),
            (
                BlockId::OakLog,
                Block::full_solid_base_block(60, ItemId::OakLog),
            ),
            (
                BlockId::OakPlanks,
                Block::full_solid_base_block(60, ItemId::OakPlanks),
            ),
            (BlockId::OakLeaves, Block::full_transparent_block(12, None)),
            (
                BlockId::Sand,
                Block::full_solid_base_block(30, ItemId::Sand),
            ),
            (
                BlockId::Cactus,
                Block::full_solid_base_block(24, ItemId::Cactus),
            ),
            (BlockId::Ice, Block::full_solid_base_block(30, ItemId::Ice)),
            (BlockId::Glass, Block::full_transparent_block(18, None)),
            (
                BlockId::Bedrock,
                Block::Unbreakable(UnbreakableBlockProperties {
                    hitbox: InternalBlockHitbox::FullBlock,
                    ray_hitbox_args: None,
                    visibility: BlockTransparency::Solid,
                }),
            ),
            (
                BlockId::Dandelion,
                Block::decoration_base_block(6, RayHitboxArgs::short_flower(), ItemId::Dandelion),
            ),
            (
                BlockId::Poppy,
                Block::decoration_base_block(6, RayHitboxArgs::short_flower(), ItemId::Poppy),
            ),
            (
                BlockId::TallGrass,
                Block::decoration_base_block(6, RayHitboxArgs::short_flower(), ItemId::TallGrass),
            ),
            (BlockId::Cobblestone, Block::full_solid_block(12, None)),
            (
                BlockId::Snow,
                Block::full_solid_block_with_multiple_drops(54, ItemId::Snowball, 4),
            ),
            (
                BlockId::SpruceLeaves,
                Block::full_transparent_block(12, None),
            ),
            (
                BlockId::SpruceLog,
                Block::full_solid_base_block(60, ItemId::SpruceLog),
            ),
            (
                BlockId::Water,
                Block::Unbreakable(UnbreakableBlockProperties {
                    hitbox: InternalBlockHitbox::None,
                    ray_hitbox_args: None,
                    visibility: BlockTransparency::Liquid,
                }),
            ),
        ])
    });

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

#[derive(Copy, Clone)]
enum InternalBlockHitbox {
    FullBlock,
    None,
}

impl InternalBlockHitbox {
    fn to_public(&self) -> BlockHitbox {
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
impl BlockHitbox {
    fn from_args(center: [f32; 3], half_size: [f32; 3]) -> Self {
        BlockHitbox::Aabb(Aabb3d::new(
            Vec3A::from_slice(&center),
            Vec3A::from_slice(&half_size),
        ))
    }

    fn get_ray_hitbox_from_block(block: &Block) -> Self {
        match block {
            Block::Breakable(props) => match props.ray_hitbox_args {
                Some(args) => BlockHitbox::from_args(args.center, args.half_size),
                None => props.hitbox.to_public(),
            },
            Block::Unbreakable(props) => match props.ray_hitbox_args {
                Some(args) => BlockHitbox::from_args(args.center, args.half_size),
                None => props.hitbox.to_public(),
            },
        }
    }
}

impl BlockId {
    fn properties(&self) -> Option<&Block> {
        BLOCK_PROPERTIES.get(self)
    }

    pub fn get_hitbox(&self) -> BlockHitbox {
        match *self {
            Self::Debug => BlockHitbox::FullBlock,
            _ => match self.properties() {
                Some(Block::Breakable(props)) => props.hitbox.to_public(),
                Some(Block::Unbreakable(props)) => props.hitbox.to_public(),
                None => BlockHitbox::FullBlock,
            },
        }
    }

    pub fn get_ray_hitbox(&self) -> BlockHitbox {
        match *self {
            Self::Debug => BlockHitbox::FullBlock,
            _ => match self.properties() {
                Some(block) => BlockHitbox::get_ray_hitbox_from_block(block),
                None => BlockHitbox::FullBlock,
            },
        }
    }

    pub fn get_break_time(&self) -> u8 {
        match *self {
            Self::Debug => 42,
            _ => match self.properties() {
                Some(Block::Breakable(props)) => props.break_time,
                Some(Block::Unbreakable(_)) => 255,
                None => 100,
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
            Some(Block::Breakable(props)) => match &props.drop_table {
                Some(drop_table) => vec![(
                    drop_table.relative_chance,
                    drop_table.corresponding_item,
                    drop_table.base_number,
                )],
                None => vec![],
            },
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
                Some(Block::Breakable(props)) => props.visibility,
                Some(Block::Unbreakable(props)) => props.visibility,
                None => BlockTransparency::Solid,
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

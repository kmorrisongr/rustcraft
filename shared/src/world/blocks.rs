use std::collections::HashMap;

use super::{GameElementId, ItemId};
use bevy::math::{bounding::Aabb3d, Vec3A};
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

    fn as_vector(&self) -> Vec<(u32, ItemId, u32)> {
        vec![(
            self.relative_chance,
            self.corresponding_item,
            self.base_number,
        )]
    }
}

#[derive(Copy, Clone)]
enum Hitbox {
    Pathable {
        /// Will only be used for raycasting, block does not collide with players
        ray_hitbox: BlockHitbox,
    },
    Solid {
        /// Will be used for both collisions with players and raycasting
        collision_hitbox: BlockHitbox,
    },
}

#[derive(Copy, Clone)]
struct BlockBreakability {
    break_time: u8,
    drop_table: Option<DropStatistics>,
}

#[derive(Copy, Clone)]
struct BlockProperties {
    hitbox: Hitbox,
    visibility: BlockTransparency,
    breakability: Option<BlockBreakability>,
}

impl BlockProperties {
    fn full_solid_block(break_time: u8, drop_table: Option<DropStatistics>) -> Self {
        BlockProperties {
            hitbox: Hitbox::Solid {
                collision_hitbox: BlockHitbox::FullBlock,
            },
            visibility: BlockTransparency::Solid,
            breakability: Some(BlockBreakability {
                break_time,
                drop_table,
            }),
        }
    }

    fn full_solid_block_with_multiple_drops(
        break_time: u8,
        corresponding_item: ItemId,
        number_of_items: u32,
    ) -> Self {
        BlockProperties::full_solid_block(
            break_time,
            Some(DropStatistics {
                relative_chance: 1,
                corresponding_item,
                base_number: number_of_items,
            }),
        )
    }

    fn full_solid_base_block(break_time: u8, corresponding_item: ItemId) -> Self {
        BlockProperties::full_solid_block(
            break_time,
            Some(DropStatistics::with_base_chance(corresponding_item)),
        )
    }

    fn decoration_base_block(
        break_time: u8,
        ray_hitbox_args: RayHitboxArgs,
        corresponding_item: ItemId,
    ) -> Self {
        BlockProperties {
            breakability: Some(BlockBreakability {
                break_time,
                drop_table: Some(DropStatistics::with_base_chance(corresponding_item)),
            }),
            hitbox: Hitbox::Pathable {
                ray_hitbox: BlockHitbox::from_args(
                    ray_hitbox_args.center,
                    ray_hitbox_args.half_size,
                ),
            },
            visibility: BlockTransparency::Decoration,
        }
    }

    fn full_transparent_block(break_time: u8, drop_table: Option<DropStatistics>) -> Self {
        BlockProperties {
            hitbox: Hitbox::Solid {
                collision_hitbox: BlockHitbox::FullBlock,
            },
            visibility: BlockTransparency::Transparent,
            breakability: Some(BlockBreakability {
                break_time,
                drop_table,
            }),
        }
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

static BLOCK_PROPERTIES: once_cell::sync::Lazy<HashMap<BlockId, BlockProperties>> =
    once_cell::sync::Lazy::new(|| {
        HashMap::from([
            (
                BlockId::Dirt,
                BlockProperties::full_solid_base_block(30, ItemId::Dirt),
            ),
            (
                BlockId::Grass,
                BlockProperties::full_solid_base_block(36, ItemId::Dirt),
            ),
            (
                BlockId::Stone,
                BlockProperties::full_solid_base_block(60, ItemId::Cobblestone),
            ),
            (
                BlockId::OakLog,
                BlockProperties::full_solid_base_block(60, ItemId::OakLog),
            ),
            (
                BlockId::OakPlanks,
                BlockProperties::full_solid_base_block(60, ItemId::OakPlanks),
            ),
            (
                BlockId::OakLeaves,
                BlockProperties::full_transparent_block(12, None),
            ),
            (
                BlockId::Sand,
                BlockProperties::full_solid_base_block(30, ItemId::Sand),
            ),
            (
                BlockId::Cactus,
                BlockProperties::full_solid_base_block(24, ItemId::Cactus),
            ),
            (
                BlockId::Ice,
                BlockProperties::full_solid_base_block(30, ItemId::Ice),
            ),
            (
                BlockId::Glass,
                BlockProperties::full_transparent_block(18, None),
            ),
            (
                BlockId::Bedrock,
                BlockProperties {
                    breakability: None,
                    hitbox: Hitbox::Solid {
                        collision_hitbox: BlockHitbox::FullBlock,
                    },
                    visibility: BlockTransparency::Solid,
                },
            ),
            (
                BlockId::Dandelion,
                BlockProperties::decoration_base_block(
                    6,
                    RayHitboxArgs::short_flower(),
                    ItemId::Dandelion,
                ),
            ),
            (
                BlockId::Poppy,
                BlockProperties::decoration_base_block(
                    6,
                    RayHitboxArgs::short_flower(),
                    ItemId::Poppy,
                ),
            ),
            (
                BlockId::TallGrass,
                BlockProperties::decoration_base_block(
                    6,
                    RayHitboxArgs::short_flower(),
                    ItemId::TallGrass,
                ),
            ),
            (
                BlockId::Cobblestone,
                BlockProperties::full_solid_block(12, None),
            ),
            (
                BlockId::Snow,
                BlockProperties::full_solid_block_with_multiple_drops(54, ItemId::Snowball, 4),
            ),
            (
                BlockId::SpruceLeaves,
                BlockProperties::full_transparent_block(12, None),
            ),
            (
                BlockId::SpruceLog,
                BlockProperties::full_solid_base_block(60, ItemId::SpruceLog),
            ),
            (
                BlockId::Water,
                BlockProperties {
                    breakability: None,
                    hitbox: Hitbox::Pathable {
                        ray_hitbox: BlockHitbox::None,
                    },
                    visibility: BlockTransparency::Liquid,
                },
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

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum BlockTransparency {
    Transparent,
    Liquid,
    Solid,
    Decoration,
}

#[derive(Copy, Clone)]
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
}

impl BlockId {
    fn properties(&self) -> Option<&BlockProperties> {
        BLOCK_PROPERTIES.get(self)
    }

    pub fn get_hitbox(&self) -> BlockHitbox {
        match *self {
            Self::Debug => BlockHitbox::FullBlock,
            _ => match self.properties() {
                Some(BlockProperties { hitbox, .. }) => match hitbox {
                    Hitbox::Pathable { ray_hitbox } => *ray_hitbox,
                    Hitbox::Solid { collision_hitbox } => *collision_hitbox,
                },
                None => BlockHitbox::FullBlock,
            },
        }
    }

    pub fn get_ray_hitbox(&self) -> BlockHitbox {
        // NOTE: for now, leave this as backwards-compatability. I am having trouble imagining a
        // use-case for having two separate hitboxes.
        return BlockId::get_hitbox(self);
    }

    pub fn get_break_time(&self) -> u8 {
        match *self {
            Self::Debug => 42,
            _ => self
                .properties()
                .and_then(|props| props.breakability)
                .and_then(|b| Some(b.break_time))
                // TODO: unbreakable should return None
                .unwrap_or(255),
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
        return self
            .properties()
            .and_then(|props| props.breakability)
            .and_then(|breakability| breakability.drop_table)
            .and_then(|drop_table| drop_table.as_vector().into())
            .unwrap_or(vec![]);
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
                Some(BlockProperties { visibility, .. }) => *visibility,
                None => BlockTransparency::Solid,
            },
        }
    }
}

impl GameElementId for BlockId {}

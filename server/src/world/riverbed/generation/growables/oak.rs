use shared::world::{BlockPos, blocks::blocks::BlockId};

use crate::world::riverbed::VoxelWorld;

use super::utils::leaf_disk;

pub fn grow_oak(world: &VoxelWorld, pos: BlockPos, _seed: i32, dist: f32) {
    let height = 12-(dist*7.) as i32;
    let mut pos = pos;
    for _ in 0..height {
        world.set_block(pos, BlockId::OakLog);
        pos.y += 1;
    }

    pos.y -= 2;
    leaf_disk(world, pos, 2, BlockId::OakLeaves);
    pos.y += 1;
    leaf_disk(world, pos, height as u32-2, BlockId::OakLeaves);
    pos.y += 1;
    leaf_disk(world, pos, height as u32-3, BlockId::OakLeaves);
    if height >= 5 {
        pos.y += 1;
        leaf_disk(world, pos, height as u32-4, BlockId::OakLeaves);
    }
    if height >= 6 {
        pos.y += 1;
        leaf_disk(world, pos, height as u32-5, BlockId::OakLeaves);
    }
}

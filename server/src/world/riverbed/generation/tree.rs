use std::ops::Range;
use shared::world::{BlockPos, Tree};
use crate::world::riverbed::{VoxelWorld, generation::growables::*};


pub trait Growable {
    fn grow(&self, world: &VoxelWorld, pos: BlockPos, seed: i32, dist: f32);
}

impl Growable for Tree {
    fn grow(&self, world: &VoxelWorld, pos: BlockPos, seed: i32, dist: f32) {
        if !world.get_block_safe(pos).map(is_fertile_soil()).unwrap_or(false) { return; }
        match self {
            Tree::Spruce => grow_spruce(world, pos, seed, dist),
            Tree::Oak | Tree::Chestnut | Tree::Ironwood => grow_oak(world, pos, seed, dist),
            _ => {}
        }
    }
}

pub type Trees = Vec<([Range<f32>; 4], Tree)>;

mod load_area;
mod block_entities;
mod voxel_world;
mod terrain_thread;
mod realm;
mod chunk;
mod utils;
pub use realm::*;
pub use voxel_world::*;
pub use chunk::*;
pub use block_entities::BlockEntities;
use bevy::prelude::*;
use crate::world::riverbed::{block_entities::unload_block_entities, terrain_thread::{assign_player_col, on_unload_col, send_player_pos_update, setup_load_thread}};

#[derive(Component, Default)]
pub struct PlayerCol(pub ColPos);

#[derive(Message)]
pub struct ColUnloadEvent(pub ColPos);

#[derive(Message)]
pub struct ChunkChanged(pub ChunkPos);

pub struct TerrainLoadPlugin;

impl Plugin for TerrainLoadPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
		app
			.add_message::<ColUnloadEvent>()
			.insert_resource(BlockEntities::default())
			.add_systems(Startup, setup_load_thread)
			.add_systems(Update, send_player_pos_update)
			.add_systems(Update, assign_player_col)
			.add_systems(Update, on_unload_col)
			.add_systems(Update, unload_block_entities)
		;
	}
}
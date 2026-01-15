pub mod celestial;
pub mod data;
pub mod rendering;
pub mod time;

use std::collections::HashMap;

pub use data::*;
pub use rendering::*;

use bevy::prelude::*;
use bevy_sun_move::SunMovePlugin;
use shared::{sets::GameSets, world::WorldSeed};

use crate::{
    camera::spawn_camera,
    world::{
        celestial::{setup_camera_atmosphere, setup_sun_and_sky},
        time::{time_update_system, ClientTime},
    },
    GameState,
};

#[derive(Resource, Default)]
pub struct FirstChunkReceived(pub bool);

pub struct WorldPlugin;
impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SunMovePlugin)
            .init_resource::<WorldSeed>()
            .init_resource::<ClientTime>()
            .init_resource::<FirstChunkReceived>()
            .init_resource::<ClientWorldMap>()
            .add_systems(
                OnEnter(GameState::Game),
                (spawn_camera, setup_sun_and_sky)
                    .chain()
                    .in_set(GameSets::OnEnter::Initialize),
            )
            .add_systems(
                Update,
                setup_camera_atmosphere.in_set(GameSets::Update::WorldInput),
            )
            .add_systems(
                FixedPostUpdate,
                time_update_system.in_set(GameSets::FixedPostUpdate::WorldTime),
            )
            .add_systems(
                OnExit(GameState::Game),
                (clear_resources).in_set(GameSets::OnExit::World),
            )
            .add_message::<WorldRenderRequestUpdateEvent>();
    }
}

fn clear_resources(mut world_map: ResMut<ClientWorldMap>) {
    world_map.map = HashMap::new();
    world_map.total_blocks_count = 0;
    world_map.total_chunks_count = 0;
    world_map.name = "".into();
    world_map.mark_dirty();
}

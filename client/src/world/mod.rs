pub mod celestial;
pub mod data;
pub mod rendering;
pub mod time;

pub use data::*;
pub use rendering::*;

use bevy::prelude::*;
use shared::sets::GameOnEnterSet;

use crate::{camera::spawn_camera, world::celestial::setup_main_lighting, GameState};

#[derive(Resource)]
pub struct FirstChunkReceived(pub bool);

pub struct WorldPlugin;
impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::Game),
            (spawn_camera, setup_main_lighting)
                .chain()
                .in_set(GameOnEnterSet::Initialize),
        );
    }
}

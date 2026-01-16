mod controller;
pub mod flashlight;
mod interactions;
mod labels;
mod update;

pub use controller::*;
pub use interactions::*;
pub use labels::*;
pub use update::*;

use bevy::prelude::*;
use shared::sets::GameSets;

use crate::GameState;

pub struct PlayerPlugin;
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PlayerInteractionsPlugin)
            .add_plugins(PlayerControllerPlugin)
            // Flashlight systems
            .add_systems(OnEnter(GameState::Game), flashlight::setup_flashlight)
            .add_systems(
                Update,
                (
                    flashlight::toggle_flashlight,
                    flashlight::update_flashlight_transform,
                )
                    .chain()
                    .in_set(GameSets::Update::PlayerInput),
            );
    }
}

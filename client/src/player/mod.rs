mod controller;
mod interactions;
mod labels;
mod update;

pub use controller::*;
pub use interactions::*;
pub use labels::*;
pub use update::*;

use bevy::prelude::*;

pub struct PlayerPlugin;
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PlayerInteractionsPlugin)
            .add_plugins(PlayerControllerPlugin);
    }
}

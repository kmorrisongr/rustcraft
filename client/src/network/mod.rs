pub mod buffered_client;
mod chat;
mod cleanup;
pub mod extensions;
mod inputs;
pub mod save;
mod setup;
mod world;

pub use chat::*;
pub use cleanup::*;
pub use extensions::SendGameMessageExtension;
pub use inputs::*;
pub use setup::*;

use bevy::prelude::*;
use shared::sets::GameSet;

use crate::{
    network::buffered_client::{CurrentFrameInputs, PlayerTickInputsBuffer, SyncTime},
    GameState,
};

pub struct NetworkPlugin;
impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentPlayerProfile>()
            .init_resource::<PlayerTickInputsBuffer>()
            .init_resource::<CurrentFrameInputs>()
            .init_resource::<SyncTime>()
            .init_resource::<UnacknowledgedInputs>()
            .add_systems(
                OnEnter(GameState::PreGameLoading),
                (launch_local_server_system, init_server_connection)
                    .chain()
                    .in_set(GameSet::Networking),
            )
            .add_systems(
                Update,
                (establish_authenticated_connection_to_server,).in_set(GameSet::Networking),
            )
            .add_systems(
                Update,
                (network_failure_handler).in_set(GameSet::Networking),
            )
            .add_systems(
                FixedPreUpdate,
                (poll_network_messages).in_set(GameSet::Networking),
            )
            .add_systems(
                FixedUpdate,
                (upload_player_inputs_system).in_set(GameSet::Networking),
            )
            .add_systems(
                OnExit(GameState::Game),
                (terminate_server_connection).in_set(GameSet::Networking),
            );
    }
}

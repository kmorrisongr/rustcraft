use crate::entities::stack::stack_update_system;
use crate::mob::MobPlugin;
use crate::player::{spawn_players_system, PlayerPlugin};
use crate::shaders::{WaterPlugin, WaterSettings};
use crate::ui::menus::{setup_server_connect_loading_screen, update_server_connect_loading_screen};
use crate::ui::PlayerUiPlugin;
use crate::world::time::time_update_system;
use crate::world::{RenderingPlugin, WorldPlugin};
use bevy::prelude::*;
use bevy_atmosphere::prelude::*;
use shared::messages::mob::MobUpdateEvent;
use shared::messages::{ItemStackUpdateEvent, PlayerSpawnEvent, PlayerUpdateEvent};
use shared::physics::RustcraftPhysicsPlugin;
use shared::players::{Inventory, ViewMode};
use shared::sets::GameSet;
use shared::TICKS_PER_SECOND;

use bevy::color::palettes::basic::WHITE;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};

use crate::ui::hud::inventory::*;

use crate::network::{NetworkPlugin, TargetServer, TargetServerState};

use shared::game_state::GameState;

/// Tracks texture atlas loading progress
#[derive(Resource, Default)]
pub struct TextureLoadingState {
    pub loaded: bool,
    pub empty_handles_warning_emitted: bool,
}

pub fn game_plugin(app: &mut App) {
    // Configure system set ordering for PreGameLoading state
    app.configure_sets(
        OnEnter(GameState::PreGameLoading),
        (
            GameSet::Initialize,
            GameSet::Networking.after(GameSet::Initialize),
            GameSet::Resources.after(GameSet::Networking),
            GameSet::Ui.after(GameSet::Resources),
        ),
    )
    .configure_sets(
        Update,
        (
            GameSet::Initialize,
            GameSet::Networking.after(GameSet::Initialize),
        )
            .run_if(in_state(GameState::PreGameLoading)),
    )
    // Configure system set ordering for Game state
    .configure_sets(
        OnEnter(GameState::Game),
        (GameSet::Initialize, GameSet::Ui.after(GameSet::Initialize)),
    )
    .configure_sets(
        PreUpdate,
        (GameSet::PlayerInput,).run_if(in_state(GameState::Game)),
    )
    .configure_sets(
        Update,
        (
            GameSet::PlayerInput,
            GameSet::PlayerPhysics.after(GameSet::PlayerInput),
            GameSet::WorldInput.after(GameSet::PlayerPhysics),
            GameSet::WorldPhysics.after(GameSet::WorldInput),
            GameSet::Networking.after(GameSet::WorldPhysics),
            GameSet::Rendering.after(GameSet::Networking),
            GameSet::Ui.after(GameSet::Rendering),
        )
            .run_if(in_state(GameState::Game)),
    )
    .configure_sets(
        FixedPreUpdate,
        (GameSet::Networking,).run_if(in_state(GameState::Game)),
    )
    .configure_sets(
        FixedUpdate,
        (GameSet::Networking,).run_if(in_state(GameState::Game)),
    )
    .configure_sets(
        PostUpdate,
        (GameSet::Rendering,).run_if(in_state(GameState::Game)),
    )
    .configure_sets(
        OnExit(GameState::Game),
        (
            GameSet::Cleanup,
            GameSet::Networking.after(GameSet::Cleanup),
        ),
    );

    app.add_plugins(PlayerUiPlugin)
        .add_plugins(WorldPlugin)
        .add_plugins(RenderingPlugin)
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(WireframePlugin::default())
        .add_plugins(bevy_simple_text_input::TextInputPlugin)
        .add_plugins(AtmospherePlugin)
        .add_plugins(MobPlugin)
        .add_plugins(RustcraftPhysicsPlugin)
        .add_plugins(NetworkPlugin)
        .add_plugins(PlayerPlugin)
        .insert_resource(WaterSettings {
            height: 0.0,       // Sea level for voxel world
            amplitude: 0.2,    // Gentle waves for block-based water
            spawn_tiles: None, // Don't spawn automatic water tiles (we use chunk meshes)
            ..default()
        })
        .add_plugins(WaterPlugin)
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 400.0,
            ..default()
        })
        .insert_resource(TextureLoadingState::default())
        .insert_resource(WireframeConfig {
            // The global wireframe config enables drawing of wireframes on every mesh,
            // except those with `NoWireframe`. Meshes with `Wireframe` will always have a wireframe,
            // regardless of the global configuration.
            global: false,
            // Controls the default color of all wireframes. Used as the default color for global wireframes.
            // Can be changed per mesh using the `WireframeColor` component.
            default_color: WHITE.into(),
        })
        .insert_resource(UIMode::Closed)
        .insert_resource(ViewMode::FirstPerson)
        .insert_resource(Inventory::new())
        .insert_resource(Time::<Fixed>::from_hz(TICKS_PER_SECOND as f64))
        .add_event::<PlayerSpawnEvent>()
        .add_event::<PlayerUpdateEvent>()
        .add_event::<MobUpdateEvent>()
        .add_event::<ItemStackUpdateEvent>()
        .add_systems(
            OnEnter(GameState::PreGameLoading),
            (
                reset_texture_loading_state,
                setup_server_connect_loading_screen,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                advance_to_game_when_ready,
                spawn_players_system,
                update_server_connect_loading_screen,
            )
                .run_if(in_state(GameState::PreGameLoading)),
        )
        .add_systems(
            Update,
            (stack_update_system,).run_if(in_state(GameState::Game)),
        )
        .add_systems(
            FixedPostUpdate,
            time_update_system.run_if(in_state(GameState::Game)),
        );
}

fn reset_texture_loading_state(mut loading: ResMut<TextureLoadingState>) {
    *loading = TextureLoadingState::default();
}

fn advance_to_game_when_ready(
    texture_state: Res<TextureLoadingState>,
    target_server: Res<TargetServer>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    let textures_ready = texture_state.loaded;
    let server_ready = target_server.state == TargetServerState::FullyReady;

    if textures_ready && server_ready {
        game_state.set(GameState::Game);
    }
}

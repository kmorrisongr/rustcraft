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
use shared::sets::{
    GameFixedPreUpdateSet, GameFixedUpdateSet, GameOnEnterSet, GameOnExitSet, GamePostUpdateSet,
    GamePreUpdateSet, GameUpdateSet, PreGameLoadingOnEnterSet, PreGameLoadingUpdateSet,
};
use shared::TICKS_PER_SECOND;

use bevy::color::palettes::basic::WHITE;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};

use crate::ui::hud::inventory::*;

use crate::network::{NetworkPlugin, TargetServer, TargetServerState};

use crate::GameState;

#[derive(Resource)]
pub struct PreLoadingCompletion {
    pub textures_loaded: bool,
    pub empty_handles_warning_emitted: bool,
}

#[derive(Resource, Default)]
struct PreloadGate {
    textures_ready: bool,
    server_ready: bool,
}

#[derive(Event, Debug, Clone, Copy)]
pub enum PreloadSignal {
    TexturesReady,
    ServerReady,
}

pub fn game_plugin(app: &mut App) {
    app.configure_sets(
        OnEnter(GameState::PreGameLoading),
        (
            PreGameLoadingOnEnterSet::Initialize,
            PreGameLoadingOnEnterSet::Networking.after(PreGameLoadingOnEnterSet::Initialize),
            PreGameLoadingOnEnterSet::Resources.after(PreGameLoadingOnEnterSet::Networking),
            PreGameLoadingOnEnterSet::Ui.after(PreGameLoadingOnEnterSet::Resources),
        ),
    )
    .configure_sets(
        Update,
        (
            PreGameLoadingUpdateSet::Initialize,
            PreGameLoadingUpdateSet::Networking,
            PreGameLoadingUpdateSet::Rest.after(PreGameLoadingUpdateSet::Networking),
        )
            .run_if(in_state(GameState::PreGameLoading)),
    )
    .configure_sets(
        OnEnter(GameState::Game),
        (
            GameOnEnterSet::Initialize,
            GameOnEnterSet::Ui.after(GameOnEnterSet::Initialize),
            GameOnEnterSet::Rest.after(GameOnEnterSet::Ui),
        ),
    )
    .configure_sets(
        PreUpdate,
        (
            GamePreUpdateSet::PlayerInput,
            GamePreUpdateSet::Rest.after(GamePreUpdateSet::PlayerInput),
        )
            .run_if(in_state(GameState::Game)),
    )
    .configure_sets(
        Update,
        (
            GameUpdateSet::PlayerInput,
            GameUpdateSet::PlayerPhysics.after(GameUpdateSet::PlayerInput),
            GameUpdateSet::WorldInput.after(GameUpdateSet::PlayerPhysics),
            GameUpdateSet::WorldPhysics.after(GameUpdateSet::WorldInput),
            GameUpdateSet::Networking.after(GameUpdateSet::WorldPhysics),
            GameUpdateSet::Rendering.after(GameUpdateSet::Networking),
            GameUpdateSet::Ui.after(GameUpdateSet::Rendering),
            GameUpdateSet::Rest.after(GameUpdateSet::Ui),
        )
            .run_if(in_state(GameState::Game)),
    )
    .configure_sets(
        FixedPreUpdate,
        (
            GameFixedPreUpdateSet::Networking,
            GameFixedPreUpdateSet::Rest.after(GameFixedPreUpdateSet::Networking),
        )
            .run_if(in_state(GameState::Game)),
    )
    .configure_sets(
        FixedUpdate,
        (
            GameFixedUpdateSet::Networking,
            GameFixedUpdateSet::Rest.after(GameFixedUpdateSet::Networking),
        )
            .run_if(in_state(GameState::Game)),
    )
    .configure_sets(
        PostUpdate,
        (
            GamePostUpdateSet::Rendering,
            GamePostUpdateSet::Rest.after(GamePostUpdateSet::Rendering),
        )
            .run_if(in_state(GameState::Game)),
    )
    .configure_sets(
        OnExit(GameState::Game),
        (
            GameOnExitSet::World,
            GameOnExitSet::Networking.after(GameOnExitSet::World),
            GameOnExitSet::Rest.after(GameOnExitSet::Networking),
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
        .insert_resource(PreLoadingCompletion {
            textures_loaded: false,
            empty_handles_warning_emitted: false,
        })
        .insert_resource(PreloadGate::default())
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
        .add_event::<PreloadSignal>()
        .add_event::<PlayerSpawnEvent>()
        .add_event::<PlayerUpdateEvent>()
        .add_event::<MobUpdateEvent>()
        .add_event::<ItemStackUpdateEvent>()
        .add_systems(
            OnEnter(GameState::PreGameLoading),
            (reset_preload_tracking, setup_server_connect_loading_screen).chain(),
        )
        .add_systems(
            Update,
            (
                emit_server_ready_signal,
                advance_to_game_on_preload,
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

fn reset_preload_tracking(
    mut loading: ResMut<PreLoadingCompletion>,
    mut gate: ResMut<PreloadGate>,
) {
    loading.textures_loaded = false;
    gate.textures_ready = false;
    gate.server_ready = false;
}

fn emit_server_ready_signal(
    target_server: Res<TargetServer>,
    mut signals: EventWriter<PreloadSignal>,
    mut gate: ResMut<PreloadGate>,
) {
    if !gate.server_ready && target_server.state == TargetServerState::FullyReady {
        signals.write(PreloadSignal::ServerReady);
        gate.server_ready = true;
    }
}

fn advance_to_game_on_preload(
    mut signals: EventReader<PreloadSignal>,
    mut gate: ResMut<PreloadGate>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    for signal in signals.read() {
        match signal {
            PreloadSignal::TexturesReady => gate.textures_ready = true,
            PreloadSignal::ServerReady => {} // Gate updated in emit_server_ready_signal
        }
    }

    if gate.textures_ready && gate.server_ready {
        game_state.set(GameState::Game);
    }
}

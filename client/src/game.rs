use crate::entities::stack::stack_update_system;
use crate::mob::MobPlugin;
use crate::player::PlayerPlugin;
use crate::shaders::{WaterPlugin, WaterSettings};
use crate::ui::PlayerUiPlugin;
use crate::world::{MaterialsPlugin, RenderingPlugin, WorldPlugin};
use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_atmosphere::prelude::*;
use iyes_progress::prelude::*;
use shared::messages::mob::MobUpdateEvent;
use shared::messages::{ItemStackUpdateEvent, PlayerSpawnEvent, PlayerUpdateEvent};
use shared::physics::RustcraftPhysicsPlugin;
use shared::players::{Inventory, ViewMode};
use shared::sets::{GameLoadingSets, GameSets, PreGameLoadingSets};
use shared::TICKS_PER_SECOND;

use bevy::color::palettes::basic::WHITE;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};

use crate::network::NetworkPlugin;

use shared::game_state::GameState;

pub fn game_plugin(app: &mut App) {
    configure_sets(app);
    app.add_plugins(
        ProgressPlugin::<GameState>::new()
            .with_state_transition(GameState::PreGameLoading, GameState::GameLoading),
    )
    .add_loading_state(
        LoadingState::new(GameState::PreGameLoading).continue_to_state(GameState::GameLoading),
    )
    .add_loading_state(LoadingState::new(GameState::GameLoading).continue_to_state(GameState::Game))
    .add_plugins(PlayerUiPlugin)
    .add_plugins(WorldPlugin)
    .add_plugins(RenderingPlugin)
    .add_plugins(MaterialsPlugin)
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
    .insert_resource(WireframeConfig {
        // The global wireframe config enables drawing of wireframes on every mesh,
        // except those with `NoWireframe`. Meshes with `Wireframe` will always have a wireframe,
        // regardless of the global configuration.
        global: false,
        // Controls the default color of all wireframes. Used as the default color for global wireframes.
        // Can be changed per mesh using the `WireframeColor` component.
        default_color: WHITE.into(),
    })
    .insert_resource(ViewMode::FirstPerson)
    .insert_resource(Inventory::new())
    .insert_resource(Time::<Fixed>::from_hz(TICKS_PER_SECOND as f64))
    .add_event::<PlayerSpawnEvent>()
    .add_event::<PlayerUpdateEvent>()
    .add_event::<MobUpdateEvent>()
    .add_event::<ItemStackUpdateEvent>()
    .add_systems(
        Update,
        (stack_update_system,).run_if(in_state(GameState::Game)),
    );
}

fn configure_sets(app: &mut App) {
    app.configure_sets(
        OnEnter(GameState::PreGameLoading),
        PreGameLoadingSets::OnEnter::chained_schedule_configs(),
    )
    .configure_sets(
        Update,
        PreGameLoadingSets::Update::chained_schedule_configs()
            .run_if(in_state(GameState::PreGameLoading)),
    )
    .configure_sets(
        Update,
        GameLoadingSets::Update::chained_schedule_configs()
            .run_if(in_state(GameState::GameLoading)),
    )
    .configure_sets(
        OnEnter(GameState::Game),
        GameSets::OnEnter::chained_schedule_configs(),
    )
    .configure_sets(
        PreUpdate,
        GameSets::PreUpdate::chained_schedule_configs().run_if(in_state(GameState::Game)),
    )
    .configure_sets(
        Update,
        GameSets::Update::chained_schedule_configs().run_if(in_state(GameState::Game)),
    )
    .configure_sets(
        FixedPreUpdate,
        GameSets::FixedPreUpdate::chained_schedule_configs().run_if(in_state(GameState::Game)),
    )
    .configure_sets(
        FixedUpdate,
        GameSets::FixedUpdate::chained_schedule_configs().run_if(in_state(GameState::Game)),
    )
    .configure_sets(
        FixedPostUpdate,
        GameSets::FixedPostUpdate::chained_schedule_configs().run_if(in_state(GameState::Game)),
    )
    .configure_sets(
        PostUpdate,
        GameSets::PostUpdate::chained_schedule_configs().run_if(in_state(GameState::Game)),
    )
    .configure_sets(
        OnExit(GameState::Game),
        GameSets::OnExit::chained_schedule_configs(),
    );
}

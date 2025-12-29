use crate::input::data::GameAction;
use crate::input::keyboard::*;
use crate::network::buffered_client::{
    CurrentFrameInputs, CurrentFrameInputsExt, PlayerTickInputsBuffer, SyncTime, SyncTimeExt,
};
use crate::ui::hud::debug::DebugOptions;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::UIMode;
use crate::world::{ClientWorldMap, WorldRenderRequestUpdateEvent};
use crate::KeyMap;
use bevy::prelude::*;
use shared::messages::NetworkAction;
use shared::players::movement::simulate_player_movement;
use shared::players::{Player, ViewMode};

use super::CurrentPlayerMarker;

/// Maps continuous movement inputs to their network equivalents.
/// Excludes one-shot actions (e.g. toggles) that rely on `is_action_just_pressed`
/// and UI mode checks handled separately in `player_movement_system`.
const ACTION_MAPPING: &[(GameAction, NetworkAction)] = &[
    (GameAction::MoveBackward, NetworkAction::MoveBackward),
    (GameAction::MoveForward, NetworkAction::MoveForward),
    (GameAction::MoveLeft, NetworkAction::MoveLeft),
    (GameAction::MoveRight, NetworkAction::MoveRight),
    (GameAction::Jump, NetworkAction::JumpOrFlyUp),
    (GameAction::FlyDown, NetworkAction::SneakOrFlyDown),
];

pub fn update_frame_inputs_system(
    camera: Query<&Transform, With<Camera>>,
    hotbar: Query<&Hotbar>,
    mut frame_inputs: ResMut<CurrentFrameInputs>,
    view_mode: Res<ViewMode>,
) {
    if frame_inputs.0.delta_ms == 0 {
        return;
    }

    let Ok(camera) = camera.single() else {
        debug!("Camera not found");
        return;
    };
    let Ok(hotbar) = hotbar.single() else {
        debug!("Hotbar not found");
        return;
    };

    frame_inputs.0.camera = *camera;
    frame_inputs.0.hotbar_slot = hotbar.selected;
    frame_inputs.0.view_mode = *view_mode;
}

#[derive(Component)]
pub struct PlayerMaterialHandle {
    pub handle: Handle<StandardMaterial>,
}

pub fn pre_input_update_system(
    mut frame_inputs: ResMut<CurrentFrameInputs>,
    mut tick_buffer: ResMut<PlayerTickInputsBuffer>,
    mut sync_time: ResMut<SyncTime>,
) {
    sync_time.advance();

    let inputs_of_last_frame = frame_inputs.0.clone();
    tick_buffer.buffer.push(inputs_of_last_frame);
    frame_inputs.reset(sync_time.curr_time_ms, sync_time.delta());
}

pub fn player_movement_system(
    queries: Query<(&mut Player, &mut Transform), (With<CurrentPlayerMarker>, Without<Camera>)>,
    resources: (
        Res<ButtonInput<KeyCode>>,
        Res<UIMode>,
        Res<KeyMap>,
        ResMut<CurrentFrameInputs>,
    ),
    world_map: Res<ClientWorldMap>,
) {
    let mut player_query = queries;
    let (keyboard_input, ui_mode, key_map, mut frame_inputs) = resources;

    if frame_inputs.0.delta_ms == 0 {
        return;
    }

    let Ok((mut player, mut player_transform)) = player_query.single_mut() else {
        debug!("player not found");
        return;
    };

    if *ui_mode == UIMode::Closed
        && is_action_just_pressed(GameAction::ToggleFlyMode, &keyboard_input, &key_map)
    {
        frame_inputs.0.inputs.insert(NetworkAction::ToggleFlyMode);
    }

    for (game_action, network_action) in ACTION_MAPPING {
        if is_action_pressed(*game_action, &keyboard_input, &key_map) {
            frame_inputs.0.inputs.insert(*network_action);
        }
    }

    simulate_player_movement(&mut player, world_map.as_ref(), &frame_inputs.0);

    frame_inputs.0.position = player.position;

    player_transform.translation = player.position;

    // debug!(
    //     "At t={}, player position: {:?}",
    //     frame_inputs.0.time_ms, player.position
    // );
}

pub fn first_and_third_person_view_system(
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut view_mode: ResMut<ViewMode>,
    mut player_query: Query<&mut PlayerMaterialHandle, With<CurrentPlayerMarker>>,
    key_map: Res<KeyMap>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    ui_mode: Res<UIMode>,
) {
    if *ui_mode == UIMode::Closed
        && is_action_just_pressed(GameAction::ToggleViewMode, &keyboard_input, &key_map)
    {
        view_mode.toggle();
    }

    let Ok(material_handle) = player_query.single_mut() else {
        debug!("player not found");
        return;
    };

    let material_handle = &material_handle.handle;

    match *view_mode {
        ViewMode::FirstPerson => {
            // make player transparent
            if let Some(material) = materials.get_mut(material_handle) {
                material.base_color = Color::srgba(0.0, 0.0, 0.0, 0.0);
            }
        }
        ViewMode::ThirdPerson => {
            if let Some(material) = materials.get_mut(material_handle) {
                material.base_color = Color::srgba(1.0, 0.0, 0.0, 1.0);
            }
        }
    }
}

pub fn toggle_debug_system(
    mut debug_options: ResMut<DebugOptions>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
) {
    const TOGGLES: &[(GameAction, fn(&mut DebugOptions))] = &[
        (
            GameAction::ToggleChunkDebugMode,
            DebugOptions::toggle_chunk_debug_mode,
        ),
        (
            GameAction::ToggleRaycastDebugMode,
            DebugOptions::toggle_raycast_debug_mode,
        ),
    ];

    for (action, toggle_fn) in TOGGLES {
        if is_action_just_pressed(*action, &keyboard_input, &key_map) {
            toggle_fn(&mut debug_options);
        }
    }
}

pub fn chunk_force_reload_system(
    mut world_map: ResMut<ClientWorldMap>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    mut ev_writer: EventWriter<WorldRenderRequestUpdateEvent>,
    mut commands: Commands,
) {
    if is_action_just_pressed(GameAction::ReloadChunks, &keyboard_input, &key_map) {
        for (pos, chunk) in world_map.map.iter_mut() {
            // Despawn the chunk's entity
            if let Some(e) = chunk.entity {
                commands.entity(e).despawn();
                chunk.entity = None;
            }
            // Request a render for this chunk
            ev_writer.write(WorldRenderRequestUpdateEvent::ChunkToReload(*pos));
        }
    }
}

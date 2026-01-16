//! Flashlight system - attaches a SpotLight to the camera that can be toggled on/off.
//!
//! The flashlight follows the camera's direction and provides a cone of light.

use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;

use crate::input::data::GameAction;
use crate::input::GlobalInputManager;
use crate::GameState;

use leafwing_input_manager::prelude::*;

#[derive(Component)]
pub struct PlayerFlashlight;

#[derive(Resource, Default)]
pub struct FlashlightState {
    pub enabled: bool,
}

pub fn setup_flashlight(mut commands: Commands) {
    commands.insert_resource(FlashlightState { enabled: false });

    commands.spawn((
        PlayerFlashlight,
        SpotLight {
            color: Color::srgb(1.0, 0.95, 0.8), // Warm white light
            intensity: 80_000.0 * 100. * 100.,  // Lumens - bright enough to see in dark areas
            range: 500.0 * 10. * 100. * 100.,
            radius: 0.2,
            inner_angle: 0.05, // ~11 degrees inner cone
            outer_angle: 0.6,  // ~29 degrees outer cone (falloff)
            shadows_enabled: true,
            ..default()
        },
        Transform::default(),
        Visibility::Hidden, // Start hidden
        DespawnOnExit(GameState::Game),
    ));
}

pub fn toggle_flashlight(
    action_query: Query<&ActionState<GameAction>, With<GlobalInputManager>>,
    mut flashlight_state: ResMut<FlashlightState>,
    mut flashlight_query: Query<&mut Visibility, With<PlayerFlashlight>>,
) {
    let Ok(action_state) = action_query.single() else {
        return;
    };

    if action_state.just_pressed(&GameAction::ToggleFlashlight) {
        flashlight_state.enabled = !flashlight_state.enabled;

        if let Ok(mut visibility) = flashlight_query.single_mut() {
            *visibility = if flashlight_state.enabled {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
}

pub fn update_flashlight_transform(
    camera_query: Query<&Transform, With<Camera3d>>,
    mut flashlight_query: Query<&mut Transform, (With<PlayerFlashlight>, Without<Camera3d>)>,
    flashlight_state: Res<FlashlightState>,
) {
    // Skip updates if flashlight is off
    if !flashlight_state.enabled {
        return;
    }

    let Ok(camera_transform) = camera_query.single() else {
        return;
    };

    let Ok(mut flashlight_transform) = flashlight_query.single_mut() else {
        return;
    };

    // Offset slightly forward and down to simulate being held
    let offset = camera_transform.forward() * 0.3 + camera_transform.down() * 0.1;
    flashlight_transform.translation = camera_transform.translation + offset;
    flashlight_transform.rotation = camera_transform.rotation;
}

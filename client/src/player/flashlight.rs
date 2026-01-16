//! Flashlight system - attaches a SpotLight to the camera that can be toggled on/off.
//!
//! The flashlight follows the camera's direction and provides a cone of light
//! useful for exploring dark areas or testing the lighting system.

use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;

use crate::input::data::GameAction;
use crate::input::GlobalInputManager;
use crate::GameState;

use leafwing_input_manager::prelude::*;

/// Marker component for the flashlight entity
#[derive(Component)]
pub struct PlayerFlashlight;

/// Resource tracking flashlight state
#[derive(Resource, Default)]
pub struct FlashlightState {
    pub enabled: bool,
}

/// System to spawn the flashlight attached to the camera.
/// The flashlight starts disabled.
pub fn setup_flashlight(mut commands: Commands) {
    commands.insert_resource(FlashlightState { enabled: false });

    // Spawn the flashlight as a SpotLight
    // It will be positioned relative to the camera each frame
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

/// System to toggle flashlight on/off with a key press.
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

        info!(
            "Flashlight {}",
            if flashlight_state.enabled {
                "ON"
            } else {
                "OFF"
            }
        );
    }
}

/// System to update flashlight position and direction to match the camera.
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

    // Position flashlight at camera location, pointing in camera direction
    // Offset slightly forward and down to simulate being held
    let offset = camera_transform.forward() * 0.3 + camera_transform.down() * 0.1;
    flashlight_transform.translation = camera_transform.translation + offset;
    flashlight_transform.rotation = camera_transform.rotation;
}

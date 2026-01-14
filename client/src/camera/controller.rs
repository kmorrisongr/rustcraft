use crate::camera::spawn::{DEFAULT_THIRD_PERSON_RADIUS, FIRST_PERSON_RADIUS, MOUSE_SENSITIVITY};
use crate::player::CurrentPlayerMarker;
use crate::ui::hud::UIMode;
use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use bevy_panorbit_camera::PanOrbitCamera;
use shared::players::ViewMode;

/// Eye height offset for first-person view
const EYE_HEIGHT_OFFSET: f32 = 0.8;

/// System to handle FPS-style camera controls.
/// Uses bevy_panorbit_camera to "cheat" and get first/third person camera easily.
pub fn camera_control_system(
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut mouse_motion: EventReader<MouseMotion>,
    player_query: Query<&Transform, With<CurrentPlayerMarker>>,
    mut camera_query: Query<&mut PanOrbitCamera, With<Camera>>,
    view_mode: Res<ViewMode>,
    ui_mode: Res<UIMode>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };

    let ui_open = *ui_mode != UIMode::Closed;

    // Manage cursor grabbing based on UI state
    if ui_open {
        window.cursor_options.grab_mode = CursorGrabMode::None;
        window.cursor_options.visible = true;
        mouse_motion.clear();
    } else {
        window.cursor_options.grab_mode = CursorGrabMode::Locked;
        window.cursor_options.visible = false;
    }

    // Accumulate mouse delta
    let mut delta = Vec2::ZERO;
    for event in mouse_motion.read() {
        delta += event.delta;
    }

    let Ok(player_transform) = player_query.single() else {
        return;
    };

    for mut camera in camera_query.iter_mut() {
        // Update yaw/pitch from mouse input (FPS-style)
        if !ui_open && (delta.x != 0.0 || delta.y != 0.0) {
            camera.target_yaw -= delta.x * MOUSE_SENSITIVITY;
            camera.target_pitch += delta.y * MOUSE_SENSITIVITY;

            // Clamp pitch to prevent camera flipping
            camera.target_pitch = camera
                .target_pitch
                .clamp(-89.0_f32.to_radians(), 89.0_f32.to_radians());

            camera.force_update = true;
        }

        // Update focus to follow player
        let focus_offset = if *view_mode == ViewMode::FirstPerson {
            Vec3::Y * EYE_HEIGHT_OFFSET
        } else {
            Vec3::ZERO
        };
        camera.target_focus = player_transform.translation + focus_offset;

        // Update radius based on view mode
        let target_radius = match *view_mode {
            ViewMode::FirstPerson => FIRST_PERSON_RADIUS,
            ViewMode::ThirdPerson => DEFAULT_THIRD_PERSON_RADIUS,
        };

        if (camera.target_radius - target_radius).abs() > 0.01 {
            camera.target_radius = target_radius;
            camera.force_update = true;
        }
    }
}

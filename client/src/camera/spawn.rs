use bevy::{prelude::*, state::state_scoped::DespawnOnExit};
use bevy_panorbit_camera::PanOrbitCamera;

use crate::GameState;

pub const DEFAULT_THIRD_PERSON_RADIUS: f32 = 10.0;
pub const FIRST_PERSON_RADIUS: f32 = 0.0;
pub const MOUSE_SENSITIVITY: f32 = 0.003;

#[derive(Bundle)]
struct GameCameraBundle {
    camera: Camera,
    camera_3d: Camera3d,
    projection: Projection,
    transform: Transform,
    pan_orbit_camera: PanOrbitCamera,
    despawn_on_exit: DespawnOnExit<GameState>,
}

pub fn spawn_camera(mut commands: Commands) {
    commands.spawn(GameCameraBundle {
        camera: Camera {
            order: 2,
            ..Camera::default()
        },
        camera_3d: Camera3d::default(),
        projection: Projection::Perspective(PerspectiveProjection {
            fov: f32::to_radians(60.0),
            ..Default::default()
        }),
        transform: Transform::from_translation(Vec3::new(0.0, 5.0, 10.0))
            .looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y),
        pan_orbit_camera: PanOrbitCamera {
            // Start in first-person mode
            radius: Some(FIRST_PERSON_RADIUS),
            target_radius: FIRST_PERSON_RADIUS,
            // Limit pitch to avoid flipping
            pitch_lower_limit: Some(-89.0_f32.to_radians()),
            pitch_upper_limit: Some(89.0_f32.to_radians()),
            // Disable all built-in controls - we handle mouse input directly for FPS-style
            orbit_sensitivity: 0.0,
            zoom_sensitivity: 0.0,
            pan_sensitivity: 0.0,
            touch_enabled: false,
            ..PanOrbitCamera::default()
        },
        despawn_on_exit: DespawnOnExit(GameState::Game),
    });
}

//! Celestial bodies and day/night cycle using Bevy's Atmosphere and bevy_sun_move.
//!
//! This module sets up:
//! - A DirectionalLight as the sun, controlled by bevy_sun_move's SkyCenter
//! - Bevy's built-in Atmosphere component on the camera for realistic sky rendering
//! - A configurable day/night cycle duration
//! - Dynamic ambient light that dims at night

use crate::constants::DAY_DURATION_IN_TICKS;
use crate::GameState;
use bevy::{
    light::light_consts::lux,
    pbr::{Atmosphere, AtmosphereSettings},
    prelude::*,
};
use bevy_sun_move::SkyCenter;
use shared::TICKS_PER_SECOND;

/// Maximum ambient light brightness during the day
const AMBIENT_BRIGHTNESS_DAY: f32 = 400.0;
/// Minimum ambient light brightness at night (moonlight equivalent)
const AMBIENT_BRIGHTNESS_NIGHT: f32 = 50.0;

/// Marker component for the sun entity
#[derive(Component)]
pub struct SunLight;

/// System to set up the sun and sky center for day/night cycles.
/// This spawns the DirectionalLight (sun) and a SkyCenter entity to control it.
pub fn setup_sun_and_sky(mut commands: Commands) {
    // Calculate cycle duration in seconds from tick-based duration
    // DAY_DURATION_IN_TICKS is the full day cycle, TICKS_PER_SECOND converts to real time
    let cycle_duration_secs = DAY_DURATION_IN_TICKS as f32 / TICKS_PER_SECOND as f32;

    // Spawn the sun (DirectionalLight)
    // Using RAW_SUNLIGHT illuminance as recommended for use with Atmosphere
    let sun_entity = commands
        .spawn((
            SunLight,
            DirectionalLight {
                illuminance: lux::RAW_SUNLIGHT,
                shadows_enabled: true,
                ..default()
            },
            Transform::default(),
            DespawnOnExit(GameState::Game),
        ))
        .id();

    // Spawn SkyCenter to control the sun's movement
    // This uses bevy_sun_move to handle realistic sun positioning
    commands.spawn((
        SkyCenter {
            // Latitude affects sun path across the sky
            // ~45° gives a nice temperate zone sun arc
            latitude_degrees: 45.0,
            // Earth-like axial tilt
            planet_tilt_degrees: 23.5,
            // Start at "morning" (around 0.25 is roughly 6am equivalent)
            year_fraction: 0.0,
            // Full day/night cycle duration
            cycle_duration_secs,
            // Reference to the sun entity
            sun: sun_entity,
            // Start time within the cycle (0.25 = ~morning)
            current_cycle_time: cycle_duration_secs * 0.25,
        },
        Transform::default(),
        Visibility::default(),
        DespawnOnExit(GameState::Game),
    ));
}

/// System to add Atmosphere to the game camera.
/// This runs after the camera is spawned to add atmospheric scattering effects.
pub fn setup_camera_atmosphere(
    mut commands: Commands,
    camera_query: Query<Entity, (With<Camera3d>, Without<Atmosphere>)>,
) {
    for camera_entity in camera_query.iter() {
        commands.entity(camera_entity).insert((
            // Use Earth-like atmosphere preset
            Atmosphere::EARTH,
            // Default atmosphere settings work well for most scenes
            AtmosphereSettings::default(),
        ));
    }
}

/// System to update ambient light based on sun position.
/// This creates a realistic day/night lighting transition by dimming ambient light
/// when the sun goes below the horizon.
pub fn update_ambient_light(
    sun_query: Query<&Transform, With<SunLight>>,
    mut ambient_light: ResMut<AmbientLight>,
) {
    let Ok(sun_transform) = sun_query.single() else {
        return;
    };

    // The sun's Y position in its transform indicates altitude
    // bevy_sun_move sets transform.translation to the sun direction vector
    // Y > 0 means sun is above horizon, Y < 0 means below
    let sun_altitude = sun_transform.translation.y;

    // Calculate ambient brightness based on sun altitude
    // Use a smooth transition around the horizon (sunrise/sunset)
    let transition_factor = if sun_altitude > 0.1 {
        // Full day - sun well above horizon
        1.0
    } else if sun_altitude < -0.1 {
        // Full night - sun well below horizon
        0.0
    } else {
        // Twilight transition - smooth interpolation around horizon
        // Map [-0.1, 0.1] to [0.0, 1.0]
        (sun_altitude + 0.1) / 0.2
    };

    // Interpolate between night and day brightness
    let brightness = AMBIENT_BRIGHTNESS_NIGHT
        + (AMBIENT_BRIGHTNESS_DAY - AMBIENT_BRIGHTNESS_NIGHT) * transition_factor;

    ambient_light.brightness = brightness;
}

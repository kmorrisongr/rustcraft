//! Celestial bodies and day/night cycle with custom sun movement.
//!
//! This module sets up:
//! - A DirectionalLight as the sun with a simple parametric orbit
//! - A DirectionalLight as the moon, moving opposite to the sun
//! - Bevy's built-in Atmosphere component on the camera for realistic sky rendering
//! - A configurable day/night cycle duration
//!
//! This approach gives us direct control over update frequency and timing,
//! which is important for performance when using atmosphere-based environment maps.

use crate::constants::DAY_DURATION_IN_TICKS;
use crate::world::time::ClientTime;
use crate::GameState;
use bevy::{
    light::light_consts::lux,
    pbr::{Atmosphere, AtmosphereSettings},
    prelude::*,
};
use bevy_light::AtmosphereEnvironmentMapLight;
use shared::TICKS_PER_SECOND;
use std::f32::consts::TAU;

/// Marker component for the sun entity
#[derive(Component)]
pub struct SunLight;

/// Marker component for the moon entity
#[derive(Component)]
pub struct MoonLight;

/// Day length in seconds, derived from tick-based duration
const DAY_LENGTH_SECS: f32 = DAY_DURATION_IN_TICKS as f32 / TICKS_PER_SECOND as f32;

/// System to set up the sun and moon for day/night cycles.
/// Spawns DirectionalLights for both celestial bodies.
pub fn setup_sun_and_sky(mut commands: Commands) {
    // Spawn the sun (DirectionalLight)
    // Using RAW_SUNLIGHT illuminance as recommended for use with Atmosphere
    commands.spawn((
        SunLight,
        DirectionalLight {
            illuminance: lux::RAW_SUNLIGHT,
            shadows_enabled: true,
            ..default()
        },
        Transform::default(),
        DespawnOnExit(GameState::Game),
    ));

    // Spawn the moon (DirectionalLight with lower illuminance)
    // Moon position is updated by update_celestial_bodies to be opposite the sun
    commands.spawn((
        MoonLight,
        DirectionalLight {
            illuminance: lux::FULL_DAYLIGHT * 0.1, // Moonlight is much dimmer
            shadows_enabled: true,
            ..default()
        },
        Transform::default(),
        DespawnOnExit(GameState::Game),
    ));
}

/// System to update sun and moon positions based on game time.
///
/// Uses a simple parametric model where:
/// - t=0.0 is midnight, t=0.25 is sunrise, t=0.5 is noon, t=0.75 is sunset
/// - The sun orbits in the XY plane (Y is up)
/// - The moon is always opposite the sun
///
/// This gives us direct control over update frequency, making it easy to
/// throttle updates or synchronize with environment map regeneration.
pub fn update_celestial_bodies(
    client_time: Res<ClientTime>,
    mut sun_query: Query<
        (&mut Transform, &mut DirectionalLight),
        (With<SunLight>, Without<MoonLight>),
    >,
    mut moon_query: Query<
        (&mut Transform, &mut DirectionalLight),
        (With<MoonLight>, Without<SunLight>),
    >,
) {
    let Ok((mut sun_transform, mut sun_light)) = sun_query.single_mut() else {
        return;
    };
    let Ok((mut moon_transform, mut moon_light)) = moon_query.single_mut() else {
        return;
    };

    // Convert game ticks to day phase (0.0 to 1.0)
    // Adding 0.25 offset so t=0 is midnight (sun below horizon)
    let elapsed_secs = client_time.0 as f32 / TICKS_PER_SECOND as f32;
    let t = (elapsed_secs / DAY_LENGTH_SECS) % 1.0;

    // Convert to angle (0 = midnight/below horizon, PI = noon/zenith)
    let angle = t * TAU;

    // Sun direction: orbits in the XZ-Y plane
    // At t=0 (angle=0), sun is at (1, 0, 0) - horizon east
    // At t=0.25 (angle=PI/2), sun is at (0, 1, 0) - zenith
    // At t=0.5 (angle=PI), sun is at (-1, 0, 0) - horizon west
    // At t=0.75 (angle=3PI/2), sun is at (0, -1, 0) - nadir (below ground)
    let sun_dir = Vec3::new(angle.cos(), angle.sin(), 0.0).normalize();

    // Update sun transform - point the light toward origin from the sun direction
    sun_transform.look_to(-sun_dir, Vec3::Z);

    // Adjust sun illuminance based on height above horizon
    // sun_dir.y > 0 means sun is above horizon
    let sun_height = sun_dir.y;
    if sun_height > 0.0 {
        // Daytime: scale illuminance by height for softer sunrise/sunset
        // Using a smoothstep-like curve for more natural lighting transitions
        let intensity = smooth_step(0.0, 0.3, sun_height);
        sun_light.illuminance = lux::RAW_SUNLIGHT * intensity;
    } else {
        // Nighttime: sun provides no direct light
        sun_light.illuminance = 0.0;
    }

    // Moon is opposite the sun
    let moon_dir = -sun_dir;
    moon_transform.look_to(-moon_dir, Vec3::Z);

    // Moon illuminance based on its height above horizon
    let moon_height = moon_dir.y;
    if moon_height > 0.0 {
        let intensity = smooth_step(0.0, 0.3, moon_height);
        moon_light.illuminance = lux::FULL_DAYLIGHT * 0.1 * intensity;
    } else {
        moon_light.illuminance = 0.0;
    }
}

/// Attempt to provide a smoother lighting transition
/// between night and day
fn smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// System to add Atmosphere to the game camera.
/// This runs after the camera is spawned to add atmospheric scattering effects.
pub fn setup_camera_atmosphere(
    mut commands: Commands,
    camera_query: Query<Entity, (With<Camera3d>, Without<Atmosphere>)>,
) {
    for camera_entity in camera_query.iter() {
        commands.entity(camera_entity).insert((
            // Earth-like atmosphere with higher ground albedo for better indirect lighting
            // Higher ground_albedo means more light bounces off terrain, providing fill light
            Atmosphere {
                // Increased from EARTH's 0.3 - makes daytime brighter and nights less pitch black
                ground_albedo: Vec3::splat(0.9),
                ..Atmosphere::EARTH
            },
            // Atmosphere settings tuned for the game's scale
            AtmosphereSettings {
                // Extended distance for aerial perspective on distant terrain
                aerial_view_lut_max_distance: 3.2e5,
                // Scale factor: 1 game unit = 1 meter (block-sized units)
                scene_units_to_m: 1.0,
                ..default()
            },
            AtmosphereEnvironmentMapLight { ..default() },
        ));
    }
}

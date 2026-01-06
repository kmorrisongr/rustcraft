//! Misty Lake water shader uniform data
//!
//! Separated into its own module to isolate the `#![allow(dead_code)]` directive,
//! which is required due to the ShaderType derive macro generating internal
//! `check` functions that trigger warnings.
//!
//! Original shader by Reinder Nijhoff: https://www.shadertoy.com/view/MsB3WR

#![allow(dead_code)]

use bevy::{prelude::*, render::render_resource::ShaderType};

/// Uniform data for water shader (matches WGSL MistyWaterUniforms struct)
#[derive(ShaderType, Debug, Clone)]
pub struct WaterUniforms {
    /// Current time for animation (updated each frame)
    pub time: f32,
    /// Wave scale - controls wave pattern size (default ~8.0)
    pub wave_scale: f32,
    /// Bump strength - controls normal perturbation intensity (default ~0.1)
    pub bump_strength: f32,
    /// Padding to maintain alignment
    pub _padding1: f32,
    /// Base color of the water (with alpha for transparency)
    pub water_color: Vec4,
    /// Deep water color (blended based on view angle)
    pub deep_color: Vec4,
    /// Sun direction (normalized, world space) - for reflections and specular
    pub sun_direction: Vec3,
    /// Fog density - distance fog factor for atmospheric effect
    pub fog_density: f32,
    /// Moon direction (normalized, world space) - for night reflections
    pub moon_direction: Vec3,
    /// Padding to align struct to 16 bytes (required by WGSL)
    pub _padding2: f32,
}

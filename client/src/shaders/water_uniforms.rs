//! Water shader uniform data
//!
//! Separated into its own module to isolate the `#![allow(dead_code)]` directive,
//! which is required due to the ShaderType derive macro generating internal
//! `check` functions that trigger warnings.

#![allow(dead_code)]

use bevy::{prelude::*, render::render_resource::ShaderType};

/// Uniform data for water shader (matches WGSL WaterUniforms struct)
#[derive(ShaderType, Debug, Clone)]
pub struct WaterUniforms {
    /// Current time for animation (updated each frame)
    pub time: f32,
    /// Wave amplitude - how high the waves rise
    pub wave_amplitude: f32,
    /// Wave frequency - how many waves per unit distance
    pub wave_frequency: f32,
    /// Wave speed - how fast the waves move
    pub wave_speed: f32,
    /// Base color of the water (with alpha for transparency)
    pub base_color: Vec4,
    /// Deep water color (blended based on depth)
    pub deep_color: Vec4,
}

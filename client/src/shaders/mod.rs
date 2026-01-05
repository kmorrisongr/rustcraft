//! Custom shader system for Rustcraft
//!
//! This module provides support for custom WGSL shaders, starting with
//! water rendering that includes animated standing waves.

pub mod water;
mod water_uniforms;

use bevy::prelude::*;

/// Plugin that sets up the custom shader system
pub struct ShadersPlugin;

impl Plugin for ShadersPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(water::WaterShaderPlugin);
    }
}

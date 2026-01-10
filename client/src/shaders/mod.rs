//! Custom shader system for Rustcraft
//!
//! This module provides custom shader integration for rendering effects.
//!
//! ## Water Shader
//! The water shader (`data/shaders/water.wgsl`) implements Gerstner wave
//! displacement for realistic water surface animation. It uses:
//! - Multiple wave layers with configurable parameters
//! - Fresnel-based reflectivity
//! - Depth-based color blending
//!
//! The shader is loaded automatically when `WaterMaterialResource` is initialized.

#![allow(dead_code)] // Shader loading functions will be used when custom material is enabled

use bevy::prelude::*;

/// Shader asset paths
pub mod paths {
    /// Path to the water Gerstner wave shader
    pub const WATER_SHADER: &str = "shaders/water.wgsl";
}

/// Preload shader assets to avoid runtime loading delays.
pub fn preload_shaders(asset_server: &AssetServer) -> Vec<Handle<Shader>> {
    vec![asset_server.load(paths::WATER_SHADER)]
}

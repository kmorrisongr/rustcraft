//! Gerstner wave calculations for water physics and rendering.
//!
//! This module provides CPU-side Gerstner wave calculations that exactly match
//! the GPU shader implementation (`data/shaders/water.wgsl`). This ensures
//! visual-physics consistency for water surfaces.
//!
//! ## Usage
//!
//! ```rust
//! use shared::world::gerstner::{GerstnerWaveParams, compute_wave_height};
//!
//! let params = GerstnerWaveParams::default();
//! let height = compute_wave_height(10.0, 5.0, 0.5, &params);
//! ```
//!
//! ## Synchronization
//!
//! Wave parameters defined in `DEFAULT_WAVE_LAYERS` are automatically passed
//! to the shader at runtime via the `WaterMaterialUniform`, ensuring perfect
//! synchronization between physics and rendering.

use bevy::math::{Vec2, Vec3};
use std::f32::consts::PI;

/// Gravity constant for wave physics (m/s²)
pub const GRAVITY: f32 = 9.8;

/// Amplitude falloff per wave layer (each layer has 70% of previous)
pub const AMPLITUDE_FALLOFF: f32 = 0.7;

/// Parameters for a single Gerstner wave layer.
#[derive(Debug, Clone, Copy)]
pub struct WaveLayer {
    /// Normalized wave direction (x, z)
    pub direction: Vec2,
    /// Steepness factor (Q) - controls how peaked the waves are (0.0-1.0)
    pub steepness: f32,
    /// Wavelength in world units
    pub wavelength: f32,
}

impl WaveLayer {
    pub const fn new(dir_x: f32, dir_z: f32, steepness: f32, wavelength: f32) -> Self {
        Self {
            direction: Vec2::new(dir_x, dir_z),
            steepness,
            wavelength,
        }
    }
}

/// Default wave layers used for water rendering and physics.
/// These values are automatically passed to the shader at runtime.
pub const DEFAULT_WAVE_LAYERS: [WaveLayer; 4] = [
    WaveLayer::new(1.0, 0.0, 0.5, 8.0),  // Primary wave - long, gentle
    WaveLayer::new(0.7, 0.7, 0.35, 4.0), // Secondary wave - medium
    WaveLayer::new(-0.3, 0.9, 0.25, 2.5), // Tertiary wave - shorter
    WaveLayer::new(0.9, -0.4, 0.15, 1.5), // Detail wave - small ripples
];

/// Global wave simulation parameters.
#[derive(Debug, Clone)]
pub struct GerstnerWaveParams {
    /// Wave layers to simulate
    pub layers: Vec<WaveLayer>,
    /// Base amplitude for the primary wave
    pub base_amplitude: f32,
    /// Wave animation speed multiplier
    pub speed: f32,
    /// Number of layers to use (1-4)
    pub num_layers: u32,
}

impl Default for GerstnerWaveParams {
    fn default() -> Self {
        Self {
            layers: DEFAULT_WAVE_LAYERS.to_vec(),
            base_amplitude: 0.08,
            speed: 1.0,
            num_layers: 3,
        }
    }
}

impl GerstnerWaveParams {
    /// Create params with custom amplitude
    pub fn with_amplitude(mut self, amplitude: f32) -> Self {
        self.base_amplitude = amplitude;
        self
    }

    /// Create params with custom number of layers
    pub fn with_layers(mut self, num_layers: u32) -> Self {
        self.num_layers = num_layers.min(4);
        self
    }
}

/// Calculate displacement from a single Gerstner wave layer.
///
/// # Arguments
/// * `pos` - World position (x, z)
/// * `layer` - Wave layer parameters
/// * `time` - Animation time
/// * `amplitude` - Wave amplitude for this layer
///
/// # Returns
/// 3D displacement vector (x, y, z)
pub fn gerstner_wave_displacement(pos: Vec2, layer: &WaveLayer, time: f32, amplitude: f32) -> Vec3 {
    let k = 2.0 * PI / layer.wavelength;
    let c = (GRAVITY / k).sqrt(); // Phase speed from dispersion relation
    let d = layer.direction.normalize();
    let f = k * (d.dot(pos) - c * time);
    let a = amplitude * layer.steepness / k;

    Vec3::new(d.x * a * f.cos(), amplitude * f.sin(), d.y * a * f.cos())
}

/// Calculate normal contribution from a single Gerstner wave layer.
///
/// # Arguments
/// * `pos` - World position (x, z)
/// * `layer` - Wave layer parameters
/// * `time` - Animation time
/// * `amplitude` - Wave amplitude for this layer
///
/// # Returns
/// Normal vector contribution (not normalized)
pub fn gerstner_wave_normal(pos: Vec2, layer: &WaveLayer, time: f32, amplitude: f32) -> Vec3 {
    let k = 2.0 * PI / layer.wavelength;
    let c = (GRAVITY / k).sqrt();
    let d = layer.direction.normalize();
    let f = k * (d.dot(pos) - c * time);
    let a = amplitude * layer.steepness;

    // Partial derivatives
    let dx = -d.x * a * f.cos();
    let dz = -d.y * a * f.cos();

    Vec3::new(dx, 1.0, dz)
}

/// Compute total wave displacement at a world position.
///
/// This function calculates the combined effect of all wave layers,
/// returning the 3D displacement from the base water surface.
///
/// # Arguments
/// * `world_x` - World X coordinate
/// * `world_z` - World Z coordinate
/// * `time` - Animation time (seconds)
/// * `params` - Wave simulation parameters
///
/// # Returns
/// 3D displacement vector (x_offset, y_offset, z_offset)
pub fn compute_wave_displacement(
    world_x: f32,
    world_z: f32,
    time: f32,
    params: &GerstnerWaveParams,
) -> Vec3 {
    let pos = Vec2::new(world_x, world_z);
    let adjusted_time = time * params.speed;
    let mut displacement = Vec3::ZERO;

    let num_layers = (params.num_layers as usize).min(params.layers.len());
    for (i, layer) in params.layers.iter().take(num_layers).enumerate() {
        let layer_amplitude = params.base_amplitude * AMPLITUDE_FALLOFF.powi(i as i32);
        displacement += gerstner_wave_displacement(pos, layer, adjusted_time, layer_amplitude);
    }

    displacement
}

/// Compute wave height at a world position (Y displacement only).
///
/// This is a convenience function when only vertical displacement is needed,
/// such as for simple collision checks.
///
/// # Arguments
/// * `world_x` - World X coordinate
/// * `world_z` - World Z coordinate
/// * `time` - Animation time (seconds)
/// * `params` - Wave simulation parameters
///
/// # Returns
/// Vertical displacement from base water level
pub fn compute_wave_height(
    world_x: f32,
    world_z: f32,
    time: f32,
    params: &GerstnerWaveParams,
) -> f32 {
    compute_wave_displacement(world_x, world_z, time, params).y
}

/// Compute wave normal at a world position.
///
/// # Arguments
/// * `world_x` - World X coordinate
/// * `world_z` - World Z coordinate
/// * `time` - Animation time (seconds)
/// * `params` - Wave simulation parameters
///
/// # Returns
/// Normalized surface normal vector
pub fn compute_wave_normal(
    world_x: f32,
    world_z: f32,
    time: f32,
    params: &GerstnerWaveParams,
) -> Vec3 {
    let pos = Vec2::new(world_x, world_z);
    let adjusted_time = time * params.speed;
    let mut accumulated_normal = Vec3::new(0.0, 1.0, 0.0);

    let num_layers = (params.num_layers as usize).min(params.layers.len());
    for (i, layer) in params.layers.iter().take(num_layers).enumerate() {
        let layer_amplitude = params.base_amplitude * AMPLITUDE_FALLOFF.powi(i as i32);
        accumulated_normal += gerstner_wave_normal(pos, layer, adjusted_time, layer_amplitude);
    }

    accumulated_normal.normalize()
}

/// Compute the displaced world position for a point on the water surface.
///
/// Given a base position on a flat water surface, this returns where that
/// point would actually be after wave displacement.
///
/// # Arguments
/// * `base_pos` - Base position on flat water surface
/// * `time` - Animation time (seconds)
/// * `params` - Wave simulation parameters
///
/// # Returns
/// Displaced world position
pub fn compute_displaced_position(base_pos: Vec3, time: f32, params: &GerstnerWaveParams) -> Vec3 {
    let displacement = compute_wave_displacement(base_pos.x, base_pos.z, time, params);
    base_pos + displacement
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wave_height_at_origin() {
        let params = GerstnerWaveParams::default();
        let height = compute_wave_height(0.0, 0.0, 0.0, &params);
        // At t=0, sin(0) = 0 for all waves, so height should be 0
        assert!((height).abs() < 0.001, "Height at t=0 should be ~0");
    }

    #[test]
    fn test_wave_height_varies_with_position() {
        let params = GerstnerWaveParams::default();
        let h1 = compute_wave_height(0.0, 0.0, 1.0, &params);
        let h2 = compute_wave_height(5.0, 5.0, 1.0, &params);
        assert!((h1 - h2).abs() > 0.001, "Height should vary with position");
    }

    #[test]
    fn test_wave_height_varies_with_time() {
        let params = GerstnerWaveParams::default();
        let h1 = compute_wave_height(0.0, 0.0, 0.0, &params);
        let h2 = compute_wave_height(0.0, 0.0, 1.0, &params);
        assert!((h1 - h2).abs() > 0.001, "Height should vary with time");
    }

    #[test]
    fn test_normal_is_normalized() {
        let params = GerstnerWaveParams::default();
        let normal = compute_wave_normal(5.0, 5.0, 1.0, &params);
        let length = normal.length();
        assert!((length - 1.0).abs() < 0.001, "Normal should be unit length");
    }

    #[test]
    fn test_amplitude_affects_height() {
        let params_small = GerstnerWaveParams::default().with_amplitude(0.01);
        let params_large = GerstnerWaveParams::default().with_amplitude(0.5);

        let h_small = compute_wave_height(5.0, 5.0, 1.0, &params_small).abs();
        let h_large = compute_wave_height(5.0, 5.0, 1.0, &params_large).abs();

        assert!(
            h_large > h_small,
            "Larger amplitude should produce larger waves"
        );
    }
}

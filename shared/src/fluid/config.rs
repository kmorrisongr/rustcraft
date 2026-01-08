//! Fluid simulation configuration.
//!
//! This module contains compile-time tunable parameters for the fluid simulation.

use bevy::prelude::*;

/// Compile-time fluid simulation configuration.
///
/// These constants control the behavior and performance of the fluid simulation.
/// Adjust these values at compile-time to tune the simulation for your needs.
pub mod constants {
    /// Number of fluid particles spawned per water block.
    ///  
    /// Higher values create more detailed fluid simulation but impact performance.
    /// Recommended range: 8-64 particles per block.
    pub const PARTICLES_PER_WATER_BLOCK: usize = 27; // 3x3x3 grid per block

    /// Radius of each fluid particle (in meters).
    ///
    /// Smaller particles allow for finer detail but require more particles.
    /// This should typically be set based on PARTICLES_PER_WATER_BLOCK.
    pub const PARTICLE_RADIUS: f32 = 0.15;

    /// Smoothing length factor for SPH (Smoothed Particle Hydrodynamics).
    ///
    /// This is multiplied by the particle radius to determine the kernel radius.
    /// Typical values: 2.0-3.0
    pub const SMOOTHING_FACTOR: f32 = 2.0;

    /// Fluid viscosity coefficient.
    ///
    /// Higher values make the fluid more viscous (honey-like).
    /// Lower values make it more fluid (water-like).
    /// Range: 0.0 (no viscosity) to 1.0 (very viscous)
    pub const VISCOSITY: f32 = 0.02;

    /// Fluid density at rest (kg/m³).
    ///
    /// Water is typically around 1000 kg/m³.
    pub const REST_DENSITY: f32 = 1000.0;

    /// Artificial pressure coefficient.
    ///
    /// Helps prevent particle clumping and maintains volume.
    /// Typical values: 0.0-0.1
    pub const ARTIFICIAL_PRESSURE: f32 = 0.01;

    /// Time step for fluid simulation (in seconds).
    ///
    /// Smaller time steps are more stable but require more computation.
    /// This should be a fraction of the game's fixed timestep.
    pub const FLUID_TIME_STEP: f32 = 1.0 / 120.0; // 120 Hz simulation

    /// Maximum number of fluid particles before warnings are logged.
    ///
    /// This helps prevent performance issues from spawning too many particles.
    pub const MAX_FLUID_PARTICLES_WARNING: usize = 50_000;

    /// Distance from chunk boundary where barriers are placed (in blocks).
    ///
    /// Barriers prevent fluid from spilling into ungenerated chunks.
    pub const CHUNK_BARRIER_MARGIN: f32 = 0.5;
}

/// Runtime fluid simulation configuration resource.
///
/// While most settings are compile-time constants, this resource allows
/// for runtime adjustments if needed (e.g., for debugging or difficulty settings).
#[derive(Resource, Clone, Debug, Reflect)]
#[reflect(Resource)]
pub struct FluidConfig {
    /// Whether fluid simulation is enabled.
    pub enabled: bool,

    /// Whether to render fluid particles (disable for performance testing).
    pub render_particles: bool,

    /// Whether to show debug visualization of chunk barriers.
    pub debug_barriers: bool,

    /// Particle render size multiplier.
    /// 
    /// Adjust this to make particles appear larger or smaller in rendering.
    pub particle_render_scale: f32,
}

impl Default for FluidConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            render_particles: true,
            debug_barriers: false,
            particle_render_scale: 1.0,
        }
    }
}

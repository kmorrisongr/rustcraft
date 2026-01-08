//! Fluid particle rendering system.
//!
//! This module handles rendering of fluid particles from the Salva3D simulation.
//! Water blocks serve as indicators for where to spawn fluid particles, and the
//! actual rendering is done via particle-based rendering.
//!
//! The old mesh-based water rendering has been replaced with a 3D fluid particle system.

use bevy::prelude::*;
use shared::fluid::{FluidConfig, FluidWorld};
use shared::fluid::config::constants::PARTICLE_RADIUS;

/// System to render fluid particles.
/// 
/// This system reads particle positions from the FluidWorld resource and
/// renders them as visual particles in the game world.
/// 
/// TODO: Implement efficient particle rendering (instancing, billboards, etc.)
pub fn render_fluid_particles(
    fluid_world: Res<FluidWorld>,
    fluid_config: Res<FluidConfig>,
    mut gizmos: Gizmos,
) {
    if !fluid_config.render_particles {
        return;
    }
    
    // For now, use debug visualization
    // TODO: Replace with proper particle rendering
    let positions = fluid_world.get_all_particle_positions();
    
    // Use configured max debug particles and render scale
    let max_particles = fluid_config.max_debug_particles;
    let render_radius = PARTICLE_RADIUS * fluid_config.particle_render_scale;
    
    for pos in positions.iter().take(max_particles) {
        gizmos.sphere(*pos, render_radius, Color::srgba(0.2, 0.4, 0.8, 0.6));
    }
}

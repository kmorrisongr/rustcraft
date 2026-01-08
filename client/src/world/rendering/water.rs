//! Fluid particle rendering system.
//!
//! This module handles rendering of fluid particles from the Salva3D simulation.
//! Water blocks serve as indicators for where to spawn fluid particles, and the
//! actual rendering is done via particle-based rendering.
//!
//! The old mesh-based water rendering has been replaced with a 3D fluid particle system.

use bevy::prelude::*;
use shared::fluid::FluidWorld;

/// System to render fluid particles.
/// 
/// This system reads particle positions from the FluidWorld resource and
/// renders them as visual particles in the game world.
/// 
/// TODO: Implement efficient particle rendering (instancing, billboards, etc.)
pub fn render_fluid_particles(
    fluid_world: Res<FluidWorld>,
    mut gizmos: Gizmos,
) {
    // For now, use debug visualization
    // TODO: Replace with proper particle rendering
    let positions = fluid_world.get_all_particle_positions();
    
    for pos in positions.iter().take(1000) {  // Limit for performance
        gizmos.sphere(*pos, 0.1, Color::srgba(0.2, 0.4, 0.8, 0.6));
    }
}

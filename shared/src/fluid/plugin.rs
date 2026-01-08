//! Bevy plugin for fluid simulation integration.

use bevy::prelude::*;

use super::{FluidConfig, FluidWorld};

/// Plugin that adds fluid simulation to the game.
///
/// This plugin:
/// - Initializes the Salva fluid world
/// - Manages fluid particle spawning based on water blocks
/// - Steps the fluid simulation each frame
/// - Integrates with Rapier physics for solid-fluid interactions
pub struct FluidPlugin;

impl Plugin for FluidPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FluidConfig>()
            .init_resource::<FluidWorld>()
            .register_type::<FluidConfig>()
            .add_systems(FixedUpdate, step_fluid_simulation);
    }
}

/// System that steps the fluid simulation forward.
///
/// This runs in FixedUpdate to ensure consistent physics timing.
fn step_fluid_simulation(
    mut fluid_world: ResMut<FluidWorld>,
    fluid_config: Res<FluidConfig>,
    time: Res<Time<Fixed>>,
) {
    if !fluid_config.enabled {
        return;
    }

    let dt = time.delta_secs();
    fluid_world.step(dt);
}

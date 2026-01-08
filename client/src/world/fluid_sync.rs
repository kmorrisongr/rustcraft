//! Fluid-chunk integration system.
//!
//! This module handles synchronization between chunk loading/unloading
//! and fluid particle spawning/despawning.

use crate::world::{ClientWorldMap, WorldRenderRequestUpdateEvent};
use bevy::log::debug;
use bevy::prelude::*;
use shared::fluid::FluidWorld;

/// System that spawns/despawns fluids when chunks load/unload.
///
/// This listens to WorldRenderRequestUpdateEvent and manages fluid particles
/// accordingly.
pub fn sync_fluids_with_chunks(
    mut fluid_world: ResMut<FluidWorld>,
    world_map: Res<ClientWorldMap>,
    mut chunk_events: EventReader<WorldRenderRequestUpdateEvent>,
) {
    for event in chunk_events.read() {
        let WorldRenderRequestUpdateEvent::ChunkToReload(chunk_pos) = event;

        // First, remove any existing fluids for this chunk
        fluid_world.remove_fluids_for_chunk(chunk_pos);

        // Then, spawn new fluids if the chunk exists and has water
        if let Some(chunk) = world_map.map.get(chunk_pos) {
            let particles_spawned = fluid_world.spawn_fluids_for_chunk(&chunk.map, chunk_pos);

            if particles_spawned > 0 {
                debug!(
                    "Spawned {} fluid particles for chunk {:?}",
                    particles_spawned, chunk_pos
                );
            }
        }
    }
}

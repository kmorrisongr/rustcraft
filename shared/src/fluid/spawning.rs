//! Fluid particle spawning based on water block positions.

use bevy::prelude::*;
use bevy_log::{debug, warn};
use nalgebra as na;
use salva3d::{
    object::{interaction_groups::InteractionGroups, Fluid, FluidHandle},
    solver::*,
};
use std::collections::HashMap;

use crate::world::BlockData;
use crate::world::BlockId;
use crate::CHUNK_SIZE;

use super::config::constants::*;

/// Component marking an entity as a fluid particle.
#[derive(Component, Clone, Copy, Debug)]
pub struct FluidParticle {
    /// Handle to the particle in the Salva fluid world.
    pub handle: FluidHandle,
}

/// Resource containing the Salva fluid world.
#[derive(Resource)]
pub struct FluidWorld {
    /// The Salva liquid world managing all fluid particles.
    pub liquid_world: salva3d::LiquidWorld,

    /// Mapping from chunk position to fluid handles spawned in that chunk.
    pub chunk_fluids: HashMap<IVec3, Vec<FluidHandle>>,
}

impl FluidWorld {
    /// Create a new fluid world with default configuration.
    pub fn new() -> Self {
        let particle_radius = PARTICLE_RADIUS;
        let smoothing_len = PARTICLE_RADIUS * SMOOTHING_FACTOR;

        // Create the liquid world with DFSPH (Divergence-Free SPH) solver
        // Using default kernel types (CubicSplineKernel for both density and gradient)
        let solver: DFSPHSolver = DFSPHSolver::new();
        let liquid_world = salva3d::LiquidWorld::new(solver, particle_radius, smoothing_len);

        Self {
            liquid_world,
            chunk_fluids: HashMap::new(),
        }
    }

    /// Spawn fluid particles for water blocks in a chunk.
    ///
    /// # Arguments
    /// * `chunk_blocks` - HashMap of blocks in the chunk
    /// * `chunk_pos` - Position of the chunk to spawn fluids in
    ///
    /// # Returns
    /// Number of particles spawned
    pub fn spawn_fluids_for_chunk(
        &mut self,
        chunk_blocks: &HashMap<IVec3, BlockData>,
        chunk_pos: &IVec3,
    ) -> usize {
        let mut particles_count = 0;
        let mut all_particles_positions = Vec::new();

        // Iterate through all blocks in the chunk
        for (local_pos, block) in chunk_blocks.iter() {
            if block.id != BlockId::Water {
                continue;
            }

            // Calculate global position
            let global_x = chunk_pos.x * CHUNK_SIZE + local_pos.x;
            let global_y = chunk_pos.y * CHUNK_SIZE + local_pos.y;
            let global_z = chunk_pos.z * CHUNK_SIZE + local_pos.z;

            // Spawn particles in a grid within the water block
            let particles_per_axis = (PARTICLES_PER_WATER_BLOCK as f32).cbrt().ceil() as i32;
            let spacing = 1.0 / (particles_per_axis as f32 + 1.0);

            for i in 1..=particles_per_axis {
                for j in 1..=particles_per_axis {
                    for k in 1..=particles_per_axis {
                        let px = global_x as f32 + (i as f32 * spacing);
                        let py = global_y as f32 + (j as f32 * spacing);
                        let pz = global_z as f32 + (k as f32 * spacing);

                        all_particles_positions.push(na::Point3::new(px, py, pz));
                        particles_count += 1;
                    }
                }
            }
        }

        // If we have particles to spawn, create a fluid object
        if !all_particles_positions.is_empty() {
            let fluid = Fluid::new(
                all_particles_positions,
                PARTICLE_RADIUS,
                REST_DENSITY,
                InteractionGroups::default(),
            );

            let fluid_handle = self.liquid_world.add_fluid(fluid);

            self.chunk_fluids
                .entry(*chunk_pos)
                .or_insert_with(Vec::new)
                .push(fluid_handle);

            if particles_count > MAX_FLUID_PARTICLES_WARNING {
                warn!(
                    "Spawned {} fluid particles in chunk {:?}, exceeding warning threshold of {}",
                    particles_count, chunk_pos, MAX_FLUID_PARTICLES_WARNING
                );
            } else {
                debug!(
                    "Spawned {} fluid particles in chunk {:?}",
                    particles_count, chunk_pos
                );
            }
        }

        particles_count
    }

    /// Remove all fluid particles associated with a chunk.
    ///
    /// # Arguments
    /// * `chunk_pos` - Position of the chunk to remove fluids from
    pub fn remove_fluids_for_chunk(&mut self, chunk_pos: &IVec3) {
        if let Some(fluid_handles) = self.chunk_fluids.remove(chunk_pos) {
            for handle in fluid_handles {
                self.liquid_world.remove_fluid(handle);
            }
            debug!("Removed fluids for chunk {:?}", chunk_pos);
        }
    }

    /// Step the fluid simulation forward by one time step.
    pub fn step(&mut self, dt: f32) {
        // Apply gravity
        let gravity = na::Vector3::new(0.0, -GRAVITY, 0.0);

        // No coupling manager yet (will add rapier integration later)
        // Using unit type () as a placeholder
        self.liquid_world.step_with_coupling(dt, &gravity, &mut ());
    }

    /// Get all particle positions for rendering.
    pub fn get_all_particle_positions(&self) -> Vec<Vec3> {
        let mut positions = Vec::new();

        for (_handle, fluid) in self.liquid_world.fluids().iter() {
            // positions is a field, not a method
            for particle_pos in &fluid.positions {
                positions.push(Vec3::new(particle_pos.x, particle_pos.y, particle_pos.z));
            }
        }

        positions
    }
}

impl Default for FluidWorld {
    fn default() -> Self {
        Self::new()
    }
}

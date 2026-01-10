//! Water simulation system for the server.
//!
//! This module handles water physics simulation including:
//! - Downward flow (gravity-driven)
//! - Triggered updates from terrain changes
//!
//! ## Design Principles
//! - Water simulation is server-authoritative
//! - Flow is event-driven, not continuous (for performance)
//! - Volume is conserved during transfers

use bevy::prelude::*;
use shared::world::{
    global_to_chunk_local, BlockData, BlockHitbox, BlockId, ServerWorldMap, WorldMap,
    MAX_WATER_VOLUME, MIN_WATER_VOLUME,
};
use std::collections::{HashMap, HashSet, VecDeque};

/// Event triggered when water needs to be re-evaluated at a position.
/// This can be caused by:
/// - Block removal exposing water from above
/// - Block placement displacing water
/// - Water flow completing
#[derive(Event, Debug, Clone)]
pub struct WaterUpdateEvent {
    /// Global position where water should be checked/updated
    pub position: IVec3,
}

/// Resource to track positions that need water simulation
#[derive(Resource, Default)]
pub struct WaterSimulationQueue {
    /// Positions queued for water simulation (global coordinates)
    pending: VecDeque<IVec3>,
    /// Set for O(1) duplicate checking
    pending_set: HashSet<IVec3>,
}

impl WaterSimulationQueue {
    /// Queue a position for water simulation
    pub fn queue(&mut self, pos: IVec3) {
        if self.pending_set.insert(pos) {
            self.pending.push_back(pos);
        }
    }

    /// Queue multiple positions (useful for batch operations)
    #[allow(dead_code)]
    pub fn queue_many(&mut self, positions: impl IntoIterator<Item = IVec3>) {
        for pos in positions {
            self.queue(pos);
        }
    }

    /// Get the next position to simulate, if any
    pub fn pop(&mut self) -> Option<IVec3> {
        if let Some(pos) = self.pending.pop_front() {
            self.pending_set.remove(&pos);
            Some(pos)
        } else {
            None
        }
    }

    /// Check if there are pending simulations
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Number of pending positions
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.pending.len()
    }
}

/// Resource to track chunks that need water surface detection update
#[derive(Resource, Default)]
pub struct WaterSurfaceUpdateQueue {
    /// Chunks that need surface detection refresh
    pub pending_chunks: HashSet<IVec3>,
}

impl WaterSurfaceUpdateQueue {
    /// Queue a chunk for surface detection update
    pub fn queue(&mut self, chunk_pos: IVec3) {
        self.pending_chunks.insert(chunk_pos);
    }

    /// Clear all pending chunks
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.pending_chunks.clear();
    }
}

/// Maximum number of water updates to process per tick
/// This prevents simulation from blocking the server
const MAX_UPDATES_PER_TICK: usize = 256;

/// Maximum number of chunks to update surfaces for per tick
const MAX_SURFACE_UPDATES_PER_TICK: usize = 8;

/// System to process blocks that were recently removed or placed.
/// This runs after player inputs and triggers water simulation for affected areas.
///
/// This is the main integration point between block changes (which happen through
/// the shared WorldMap trait) and the water simulation systems.
pub fn process_block_changes_for_water(
    mut world_map: ResMut<ServerWorldMap>,
    mut simulation_queue: ResMut<WaterSimulationQueue>,
    mut surface_queue: ResMut<WaterSurfaceUpdateQueue>,
    mut lateral_queue: ResMut<super::water_flow::LateralFlowQueue>,
) {
    // Process removed blocks
    let removed_blocks: Vec<IVec3> = world_map.chunks.recently_removed_blocks.drain(..).collect();

    for pos in removed_blocks {
        log::info!(
            "[WATER SIM] Processing removed block at {:?} for water flow",
            pos
        );
        super::terrain_mutation::handle_block_removal(
            &mut world_map,
            pos,
            &mut simulation_queue,
            &mut surface_queue,
            &mut lateral_queue,
        );
    }

    // Process placed blocks
    let placed_blocks: Vec<IVec3> = world_map.chunks.recently_placed_blocks.drain(..).collect();

    for pos in placed_blocks {
        log::info!(
            "[WATER SIM] Processing placed block at {:?} for water displacement",
            pos
        );
        let result = super::terrain_mutation::handle_block_placement(
            &mut world_map,
            pos,
            &mut simulation_queue,
            &mut surface_queue,
            &mut lateral_queue,
        );

        if result.displaced > 0.0 {
            log::info!(
                "[WATER SIM] Block placement at {:?}: displaced {:.3}, overflow {:.3}",
                pos,
                result.displaced,
                result.overflow
            );
        }
    }
}

/// System to handle water update events and queue them for simulation
pub fn handle_water_update_events(
    mut events: EventReader<WaterUpdateEvent>,
    mut queue: ResMut<WaterSimulationQueue>,
) {
    for event in events.read() {
        queue.queue(event.position);
        // Also queue the position below to check for downward flow
        queue.queue(event.position + IVec3::new(0, -1, 0));
    }
}

/// Main water simulation system - processes queued water updates
///
/// This system handles downward-only flow:
/// 1. Check if there's water at the position
/// 2. Check if the block below is air (or has room for more water)
/// 3. Transfer water volume downward
///
/// Integrates with sleep system to record vertical flow activity.
pub fn water_simulation_system(
    mut world_map: ResMut<ServerWorldMap>,
    mut queue: ResMut<WaterSimulationQueue>,
    mut surface_queue: ResMut<WaterSurfaceUpdateQueue>,
    mut sleep_manager: ResMut<super::water_sleep::WaterSleepManager>,
) {
    let mut updates_this_tick = 0;
    let mut chunks_modified: HashSet<IVec3> = HashSet::new();
    let mut chunk_flow_counts: HashMap<IVec3, usize> = HashMap::new();

    while updates_this_tick < MAX_UPDATES_PER_TICK {
        let Some(pos) = queue.pop() else {
            break;
        };

        // Check if the chunk containing this position is sleeping
        let (chunk_pos, _) = global_to_chunk_local(&pos);
        if !sleep_manager.should_simulate(&chunk_pos) {
            // Wake the chunk since we have pending work
            sleep_manager.wake_chunk(chunk_pos, "pending vertical flow", false);
        }

        if let Some(chunk_pos) = process_water_at_position(&mut world_map, pos, &mut queue) {
            chunks_modified.insert(chunk_pos);
            *chunk_flow_counts.entry(chunk_pos).or_insert(0) += 1;
        }

        updates_this_tick += 1;
    }

    // Record flow activity for sleep detection
    for (chunk_pos, flow_count) in chunk_flow_counts {
        // Vertical flow is significant activity
        sleep_manager.record_activity(chunk_pos, flow_count as f32 * 0.1, flow_count);
    }

    // Mark modified chunks for broadcast and surface update
    for chunk_pos in chunks_modified {
        if !world_map.chunks.chunks_to_update.contains(&chunk_pos) {
            world_map.chunks.chunks_to_update.push(chunk_pos);
        }
        // Queue for surface detection update
        surface_queue.queue(chunk_pos);
    }
}

/// System to update water surface detection for chunks that have been modified.
///
/// This runs after water simulation to detect which water cells are surface cells
/// (have air above them) and groups them into patches for efficient simulation.
///
/// Surface detection is important for:
/// 1. Efficient shallow-water wave simulation (only simulate surfaces)
/// 2. Mesh generation (only render surfaces)
/// 3. Lateral flow simulation (waves propagate on surfaces)
pub fn water_surface_detection_system(
    mut world_map: ResMut<ServerWorldMap>,
    mut surface_queue: ResMut<WaterSurfaceUpdateQueue>,
    mut lateral_flow_queue: ResMut<super::water_flow::LateralFlowQueue>,
) {
    if surface_queue.pending_chunks.is_empty() {
        return;
    }

    log::debug!(
        "[SURFACE DETECT] Starting with {} chunks queued",
        surface_queue.pending_chunks.len()
    );

    // Take up to MAX_SURFACE_UPDATES_PER_TICK chunks to process
    let chunks_to_process: Vec<IVec3> = surface_queue
        .pending_chunks
        .iter()
        .take(MAX_SURFACE_UPDATES_PER_TICK)
        .copied()
        .collect();

    for chunk_pos in &chunks_to_process {
        surface_queue.pending_chunks.remove(chunk_pos);
    }

    // Process each chunk
    for chunk_pos in chunks_to_process {
        let has_surfaces = update_chunk_surfaces(&mut world_map, chunk_pos);
        log::debug!(
            "[SURFACE DETECT] Chunk {:?}: has_surfaces={}",
            chunk_pos,
            has_surfaces
        );
        // Queue for lateral flow if surfaces were detected
        if has_surfaces {
            lateral_flow_queue.queue(chunk_pos);
        }
    }
}

/// Updates water surface detection for a single chunk.
/// Returns true if any surfaces were detected.
fn update_chunk_surfaces(world_map: &mut ServerWorldMap, chunk_pos: IVec3) -> bool {
    use shared::CHUNK_SIZE;

    // First, extract the water positions we need to check
    // and gather information about which positions above have solid blocks
    let water_positions: Vec<IVec3> = {
        let Some(chunk) = world_map.chunks.map.get(&chunk_pos) else {
            return false;
        };
        chunk.water.iter().map(|(pos, _)| *pos).collect()
    };

    if water_positions.is_empty() {
        // No water, clear surfaces
        if let Some(chunk) = world_map.chunks.map.get_mut(&chunk_pos) {
            chunk.water_surfaces.clear();
        }
        return false;
    }

    // Build a set of positions that have solid blocks above (for local lookups)
    // and check cross-chunk boundaries separately
    let mut solid_above_positions: HashSet<IVec3> = HashSet::new();

    for local_pos in &water_positions {
        let above_local = *local_pos + IVec3::new(0, 1, 0);

        // Check if above is within this chunk
        if above_local.y < CHUNK_SIZE {
            // Check within same chunk
            if let Some(chunk) = world_map.chunks.map.get(&chunk_pos) {
                if let Some(block) = chunk.map.get(&above_local) {
                    if block.id != BlockId::Water {
                        let is_solid = matches!(
                            block.id.get_hitbox(),
                            BlockHitbox::FullBlock | BlockHitbox::Aabb(_)
                        );
                        if is_solid {
                            solid_above_positions.insert(above_local);
                        }
                    }
                }
            }
        } else {
            // Need cross-chunk lookup
            let global_above = IVec3::new(
                chunk_pos.x * CHUNK_SIZE + above_local.x,
                chunk_pos.y * CHUNK_SIZE + above_local.y,
                chunk_pos.z * CHUNK_SIZE + above_local.z,
            );
            if let Some(block) = world_map.chunks.get_block_by_coordinates(&global_above) {
                if block.id != BlockId::Water {
                    let is_solid = matches!(
                        block.id.get_hitbox(),
                        BlockHitbox::FullBlock | BlockHitbox::Aabb(_)
                    );
                    if is_solid {
                        solid_above_positions.insert(above_local);
                    }
                }
            }
        }
    }

    // Now we can do the surface detection with owned data
    let is_solid_above = |above_pos: IVec3| -> bool { solid_above_positions.contains(&above_pos) };

    // Get the chunk and update its surfaces
    if let Some(chunk) = world_map.chunks.map.get_mut(&chunk_pos) {
        chunk.detect_water_surfaces(chunk_pos, is_solid_above);

        // Log surface detection results for debugging
        let stats = chunk.water_surfaces.stats();
        if stats.total_cells > 0 {
            log::debug!(
                "Chunk {:?}: detected {} surface cells in {} patches",
                chunk_pos,
                stats.total_cells,
                stats.patch_count
            );
            return true;
        }
    }
    false
}

/// Process water simulation at a single position
/// Returns the chunk position if water was modified
fn process_water_at_position(
    world_map: &mut ServerWorldMap,
    pos: IVec3,
    queue: &mut WaterSimulationQueue,
) -> Option<IVec3> {
    let (chunk_pos, local_pos) = global_to_chunk_local(&pos);

    // Get the chunk, if it exists
    let chunk = world_map.chunks.map.get(&chunk_pos)?;

    // Check if there's water at this position
    let water_volume = chunk.water.volume_at(&local_pos);
    if water_volume < MIN_WATER_VOLUME {
        return None;
    }

    // Check the block below
    let below_pos = pos + IVec3::new(0, -1, 0);
    let (below_chunk_pos, below_local_pos) = global_to_chunk_local(&below_pos);

    // Don't flow below y=0
    if below_pos.y < 0 {
        return None;
    }

    // Check if below position has a solid block
    let below_block = world_map.chunks.get_block_by_coordinates(&below_pos);
    let below_is_solid = below_block
        .map(|b| {
            // Water blocks are not solid for flow purposes
            if b.id == BlockId::Water {
                false
            } else {
                // Check if the block has a solid hitbox
                matches!(
                    b.id.get_hitbox(),
                    BlockHitbox::FullBlock | BlockHitbox::Aabb(_)
                )
            }
        })
        .unwrap_or(false);

    if below_is_solid {
        // Can't flow down through solid blocks
        return None;
    }

    // Get water volume in the cell below (0 if none)
    let below_water_volume = world_map
        .chunks
        .map
        .get(&below_chunk_pos)
        .map(|c| c.water.volume_at(&below_local_pos))
        .unwrap_or(0.0);

    // Calculate how much water can flow down
    let space_below = MAX_WATER_VOLUME - below_water_volume;
    if space_below < MIN_WATER_VOLUME {
        // No room below
        return None;
    }

    // Transfer water (all of it if possible, otherwise fill to capacity)
    let transfer_amount = water_volume.min(space_below);

    // Update source cell
    let chunk = world_map.chunks.map.get_mut(&chunk_pos)?;
    let new_source_volume = water_volume - transfer_amount;
    if new_source_volume < MIN_WATER_VOLUME {
        chunk.water.remove(&local_pos);
        // Also remove the Water block if water is gone
        if chunk.map.get(&local_pos).map(|b| b.id) == Some(BlockId::Water) {
            chunk.map.remove(&local_pos);
        }
    } else {
        chunk.water.set(local_pos, new_source_volume);
    }

    // Update destination cell
    let below_chunk = world_map.chunks.map.get_mut(&below_chunk_pos)?;
    let new_below_volume = below_water_volume + transfer_amount;
    below_chunk.water.set(below_local_pos, new_below_volume);

    // Add Water block if it doesn't exist
    if below_chunk.map.get(&below_local_pos).is_none() {
        below_chunk.map.insert(
            below_local_pos,
            BlockData::new(BlockId::Water, shared::world::BlockDirection::Front),
        );
    }

    // Queue the position below for continued flow
    queue.queue(below_pos);

    // If source still has water, queue it for potential continued flow
    if new_source_volume >= MIN_WATER_VOLUME {
        queue.queue(pos);
    }

    // Mark both chunks as modified
    if !world_map.chunks.chunks_to_update.contains(&chunk_pos) {
        world_map.chunks.chunks_to_update.push(chunk_pos);
    }
    if below_chunk_pos != chunk_pos && !world_map.chunks.chunks_to_update.contains(&below_chunk_pos)
    {
        world_map.chunks.chunks_to_update.push(below_chunk_pos);
    }

    Some(chunk_pos)
}

/// System to trigger water updates when blocks are removed
/// This should be called after block removal to check if water above should flow down
pub fn trigger_water_flow_on_block_removal(pos: IVec3, queue: &mut WaterSimulationQueue) {
    // Queue the position above for water flow check
    queue.queue(pos + IVec3::new(0, 1, 0));
    // Also queue the position itself (water might flow into it from sides later)
    queue.queue(pos);
}

/// System to trigger water displacement when blocks are placed
/// This should be called after block placement to displace any water
pub fn trigger_water_displacement_on_block_placement(
    world_map: &mut ServerWorldMap,
    pos: IVec3,
    queue: &mut WaterSimulationQueue,
) {
    let (chunk_pos, local_pos) = global_to_chunk_local(&pos);

    // Check if there was water at this position
    if let Some(chunk) = world_map.chunks.map.get_mut(&chunk_pos) {
        if let Some(water_cell) = chunk.water.remove(&local_pos) {
            let volume = water_cell.volume();

            // Try to push water upward
            let above_pos = pos + IVec3::new(0, 1, 0);
            let (above_chunk_pos, above_local_pos) = global_to_chunk_local(&above_pos);

            // Check if above is air
            let above_is_air = world_map
                .chunks
                .get_block_by_coordinates(&above_pos)
                .is_none();

            if above_is_air {
                if let Some(above_chunk) = world_map.chunks.map.get_mut(&above_chunk_pos) {
                    let existing_above = above_chunk.water.volume_at(&above_local_pos);
                    let new_above_volume = (existing_above + volume).min(MAX_WATER_VOLUME);
                    above_chunk.water.set(above_local_pos, new_above_volume);

                    // Add Water block
                    if above_chunk.map.get(&above_local_pos).is_none() {
                        above_chunk.map.insert(
                            above_local_pos,
                            BlockData::new(BlockId::Water, shared::world::BlockDirection::Front),
                        );
                    }

                    // Queue for further flow
                    queue.queue(above_pos);

                    if !world_map.chunks.chunks_to_update.contains(&above_chunk_pos) {
                        world_map.chunks.chunks_to_update.push(above_chunk_pos);
                    }
                }
            }
            // Note: If can't push up, water is lost (simplified physics)
            // Future: could try lateral displacement
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_water_simulation_queue() {
        let mut queue = WaterSimulationQueue::default();

        // Queue some positions
        queue.queue(IVec3::new(0, 0, 0));
        queue.queue(IVec3::new(1, 0, 0));
        queue.queue(IVec3::new(0, 0, 0)); // Duplicate - should not be added

        assert_eq!(queue.len(), 2);

        // Pop them
        assert_eq!(queue.pop(), Some(IVec3::new(0, 0, 0)));
        assert_eq!(queue.pop(), Some(IVec3::new(1, 0, 0)));
        assert_eq!(queue.pop(), None);
        assert!(queue.is_empty());
    }
}

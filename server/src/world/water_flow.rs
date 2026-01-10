//! Lateral water flow simulation using shallow-water equations.
//!
//! This module implements horizontal water spreading based on surface height
//! differences within surface patches. It uses a simplified shallow-water model
//! that is stable and efficient for voxel-based water simulation.
//!
//! ## Design Principles
//! - Flow is driven by surface height differences (pressure gradient)
//! - Simulation operates per-patch (2D, not full 3D)
//! - Volume is conserved during transfers
//! - Uses semi-implicit integration for stability
//!
//! ## Algorithm
//! For each surface cell, we compute outflow to each of the 4 cardinal neighbors
//! based on the height difference. Flow rate is proportional to height difference
//! and limited by available volume and timestep constraints.

use bevy::prelude::*;
use shared::world::{
    water_utils::{is_valid_local_pos, LATERAL_NEIGHBORS},
    BlockData, BlockHitbox, BlockId, ServerWorldMap, FULL_WATER_HEIGHT, MAX_WATER_VOLUME,
    MIN_WATER_VOLUME,
};
use shared::CHUNK_SIZE;
use std::collections::{HashMap, HashSet};

/// Flow rate coefficient - controls how fast water spreads laterally.
/// Higher values = faster spreading, but may cause instability.
/// This is effectively the "hydraulic conductivity" of our simplified model.
pub const FLOW_RATE: f32 = 0.25;

/// Minimum height difference to trigger flow (prevents oscillation).
/// Water won't flow if the height difference is below this threshold.
pub const MIN_HEIGHT_DIFF: f32 = 0.001;

/// Maximum flow per cell per tick (as fraction of cell volume).
/// Limits how much water can leave a cell in one simulation step.
pub const MAX_FLOW_PER_TICK: f32 = 0.25;

/// Damping factor to reduce oscillations (0 = no damping, 1 = full damping).
/// Applied to flow calculations to prevent water from sloshing indefinitely.
pub const FLOW_DAMPING: f32 = 0.1;

/// Maximum number of lateral flow updates per tick.
const MAX_LATERAL_UPDATES_PER_TICK: usize = 512;

/// Resource to track chunks that need lateral flow simulation.
#[derive(Resource, Default)]
pub struct LateralFlowQueue {
    /// Chunks queued for lateral flow simulation
    pub pending_chunks: HashSet<IVec3>,
}

impl LateralFlowQueue {
    /// Queue a chunk for lateral flow simulation.
    pub fn queue(&mut self, chunk_pos: IVec3) {
        self.pending_chunks.insert(chunk_pos);
    }

    /// Remove a chunk from the queue.
    pub fn remove(&mut self, chunk_pos: &IVec3) {
        self.pending_chunks.remove(chunk_pos);
    }
}

/// Temporary storage for flow calculations within a chunk.
/// We compute all flows first, then apply them to avoid order-dependent results.
#[derive(Default)]
struct FlowAccumulator {
    /// Net volume change for each cell position (local coordinates)
    /// Positive = gaining water, negative = losing water
    delta: HashMap<IVec3, f32>,
}

impl FlowAccumulator {
    fn new() -> Self {
        Self {
            delta: HashMap::new(),
        }
    }

    /// Record flow from source to destination.
    fn record_flow(&mut self, from: IVec3, to: IVec3, amount: f32) {
        if amount > MIN_WATER_VOLUME {
            *self.delta.entry(from).or_insert(0.0) -= amount;
            *self.delta.entry(to).or_insert(0.0) += amount;
        }
    }

    /// Check if any flow occurred.
    fn has_changes(&self) -> bool {
        self.delta.values().any(|&v| v.abs() > MIN_WATER_VOLUME)
    }
}

/// Lateral flow simulation system.
///
/// This system processes surface patches and simulates horizontal water flow
/// based on height differences. It runs after surface detection and before
/// the next frame's vertical flow.
///
/// Supports cross-chunk flow via the WaterBoundaryCache.
/// Integrates with water sleep system to skip sleeping chunks and record activity.
pub fn lateral_flow_system(
    mut world_map: ResMut<ServerWorldMap>,
    mut flow_queue: ResMut<LateralFlowQueue>,
    boundary_cache: Res<super::water_boundary::WaterBoundaryCache>,
    mut vertical_queue: ResMut<super::water_simulation::WaterSimulationQueue>,
    mut sleep_manager: ResMut<super::water_sleep::WaterSleepManager>,
) {
    if flow_queue.pending_chunks.is_empty() {
        return;
    }

    let queue_size = flow_queue.pending_chunks.len();
    log::debug!(
        "[LATERAL FLOW] Starting lateral_flow_system with {} chunks queued",
        queue_size
    );

    // Take chunks to process this tick
    let chunks_to_process: Vec<IVec3> = flow_queue.pending_chunks.iter().copied().collect();

    let mut total_updates = 0;
    let mut chunks_modified: HashSet<IVec3> = HashSet::new();
    let mut cross_chunk_flows: Vec<super::water_boundary::CrossChunkFlow> = Vec::new();
    let mut skipped_sleeping = 0;

    for chunk_pos in chunks_to_process {
        if total_updates >= MAX_LATERAL_UPDATES_PER_TICK {
            log::debug!(
                "[LATERAL FLOW] Hit max updates limit ({})",
                MAX_LATERAL_UPDATES_PER_TICK
            );
            break;
        }

        flow_queue.remove(&chunk_pos);

        // Check if this chunk is sleeping (skip simulation if so)
        if !sleep_manager.should_simulate(&chunk_pos) {
            skipped_sleeping += 1;
            continue;
        }

        log::debug!("[LATERAL FLOW] Processing chunk {:?}", chunk_pos);

        if let Some((updates, neighbor_flows, volume_delta)) = process_chunk_lateral_flow(
            &mut world_map,
            chunk_pos,
            &boundary_cache,
            &mut vertical_queue,
        ) {
            log::debug!(
                "[LATERAL FLOW] Chunk {:?}: {} cell updates, {} cross-chunk flows",
                chunk_pos,
                updates,
                neighbor_flows.len()
            );

            // Record activity for sleep detection
            sleep_manager.record_activity(chunk_pos, volume_delta, updates);

            total_updates += updates;
            if updates > 0 {
                chunks_modified.insert(chunk_pos);
                // Re-queue for continued simulation if flow occurred
                flow_queue.queue(chunk_pos);
            }
            // Collect cross-chunk flows for batch processing
            cross_chunk_flows.extend(neighbor_flows);
        }
    }

    // Apply cross-chunk flows
    let neighbor_chunks =
        apply_cross_chunk_flows(&mut world_map, cross_chunk_flows, &mut vertical_queue);
    for neighbor_chunk in &neighbor_chunks {
        chunks_modified.insert(*neighbor_chunk);
        // Queue neighbor for continued simulation
        flow_queue.queue(*neighbor_chunk);
        // Wake neighboring chunks that received cross-chunk flow
        sleep_manager.wake_chunk(*neighbor_chunk, "cross-chunk flow", false);
    }

    if skipped_sleeping > 0 {
        log::debug!(
            "[LATERAL FLOW] Skipped {} sleeping chunks",
            skipped_sleeping
        );
    }

    log::debug!(
        "[LATERAL FLOW] Finished: {} total updates, {} chunks modified",
        total_updates,
        chunks_modified.len()
    );

    // Mark modified chunks for broadcast
    for chunk_pos in chunks_modified {
        if !world_map.chunks.chunks_to_update.contains(&chunk_pos) {
            world_map.chunks.chunks_to_update.push(chunk_pos);
        }
    }
}

/// Process lateral flow for a single chunk.
/// Returns the number of cells that were updated, any cross-chunk flows,
/// and the total volume delta (for sleep detection), or None if chunk doesn't exist.
fn process_chunk_lateral_flow(
    world_map: &mut ServerWorldMap,
    chunk_pos: IVec3,
    boundary_cache: &super::water_boundary::WaterBoundaryCache,
    vertical_queue: &mut super::water_simulation::WaterSimulationQueue,
) -> Option<(usize, Vec<super::water_boundary::CrossChunkFlow>, f32)> {
    // First pass: gather surface cells and their current heights
    let (surface_cells, water_volumes): (Vec<IVec3>, HashMap<IVec3, f32>) = {
        let chunk = world_map.chunks.map.get(&chunk_pos)?;

        let surface_count = chunk.water_surfaces.cell_count();
        let water_count = chunk.water.len();

        log::debug!(
            "[LATERAL FLOW] Chunk {:?}: {} surface cells, {} total water cells",
            chunk_pos,
            surface_count,
            water_count
        );

        if surface_count == 0 {
            return Some((0, Vec::new(), 0.0));
        }

        let cells: Vec<IVec3> = chunk.water_surfaces.cell_positions().copied().collect();

        let volumes: HashMap<IVec3, f32> = cells
            .iter()
            .map(|pos| (*pos, chunk.water.volume_at(pos)))
            .collect();

        (cells, volumes)
    };

    if surface_cells.is_empty() {
        return Some((0, Vec::new(), 0.0));
    }

    log::debug!(
        "[LATERAL FLOW] Processing {} surface cells in chunk {:?}",
        surface_cells.len(),
        chunk_pos
    );

    // Build set of surface positions for quick neighbor lookup
    // Note: Currently unused but available for surface-only flow rules (see TODO below)
    let _surface_set: HashSet<IVec3> = surface_cells.iter().copied().collect();

    // Compute flows for all cells
    let mut accumulator = FlowAccumulator::new();
    let mut cross_chunk_flows: Vec<super::water_boundary::CrossChunkFlow> = Vec::new();
    let mut flow_attempts = 0;
    let mut flow_blocked = 0;
    let mut flow_height_rejected = 0;
    let mut flow_recorded = 0;

    for &pos in &surface_cells {
        let volume = water_volumes.get(&pos).copied().unwrap_or(0.0);
        if volume < MIN_WATER_VOLUME {
            continue;
        }

        // Calculate surface height at this cell (global Y + local height)
        let surface_height = pos.y as f32 + volume * FULL_WATER_HEIGHT;

        // Check 4 cardinal neighbors
        for offset in LATERAL_NEIGHBORS {
            let neighbor_pos = pos + offset;
            flow_attempts += 1;

            // Check if neighbor is within chunk bounds
            if !is_valid_local_pos(&neighbor_pos) {
                // Cross-chunk flow: calculate using boundary cache
                let flows = super::water_boundary::calculate_cross_chunk_flows(
                    chunk_pos,
                    pos,
                    volume,
                    surface_height,
                    boundary_cache,
                    world_map,
                );
                cross_chunk_flows.extend(flows);
                continue;
            }

            // Get neighbor info
            let neighbor_volume = water_volumes.get(&neighbor_pos).copied().unwrap_or(0.0);
            // TODO: surface_set.contains(&neighbor_pos) could enable surface-only flow rules

            // Check if neighbor position is blocked by solid block
            let neighbor_blocked = {
                let chunk = world_map.chunks.map.get(&chunk_pos)?;
                if let Some(block) = chunk.map.get(&neighbor_pos) {
                    block.id != BlockId::Water
                        && matches!(
                            block.id.get_hitbox(),
                            BlockHitbox::FullBlock | BlockHitbox::Aabb(_)
                        )
                } else {
                    false
                }
            };

            if neighbor_blocked {
                flow_blocked += 1;
                continue;
            }

            // Calculate neighbor surface height
            // If neighbor has water, use its surface height
            // If neighbor is empty but at same or lower Y, water can spread there
            let neighbor_surface_height = if neighbor_volume > MIN_WATER_VOLUME {
                neighbor_pos.y as f32 + neighbor_volume * FULL_WATER_HEIGHT
            } else if neighbor_pos.y <= pos.y {
                // Empty neighbor at same or lower level - water can spread here
                // Use the floor of the neighbor cell as its "height"
                neighbor_pos.y as f32
            } else {
                // Empty neighbor above us - can't flow upward
                flow_height_rejected += 1;
                continue;
            };

            // Calculate height difference
            let height_diff = surface_height - neighbor_surface_height;

            // Only flow downhill (or to same level if we have more volume)
            if height_diff < MIN_HEIGHT_DIFF {
                flow_height_rejected += 1;
                continue;
            }

            // Calculate flow amount based on height difference
            // Using a simple linear model: flow ∝ height_diff
            let mut flow_amount = height_diff * FLOW_RATE;

            // Apply damping to reduce oscillation
            flow_amount *= 1.0 - FLOW_DAMPING;

            // Limit flow to available volume
            flow_amount = flow_amount.min(volume * MAX_FLOW_PER_TICK);

            // Limit flow to space available in neighbor
            let neighbor_space = MAX_WATER_VOLUME - neighbor_volume;
            flow_amount = flow_amount.min(neighbor_space);

            // Minimum threshold check
            if flow_amount < MIN_WATER_VOLUME {
                flow_height_rejected += 1;
                continue;
            }

            log::trace!(
                "[LATERAL FLOW] Flow: {:?} -> {:?}, height_diff={:.3}, amount={:.3}",
                pos,
                neighbor_pos,
                height_diff,
                flow_amount
            );

            flow_recorded += 1;
            accumulator.record_flow(pos, neighbor_pos, flow_amount);
        }
    }

    log::debug!(
        "[LATERAL FLOW] Chunk {:?}: {} attempts, {} blocked, {} height-rejected, {} flows recorded",
        chunk_pos,
        flow_attempts,
        flow_blocked,
        flow_height_rejected,
        flow_recorded
    );

    // Apply accumulated flows
    if !accumulator.has_changes() && cross_chunk_flows.is_empty() {
        return Some((0, Vec::new(), 0.0));
    }

    // Even if no intra-chunk changes, we may have cross-chunk flows
    if !accumulator.has_changes() {
        // Cross-chunk flows still count as activity
        let cross_chunk_delta: f32 = cross_chunk_flows.iter().map(|f| f.flow_amount).sum();
        return Some((0, cross_chunk_flows, cross_chunk_delta));
    }

    // Calculate total volume delta for sleep detection BEFORE consuming accumulator
    let total_volume_delta: f32 = accumulator.delta.values().map(|d| d.abs()).sum();
    let cross_chunk_delta: f32 = cross_chunk_flows.iter().map(|f| f.flow_amount).sum();

    let chunk = world_map.chunks.map.get_mut(&chunk_pos)?;
    let mut cells_updated = 0;

    for (pos, delta) in accumulator.delta {
        let current_volume = chunk.water.volume_at(&pos);
        let new_volume = (current_volume + delta).clamp(0.0, MAX_WATER_VOLUME);

        if (new_volume - current_volume).abs() > MIN_WATER_VOLUME {
            // If water is flowing OUT of this cell, queue the cell above
            // for vertical flow check (so water above can fall down)
            if delta < -MIN_WATER_VOLUME {
                let global_pos = chunk_pos * CHUNK_SIZE as i32 + pos;
                let above_pos = global_pos + IVec3::new(0, 1, 0);
                vertical_queue.queue(above_pos);
                log::trace!(
                    "[LATERAL FLOW] Water left {:?}, queuing cell above {:?} for vertical flow",
                    global_pos,
                    above_pos
                );
            }

            if new_volume < MIN_WATER_VOLUME {
                chunk.water.remove(&pos);
                // Also remove Water block if present
                if chunk.map.get(&pos).map(|b| b.id) == Some(BlockId::Water) {
                    chunk.map.remove(&pos);
                }
            } else {
                chunk.water.set(pos, new_volume);
                // Add Water block if not present
                if chunk.map.get(&pos).is_none() {
                    chunk.map.insert(
                        pos,
                        BlockData::new(BlockId::Water, shared::world::BlockDirection::Front),
                    );
                }
            }
            cells_updated += 1;
        }
    }

    Some((
        cells_updated,
        cross_chunk_flows,
        total_volume_delta + cross_chunk_delta,
    ))
}

/// Applies cross-chunk water flows collected during lateral flow processing.
/// Returns the set of neighbor chunks that were modified.
///
/// This function:
/// 1. Deducts water from the source cell
/// 2. Adds water to the destination cell
/// 3. Queues cells above source for vertical flow re-evaluation
fn apply_cross_chunk_flows(
    world_map: &mut ServerWorldMap,
    flows: Vec<super::water_boundary::CrossChunkFlow>,
    vertical_queue: &mut super::water_simulation::WaterSimulationQueue,
) -> HashSet<IVec3> {
    let mut modified_chunks = HashSet::new();

    for flow in flows {
        // First, deduct water from source chunk
        if let Some(source_chunk) = world_map.chunks.map.get_mut(&flow.source_chunk) {
            let source_volume = source_chunk.water.volume_at(&flow.source_local_pos);
            let new_source_volume = (source_volume - flow.flow_amount).max(0.0);

            if new_source_volume < MIN_WATER_VOLUME {
                source_chunk.water.remove(&flow.source_local_pos);
                if source_chunk.map.get(&flow.source_local_pos).map(|b| b.id)
                    == Some(BlockId::Water)
                {
                    source_chunk.map.remove(&flow.source_local_pos);
                }
            } else {
                source_chunk
                    .water
                    .set(flow.source_local_pos, new_source_volume);
            }

            modified_chunks.insert(flow.source_chunk);

            // Queue the cell above source for vertical flow check
            let global_source_pos = flow.source_chunk * CHUNK_SIZE as i32 + flow.source_local_pos;
            let above_pos = global_source_pos + IVec3::new(0, 1, 0);
            vertical_queue.queue(above_pos);
            log::trace!(
                "[CROSS-CHUNK FLOW] Water left {:?}, queuing cell above {:?} for vertical flow",
                global_source_pos,
                above_pos
            );
        }

        // Then, add water to destination chunk
        let Some(neighbor_chunk) = world_map.chunks.map.get_mut(&flow.neighbor_chunk) else {
            continue;
        };

        // Get current water at destination
        let current_volume = neighbor_chunk.water.volume_at(&flow.neighbor_local_pos);
        let new_volume = (current_volume + flow.flow_amount).min(MAX_WATER_VOLUME);

        if (new_volume - current_volume).abs() > MIN_WATER_VOLUME {
            neighbor_chunk
                .water
                .set(flow.neighbor_local_pos, new_volume);

            // Add Water block if not present
            if neighbor_chunk.map.get(&flow.neighbor_local_pos).is_none() {
                neighbor_chunk.map.insert(
                    flow.neighbor_local_pos,
                    BlockData::new(BlockId::Water, shared::world::BlockDirection::Front),
                );
            }

            modified_chunks.insert(flow.neighbor_chunk);
        }
    }

    modified_chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_accumulator() {
        let mut acc = FlowAccumulator::new();
        let pos_a = IVec3::new(5, 10, 5);
        let pos_b = IVec3::new(6, 10, 5);

        acc.record_flow(pos_a, pos_b, 0.1);

        assert!(acc.has_changes());
        assert!(acc.delta.get(&pos_a).unwrap() < &0.0);
        assert!(acc.delta.get(&pos_b).unwrap() > &0.0);
    }

    #[test]
    fn test_flow_accumulator_bidirectional() {
        let mut acc = FlowAccumulator::new();
        let pos_a = IVec3::new(5, 10, 5);
        let pos_b = IVec3::new(6, 10, 5);

        // Flow both ways should partially cancel
        acc.record_flow(pos_a, pos_b, 0.2);
        acc.record_flow(pos_b, pos_a, 0.1);

        // Net: A loses 0.1, B gains 0.1
        assert!((acc.delta.get(&pos_a).unwrap() + 0.1).abs() < 0.001);
        assert!((acc.delta.get(&pos_b).unwrap() - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_lateral_neighbors_constant() {
        // Verify the shared constant has 4 horizontal directions
        assert_eq!(LATERAL_NEIGHBORS.len(), 4);
        for offset in LATERAL_NEIGHBORS {
            assert_eq!(
                offset.y, 0,
                "LATERAL_NEIGHBORS should not have vertical components"
            );
        }
    }
}

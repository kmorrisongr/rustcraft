//! Terrain mutation handling for water physics.
//!
//! This module implements water behavior when terrain changes:
//! - Block removal: Water flows down from above, or spreads laterally into vacated space
//! - Block placement: Water is displaced to neighbors (up, then lateral)
//!
//! ## Design Principles
//! - Volume conservation: Water is never created or destroyed, only moved
//! - Local updates: Only affected positions and their neighbors are queued
//! - Incremental: Changes trigger cascading updates through the simulation queue
//!
//! ## Block Removal Algorithm
//! 1. Check if there's water directly above → queue for downward flow
//! 2. Check lateral neighbors for water → queue them for potential inflow
//! 3. Mark affected chunks for surface re-detection
//!
//! ## Block Placement Algorithm
//! 1. Check if there's water at the target position
//! 2. Try to push water upward first (if air above)
//! 3. If upward fails, distribute water to lateral neighbors
//! 4. If no neighbors can accept, water overflows upward anyway (pressure)
//! 5. Update surface patches

use bevy::math::IVec3;
use shared::world::{
    global_to_chunk_local,
    water_utils::{ALL_NEIGHBORS, LATERAL_NEIGHBORS},
    BlockData, BlockHitbox, BlockId, ServerWorldMap, WorldMap, MAX_WATER_VOLUME, MIN_WATER_VOLUME,
};

/// Maximum vertical distance to search when forcing water overflow upward.
/// Limits computational cost when water is trapped in deep columns.
const MAX_UPWARD_FLOW_SEARCH: i32 = 10;

use super::water_flow::LateralFlowQueue;
use super::water_simulation::{WaterSimulationQueue, WaterSurfaceUpdateQueue};

/// Result of a water displacement operation
#[derive(Debug, Default)]
pub struct DisplacementResult {
    /// Volume that was successfully displaced
    pub displaced: f32,
    /// Volume that couldn't be placed anywhere (lost to overflow)
    pub overflow: f32,
    /// Chunks that were modified
    pub modified_chunks: Vec<IVec3>,
}

/// Handles water flow when a block is removed from the world.
///
/// When a solid block is removed:
/// 1. Water above it should flow down (gravity)
/// 2. Water from lateral neighbors should flow into the new space (pressure equalization)
/// 3. Water surfaces need to be recalculated
///
/// This function performs immediate lateral inflow and queues further simulation.
pub fn handle_block_removal(
    world_map: &mut ServerWorldMap,
    removed_pos: IVec3,
    simulation_queue: &mut WaterSimulationQueue,
    surface_queue: &mut WaterSurfaceUpdateQueue,
    lateral_queue: &mut LateralFlowQueue,
) {
    let (chunk_pos, _local_pos) = global_to_chunk_local(&removed_pos);

    log::info!(
        "[TERRAIN MUT] Block removed at {:?}, chunk {:?}",
        removed_pos,
        chunk_pos
    );

    // Check lateral neighbors for water
    let mut lateral_water_count = 0;
    for offset in LATERAL_NEIGHBORS {
        let neighbor_pos = removed_pos + offset;
        if has_water_at(world_map, &neighbor_pos) {
            lateral_water_count += 1;
            let vol = get_water_volume_at(world_map, &neighbor_pos);
            log::info!(
                "[TERRAIN MUT] Lateral neighbor {:?} has water, volume={:.3}",
                neighbor_pos,
                vol
            );
        }
    }
    log::info!(
        "[TERRAIN MUT] Found {} lateral neighbors with water",
        lateral_water_count
    );

    // 1. Check for water above - it should flow down (handled by simulation queue)
    let above_pos = removed_pos + IVec3::new(0, 1, 0);
    if has_water_at(world_map, &above_pos) {
        simulation_queue.queue(above_pos);
        log::info!(
            "[TERRAIN MUT] Block removed at {:?}: queuing water above at {:?} for downward flow",
            removed_pos,
            above_pos
        );
    }

    // 2. Perform IMMEDIATE lateral inflow from neighbors
    // This is crucial - water needs to flow into the freed space right away,
    // not wait for surface-based wave propagation
    let inflow_result = perform_immediate_lateral_inflow(world_map, removed_pos);

    if inflow_result.volume_received > MIN_WATER_VOLUME {
        log::debug!(
            "Block removed at {:?}: immediate lateral inflow of {:.3} from {} neighbors",
            removed_pos,
            inflow_result.volume_received,
            inflow_result.contributing_neighbor_count
        );

        // Queue the position and neighbors for continued simulation
        simulation_queue.queue(removed_pos);
        for neighbor_pos in &inflow_result.donor_positions {
            simulation_queue.queue(*neighbor_pos);
        }

        // Mark modified chunks for surface update and lateral flow
        for chunk in &inflow_result.chunks_requiring_update {
            surface_queue.queue(*chunk);
            lateral_queue.queue(*chunk);
        }
    }

    // 3. Even if no immediate inflow, queue the position for potential future flow
    simulation_queue.queue(removed_pos);

    // 4. Mark chunk and neighbors for surface update
    surface_queue.queue(chunk_pos);
    for offset in ALL_NEIGHBORS {
        let neighbor_chunk = chunk_pos + offset;
        if world_map.chunks.map.contains_key(&neighbor_chunk) {
            surface_queue.queue(neighbor_chunk);
        }
    }

    // 5. Queue chunks with water neighbors for lateral flow continuation
    for offset in LATERAL_NEIGHBORS {
        let neighbor_pos = removed_pos + offset;
        if has_water_at(world_map, &neighbor_pos) {
            let (neighbor_chunk, _) = global_to_chunk_local(&neighbor_pos);
            lateral_queue.queue(neighbor_chunk);
        }
    }

    log::debug!(
        "Block removal at {:?}: processing complete, inflow={:.3}",
        removed_pos,
        inflow_result.volume_received
    );
}

/// Result of immediate lateral inflow calculation when a block is removed.
/// Captures the water equalization that occurs when space opens up.
#[derive(Debug, Default)]
struct LateralInflowResult {
    /// Total water volume (0.0 to MAX_WATER_VOLUME) that flowed into the freed space
    volume_received: f32,
    /// Count of neighboring cells that contributed water to equalization
    contributing_neighbor_count: u32,
    /// Global positions of cells that donated water
    donor_positions: Vec<IVec3>,
    /// Chunk coordinates that were modified and need mesh updates
    chunks_requiring_update: Vec<IVec3>,
}

/// Performs immediate lateral water inflow when a block is removed.
///
/// This is different from the surface-based lateral flow system:
/// - Surface flow: gradual wave propagation between surface cells
/// - This: immediate pressure equalization when space opens up
///
/// Algorithm:
/// 1. Find all lateral neighbors with water
/// 2. Calculate how much each neighbor can contribute (based on height difference)
/// 3. Transfer water immediately to equalize levels
fn perform_immediate_lateral_inflow(
    world_map: &mut ServerWorldMap,
    freed_pos: IVec3,
) -> LateralInflowResult {
    let mut result = LateralInflowResult::default();

    log::info!(
        "[TERRAIN MUT] perform_immediate_lateral_inflow at {:?}",
        freed_pos
    );

    // Gather water info from lateral neighbors
    let mut neighbor_water: Vec<(IVec3, f32)> = Vec::new();

    for offset in LATERAL_NEIGHBORS {
        let neighbor_pos = freed_pos + offset;

        // Skip if neighbor is blocked by solid (non-water) block
        if let Some(block) = world_map.chunks.get_block_by_coordinates(&neighbor_pos) {
            if block.id != BlockId::Water {
                let is_solid = matches!(
                    block.id.get_hitbox(),
                    BlockHitbox::FullBlock | BlockHitbox::Aabb(_)
                );
                if is_solid {
                    log::info!(
                        "[TERRAIN MUT] Neighbor {:?} blocked by {:?}",
                        neighbor_pos,
                        block.id
                    );
                    continue;
                }
            }
        }

        let volume = get_water_volume_at(world_map, &neighbor_pos);
        log::info!(
            "[TERRAIN MUT] Neighbor {:?} water volume: {:.3}",
            neighbor_pos,
            volume
        );
        if volume > MIN_WATER_VOLUME {
            neighbor_water.push((neighbor_pos, volume));
            result.contributing_neighbor_count += 1;
        }
    }

    if neighbor_water.is_empty() {
        log::info!("[TERRAIN MUT] No neighbors with water, returning early");
        return result;
    }

    // Calculate total available water and target equalization level
    let total_water: f32 = neighbor_water.iter().map(|(_, v)| *v).sum();
    let cells_in_equalization_group = neighbor_water.len() as f32 + 1.0; // neighbors + freed position
    let target_level = (total_water / cells_in_equalization_group).min(MAX_WATER_VOLUME);

    log::info!(
        "[TERRAIN MUT] Lateral inflow at {:?}: {} neighbors with total {:.3} water, target level {:.3}",
        freed_pos,
        neighbor_water.len(),
        total_water,
        target_level
    );

    // Calculate how much each neighbor contributes
    // Neighbors with more water than target give to the freed space
    let mut total_contribution = 0.0_f32;
    let mut contributions: Vec<(IVec3, f32)> = Vec::new();

    for (neighbor_pos, volume) in &neighbor_water {
        if *volume > target_level {
            let contribution = *volume - target_level;
            contributions.push((*neighbor_pos, contribution));
            total_contribution += contribution;
        }
    }

    if total_contribution < MIN_WATER_VOLUME {
        return result;
    }

    // The freed position can accept up to MAX_WATER_VOLUME
    let actual_inflow = total_contribution.min(MAX_WATER_VOLUME);
    // Scale contributions proportionally if total exceeds capacity
    let contribution_scale_factor = if total_contribution > actual_inflow {
        actual_inflow / total_contribution
    } else {
        1.0
    };

    // Apply the water transfers
    let (freed_chunk_pos, freed_local_pos) = global_to_chunk_local(&freed_pos);

    // Add water to freed position
    if actual_inflow > MIN_WATER_VOLUME {
        if let Some(chunk) = world_map.chunks.map.get_mut(&freed_chunk_pos) {
            chunk.water.set(freed_local_pos, actual_inflow);

            // Add Water block
            chunk.map.insert(
                freed_local_pos,
                BlockData::new(BlockId::Water, shared::world::BlockDirection::Front),
            );

            if !result.chunks_requiring_update.contains(&freed_chunk_pos) {
                result.chunks_requiring_update.push(freed_chunk_pos);
            }

            // Mark chunk for update
            if !world_map.chunks.chunks_to_update.contains(&freed_chunk_pos) {
                world_map.chunks.chunks_to_update.push(freed_chunk_pos);
            }
        }

        result.volume_received = actual_inflow;
    }

    // Remove water from contributing neighbors
    for (neighbor_pos, contribution) in contributions {
        let scaled_contribution = contribution * contribution_scale_factor;
        if scaled_contribution < MIN_WATER_VOLUME {
            continue;
        }

        let (neighbor_chunk_pos, neighbor_local_pos) = global_to_chunk_local(&neighbor_pos);

        if let Some(chunk) = world_map.chunks.map.get_mut(&neighbor_chunk_pos) {
            let current_volume = chunk.water.volume_at(&neighbor_local_pos);
            let new_volume = current_volume - scaled_contribution;

            if new_volume < MIN_WATER_VOLUME {
                chunk.water.remove(&neighbor_local_pos);
                // Remove Water block if water is gone
                if chunk.map.get(&neighbor_local_pos).map(|b| b.id) == Some(BlockId::Water) {
                    chunk.map.remove(&neighbor_local_pos);
                }
            } else {
                chunk.water.set(neighbor_local_pos, new_volume);
            }

            result.donor_positions.push(neighbor_pos);

            if !result.chunks_requiring_update.contains(&neighbor_chunk_pos) {
                result.chunks_requiring_update.push(neighbor_chunk_pos);
            }

            // Mark chunk for update
            if !world_map
                .chunks
                .chunks_to_update
                .contains(&neighbor_chunk_pos)
            {
                world_map.chunks.chunks_to_update.push(neighbor_chunk_pos);
            }
        }
    }

    result
}

/// Handles water displacement when a block is placed in the world.
///
/// When a block is placed where water exists:
/// 1. The water at that position must be moved elsewhere
/// 2. First, try to push water upward (if space above)
/// 3. If upward fails, distribute to lateral neighbors
/// 4. Remaining water overflows upward under pressure
///
/// Returns information about the displacement operation.
pub fn handle_block_placement(
    world_map: &mut ServerWorldMap,
    placed_pos: IVec3,
    simulation_queue: &mut WaterSimulationQueue,
    surface_queue: &mut WaterSurfaceUpdateQueue,
    lateral_queue: &mut LateralFlowQueue,
) -> DisplacementResult {
    let mut result = DisplacementResult::default();

    let (chunk_pos, local_pos) = global_to_chunk_local(&placed_pos);

    // Check if there's water at the placement position
    let water_volume = {
        let Some(chunk) = world_map.chunks.map.get(&chunk_pos) else {
            return result;
        };
        chunk.water.volume_at(&local_pos)
    };

    if water_volume < MIN_WATER_VOLUME {
        // No water to displace, but still update surfaces
        surface_queue.queue(chunk_pos);
        return result;
    }

    log::debug!(
        "Block placement at {:?}: displacing {:.3} water volume",
        placed_pos,
        water_volume
    );

    // Remove water from placement position
    if let Some(chunk) = world_map.chunks.map.get_mut(&chunk_pos) {
        chunk.water.remove(&local_pos);
        result.modified_chunks.push(chunk_pos);
    }

    let mut remaining_volume = water_volume;

    // Strategy 1: Try to push water upward first (most natural behavior)
    let above_pos = placed_pos + IVec3::new(0, 1, 0);
    let pushed_up = try_add_water_at(world_map, above_pos, remaining_volume, &mut result);
    remaining_volume -= pushed_up;
    if pushed_up > MIN_WATER_VOLUME {
        simulation_queue.queue(above_pos);
        log::debug!(
            "Displaced {:.3} water volume upward to {:?}",
            pushed_up,
            above_pos
        );
    }

    // Strategy 2: Distribute remaining water to lateral neighbors
    if remaining_volume > MIN_WATER_VOLUME {
        let distributed = distribute_water_laterally(
            world_map,
            placed_pos,
            remaining_volume,
            simulation_queue,
            &mut result,
        );
        remaining_volume -= distributed;
    }

    // Strategy 3: Force overflow upward (water finds a way under pressure)
    if remaining_volume > MIN_WATER_VOLUME {
        let forced = force_water_upward(world_map, placed_pos, remaining_volume, &mut result);
        remaining_volume -= forced;
        if forced > MIN_WATER_VOLUME {
            // Queue the column above for continued flow
            let mut check_pos = above_pos;
            while has_water_at(world_map, &check_pos) {
                simulation_queue.queue(check_pos);
                check_pos.y += 1;
                if check_pos.y > placed_pos.y + MAX_UPWARD_FLOW_SEARCH {
                    break;
                }
            }
        }
    }

    // Any remaining volume is lost (very rare edge case)
    if remaining_volume > MIN_WATER_VOLUME {
        result.overflow = remaining_volume;
        log::warn!(
            "Block placement at {:?}: {:.3} water volume lost to overflow",
            placed_pos,
            remaining_volume
        );
    }

    result.displaced = water_volume - remaining_volume;

    // Queue affected chunks for surface update and lateral flow
    for &modified_chunk in &result.modified_chunks {
        surface_queue.queue(modified_chunk);
        lateral_queue.queue(modified_chunk);
    }

    // Also update neighboring chunks
    for offset in ALL_NEIGHBORS {
        let neighbor_chunk = chunk_pos + offset;
        if world_map.chunks.map.contains_key(&neighbor_chunk) {
            surface_queue.queue(neighbor_chunk);
        }
    }

    log::debug!(
        "Block placement at {:?}: displaced {:.3}, overflow {:.3}",
        placed_pos,
        result.displaced,
        result.overflow
    );

    result
}

/// Checks if there's water at the given global position.
fn has_water_at(world_map: &ServerWorldMap, pos: &IVec3) -> bool {
    let (chunk_pos, local_pos) = global_to_chunk_local(pos);
    world_map
        .chunks
        .map
        .get(&chunk_pos)
        .map(|c| c.water.has_water(&local_pos))
        .unwrap_or(false)
}

/// Gets the water volume at a position (0.0 if none or chunk doesn't exist).
fn get_water_volume_at(world_map: &ServerWorldMap, pos: &IVec3) -> f32 {
    let (chunk_pos, local_pos) = global_to_chunk_local(pos);
    world_map
        .chunks
        .map
        .get(&chunk_pos)
        .map(|c| c.water.volume_at(&local_pos))
        .unwrap_or(0.0)
}

/// Checks if a position can accept water (is air or partial water, not solid).
fn can_accept_water(world_map: &ServerWorldMap, pos: &IVec3) -> bool {
    let block = world_map.chunks.get_block_by_coordinates(pos);

    match block {
        None => true, // Air can accept water
        Some(b) if b.id == BlockId::Water => {
            // Existing water can accept more if not full
            get_water_volume_at(world_map, pos) < MAX_WATER_VOLUME - MIN_WATER_VOLUME
        }
        Some(b) => {
            // Check if block is non-solid (can coexist with water)
            !matches!(
                b.id.get_hitbox(),
                BlockHitbox::FullBlock | BlockHitbox::Aabb(_)
            )
        }
    }
}

/// Tries to add water at a position, respecting capacity limits.
/// Returns the amount of water actually added.
fn try_add_water_at(
    world_map: &mut ServerWorldMap,
    pos: IVec3,
    volume: f32,
    result: &mut DisplacementResult,
) -> f32 {
    if !can_accept_water(world_map, &pos) {
        return 0.0;
    }

    let (chunk_pos, local_pos) = global_to_chunk_local(&pos);

    let Some(chunk) = world_map.chunks.map.get_mut(&chunk_pos) else {
        return 0.0;
    };

    let existing_volume = chunk.water.volume_at(&local_pos);
    let space_available = MAX_WATER_VOLUME - existing_volume;
    let to_add = volume.min(space_available);

    if to_add > MIN_WATER_VOLUME {
        let new_volume = existing_volume + to_add;
        chunk.water.set(local_pos, new_volume);

        // Ensure there's a Water block
        if chunk.map.get(&local_pos).map(|b| b.id) != Some(BlockId::Water) {
            chunk.map.insert(
                local_pos,
                BlockData::new(BlockId::Water, shared::world::BlockDirection::Front),
            );
        }

        if !result.modified_chunks.contains(&chunk_pos) {
            result.modified_chunks.push(chunk_pos);
        }

        // Mark chunk for update
        if !world_map.chunks.chunks_to_update.contains(&chunk_pos) {
            world_map.chunks.chunks_to_update.push(chunk_pos);
        }
    }

    to_add
}

/// Distributes water to lateral neighbors.
/// Returns the total volume distributed.
fn distribute_water_laterally(
    world_map: &mut ServerWorldMap,
    source_pos: IVec3,
    volume: f32,
    simulation_queue: &mut WaterSimulationQueue,
    result: &mut DisplacementResult,
) -> f32 {
    // Find neighbors that can accept water
    let mut accepting_neighbors: Vec<(IVec3, f32)> = Vec::new();

    for offset in LATERAL_NEIGHBORS {
        let neighbor_pos = source_pos + offset;
        if can_accept_water(world_map, &neighbor_pos) {
            let existing = get_water_volume_at(world_map, &neighbor_pos);
            let space = MAX_WATER_VOLUME - existing;
            if space > MIN_WATER_VOLUME {
                accepting_neighbors.push((neighbor_pos, space));
            }
        }
    }

    if accepting_neighbors.is_empty() {
        return 0.0;
    }

    // Distribute water evenly among neighbors
    let total_space: f32 = accepting_neighbors.iter().map(|(_, s)| s).sum();
    let mut distributed = 0.0;
    let mut remaining = volume;

    for (neighbor_pos, space) in accepting_neighbors {
        // Distribute proportionally to available space
        let share = (space / total_space) * volume;
        let to_add = share.min(space).min(remaining);

        let added = try_add_water_at(world_map, neighbor_pos, to_add, result);
        distributed += added;
        remaining -= added;

        if added > MIN_WATER_VOLUME {
            simulation_queue.queue(neighbor_pos);
            log::debug!(
                "Distributed {:.3} water volume laterally to {:?}",
                added,
                neighbor_pos
            );
        }

        if remaining < MIN_WATER_VOLUME {
            break;
        }
    }

    distributed
}

/// Forces water upward under pressure, stacking if necessary.
/// Returns the total volume placed.
fn force_water_upward(
    world_map: &mut ServerWorldMap,
    base_pos: IVec3,
    volume: f32,
    result: &mut DisplacementResult,
) -> f32 {
    let mut remaining = volume;
    let mut current_y = base_pos.y + 1;
    let max_y = base_pos.y + 16; // Reasonable limit

    while remaining > MIN_WATER_VOLUME && current_y <= max_y {
        let check_pos = IVec3::new(base_pos.x, current_y, base_pos.z);

        if can_accept_water(world_map, &check_pos) {
            let added = try_add_water_at(world_map, check_pos, remaining, result);
            remaining -= added;

            if added > MIN_WATER_VOLUME {
                log::debug!("Forced {:.3} water volume upward to {:?}", added, check_pos);
            }
        } else {
            // Hit a solid block, try the next position up
            // (water might find a way through cracks)
        }

        current_y += 1;
    }

    volume - remaining
}

/// Checks if water at a position should be updated after terrain changes nearby.
/// This is used for cascading updates when multiple blocks change.
pub fn should_update_water_at(world_map: &ServerWorldMap, pos: &IVec3) -> bool {
    if !has_water_at(world_map, pos) {
        return false;
    }

    // Check if there's air below (unstable - should flow down)
    let below_pos = *pos + IVec3::new(0, -1, 0);
    if below_pos.y >= 0 {
        let below_block = world_map.chunks.get_block_by_coordinates(&below_pos);
        if below_block.is_none() || below_block.map(|b| b.id) == Some(BlockId::Water) {
            let below_volume = get_water_volume_at(world_map, &below_pos);
            if below_volume < MAX_WATER_VOLUME - MIN_WATER_VOLUME {
                return true; // Can flow down
            }
        }
    }

    // Check if any lateral neighbor has significantly lower water level
    let current_volume = get_water_volume_at(world_map, pos);
    for offset in LATERAL_NEIGHBORS {
        let neighbor_pos = *pos + offset;
        if can_accept_water(world_map, &neighbor_pos) {
            let neighbor_volume = get_water_volume_at(world_map, &neighbor_pos);
            if current_volume - neighbor_volume > 0.1 {
                return true; // Significant height difference
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full tests would require setting up a ServerWorldMap
    // These are placeholder tests for the helper functions

    #[test]
    fn test_lateral_neighbors() {
        assert_eq!(LATERAL_NEIGHBORS.len(), 4);
        // Verify no vertical component
        for offset in LATERAL_NEIGHBORS {
            assert_eq!(offset.y, 0);
        }
    }

    #[test]
    fn test_all_neighbors() {
        assert_eq!(ALL_NEIGHBORS.len(), 6);
        // Verify we have both vertical directions
        let has_up = ALL_NEIGHBORS.iter().any(|o| o.y == 1);
        let has_down = ALL_NEIGHBORS.iter().any(|o| o.y == -1);
        assert!(has_up);
        assert!(has_down);
    }
}

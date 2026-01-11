//! Water body sleeping system for performance optimization.
//!
//! This module implements sleeping behavior for water bodies to reduce
//! simulation cost when water reaches a stable state.
//!
//! ## Design Principles
//! - Water bodies that are stable (no significant flow) can "sleep"
//! - Sleeping water skips simulation, saving CPU cycles
//! - Water wakes up when terrain changes or external forces occur
//! - Sleep detection is based on accumulated stability over time
//!
//! ## Sleep Criteria
//! A water region is considered stable when:
//! 1. No significant flow has occurred for multiple ticks
//! 2. Volume changes are below a threshold
//! 3. No recent terrain modifications nearby
//!
//! ## Wake Triggers
//! - Block placement/removal near water
//! - Cross-chunk water flow into the chunk
//! - Manual wake signal (for debugging)

use bevy::prelude::*;
use shared::world::ServerWorldMap;
use std::collections::{HashMap, HashSet};

// ============================================================================
// Configuration Constants
// ============================================================================

/// Minimum number of stable ticks before water can sleep.
/// Higher values = more conservative sleeping (less likely to miss flow).
pub const MIN_STABLE_TICKS_TO_SLEEP: u32 = 20;

/// Maximum volume change (sum of absolute deltas) per tick to be considered stable.
/// Below this threshold, the chunk is considered "calm".
pub const STABILITY_VOLUME_THRESHOLD: f32 = 0.01;

/// Maximum number of flow events per tick to be considered stable.
/// Even small flows count against stability if they're numerous.
pub const STABILITY_FLOW_COUNT_THRESHOLD: usize = 2;

/// Number of ticks a chunk remains awake after being woken.
/// This prevents rapid sleep/wake oscillation.
pub const MIN_AWAKE_TICKS_AFTER_WAKE: u32 = 10;

/// Distance (in chunks) within which terrain changes wake sleeping water.
/// A value of 1 means only adjacent chunks are affected.
pub const WAKE_RADIUS_CHUNKS: i32 = 1;

// ============================================================================
// Sleep State Tracking
// ============================================================================

/// Represents the sleep state of water in a chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WaterSleepState {
    /// Water is being actively simulated.
    #[default]
    Awake,
    /// Water is stable and not being simulated.
    Asleep,
    /// Water was recently woken and cannot sleep yet.
    Waking,
}

/// Detailed state tracking for water simulation in a chunk.
#[derive(Debug, Clone, Default)]
pub struct ChunkWaterSleepState {
    /// Current sleep state.
    pub state: WaterSleepState,
    /// Number of consecutive stable ticks (resets on flow).
    pub stable_ticks: u32,
    /// Number of ticks since last wake (used for wake cooldown).
    pub ticks_since_wake: u32,
    /// Total volume change in the last tick (for stability detection).
    pub last_tick_volume_delta: f32,
    /// Number of flow events in the last tick.
    pub last_tick_flow_count: usize,
    /// Generation counter when this state was last updated.
    pub generation: u64,
}

impl ChunkWaterSleepState {
    /// Creates a new awake sleep state.
    pub fn new() -> Self {
        Self {
            state: WaterSleepState::Awake,
            stable_ticks: 0,
            ticks_since_wake: 0,
            last_tick_volume_delta: 0.0,
            last_tick_flow_count: 0,
            generation: 0,
        }
    }

    /// Returns true if this chunk's water should be simulated.
    #[inline]
    pub fn should_simulate(&self) -> bool {
        self.state != WaterSleepState::Asleep
    }

    /// Returns true if this chunk's water is sleeping.
    #[inline]
    pub fn is_asleep(&self) -> bool {
        self.state == WaterSleepState::Asleep
    }

    /// Records activity for this tick (called during simulation).
    pub fn record_activity(&mut self, volume_delta: f32, flow_count: usize) {
        self.last_tick_volume_delta = volume_delta.abs();
        self.last_tick_flow_count = flow_count;
        self.generation += 1;
    }

    /// Updates sleep state based on recent activity.
    /// Call this once per tick after simulation.
    pub fn update(&mut self) {
        match self.state {
            WaterSleepState::Awake => {
                // Check if chunk is stable this tick
                let is_stable = self.last_tick_volume_delta < STABILITY_VOLUME_THRESHOLD
                    && self.last_tick_flow_count <= STABILITY_FLOW_COUNT_THRESHOLD;

                if is_stable {
                    self.stable_ticks += 1;
                    if self.stable_ticks >= MIN_STABLE_TICKS_TO_SLEEP {
                        self.state = WaterSleepState::Asleep;
                        log::debug!("Water sleeping after {} stable ticks", self.stable_ticks);
                    }
                } else {
                    // Reset stability counter on activity
                    self.stable_ticks = 0;
                }
            }
            WaterSleepState::Waking => {
                self.ticks_since_wake += 1;
                if self.ticks_since_wake >= MIN_AWAKE_TICKS_AFTER_WAKE {
                    self.state = WaterSleepState::Awake;
                    self.ticks_since_wake = 0;
                }
            }
            WaterSleepState::Asleep => {
                // Sleeping chunks don't update unless woken
            }
        }

        // Reset per-tick counters for next tick
        self.last_tick_volume_delta = 0.0;
        self.last_tick_flow_count = 0;
    }

    /// Wakes the chunk from sleep.
    pub fn wake(&mut self, chunk_pos: IVec3, reason: &str) {
        if self.state == WaterSleepState::Asleep {
            log::debug!("Water waking at chunk {:?}: {}", chunk_pos, reason);
            self.state = WaterSleepState::Waking;
            self.ticks_since_wake = 0;
            self.stable_ticks = 0;
        } else if self.state == WaterSleepState::Awake {
            // Reset stability counter when forced awake
            self.stable_ticks = 0;
        }
    }

    /// Forces the chunk to stay awake (resets stability counter).
    pub fn keep_awake(&mut self) {
        self.stable_ticks = 0;
        if self.state == WaterSleepState::Asleep {
            self.state = WaterSleepState::Waking;
            self.ticks_since_wake = 0;
        }
    }
}

// ============================================================================
// Global Sleep State Resource
// ============================================================================

/// Resource tracking sleep state for all chunks with water.
#[derive(Resource, Default)]
pub struct WaterSleepManager {
    /// Sleep state per chunk.
    chunk_states: HashMap<IVec3, ChunkWaterSleepState>,
    /// Chunks that were woken this tick (for logging/debugging).
    pub recently_woken: HashSet<IVec3>,
    /// Chunks that went to sleep this tick.
    pub recently_slept: HashSet<IVec3>,
    /// Global statistics.
    pub stats: WaterSleepStats,
}

/// Statistics about the water sleep system.
#[derive(Debug, Clone, Copy, Default)]
pub struct WaterSleepStats {
    /// Number of chunks currently sleeping.
    pub sleeping_chunks: usize,
    /// Number of chunks being simulated.
    pub awake_chunks: usize,
    /// Number of chunks in wake cooldown.
    pub waking_chunks: usize,
    /// Total simulation ticks saved (estimated).
    pub ticks_saved: u64,
}

impl WaterSleepManager {
    /// Creates a new sleep manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets the sleep state for a chunk, creating if needed.
    pub fn get_or_create(&mut self, chunk_pos: IVec3) -> &mut ChunkWaterSleepState {
        self.chunk_states
            .entry(chunk_pos)
            .or_insert_with(ChunkWaterSleepState::new)
    }

    /// Gets the sleep state for a chunk, if it exists.
    pub fn get(&self, chunk_pos: &IVec3) -> Option<&ChunkWaterSleepState> {
        self.chunk_states.get(chunk_pos)
    }

    /// Returns true if the chunk should be simulated.
    pub fn should_simulate(&self, chunk_pos: &IVec3) -> bool {
        self.chunk_states
            .get(chunk_pos)
            .map(|s| s.should_simulate())
            .unwrap_or(true) // Default to simulating if no state exists
    }

    /// Returns true if the chunk is sleeping.
    pub fn is_sleeping(&self, chunk_pos: &IVec3) -> bool {
        self.chunk_states
            .get(chunk_pos)
            .map(|s| s.is_asleep())
            .unwrap_or(false)
    }

    /// Records simulation activity for a chunk.
    pub fn record_activity(&mut self, chunk_pos: IVec3, volume_delta: f32, flow_count: usize) {
        let state = self.get_or_create(chunk_pos);
        state.record_activity(volume_delta, flow_count);
    }

    /// Wakes a chunk and optionally its neighbors.
    pub fn wake_chunk(&mut self, chunk_pos: IVec3, reason: &str, wake_neighbors: bool) {
        // Wake the target chunk
        if let Some(state) = self.chunk_states.get_mut(&chunk_pos) {
            if state.is_asleep() {
                self.recently_woken.insert(chunk_pos);
            }
            state.wake(chunk_pos, reason);
        }

        // Optionally wake neighbors
        if wake_neighbors {
            for dx in -WAKE_RADIUS_CHUNKS..=WAKE_RADIUS_CHUNKS {
                for dy in -WAKE_RADIUS_CHUNKS..=WAKE_RADIUS_CHUNKS {
                    for dz in -WAKE_RADIUS_CHUNKS..=WAKE_RADIUS_CHUNKS {
                        if dx == 0 && dy == 0 && dz == 0 {
                            continue;
                        }
                        let neighbor = chunk_pos + IVec3::new(dx, dy, dz);
                        if let Some(state) = self.chunk_states.get_mut(&neighbor) {
                            if state.is_asleep() {
                                self.recently_woken.insert(neighbor);
                            }
                            state.wake(neighbor, "neighbor terrain change");
                        }
                    }
                }
            }
        }
    }

    /// Wakes all chunks near a global position.
    pub fn wake_near_position(&mut self, global_pos: IVec3, reason: &str) {
        use shared::CHUNK_SIZE;

        let chunk_pos = IVec3::new(
            global_pos.x.div_euclid(CHUNK_SIZE),
            global_pos.y.div_euclid(CHUNK_SIZE),
            global_pos.z.div_euclid(CHUNK_SIZE),
        );

        self.wake_chunk(chunk_pos, reason, true);
    }

    /// Updates all chunk sleep states (call once per tick after simulation).
    pub fn update_all(&mut self) {
        self.recently_woken.clear();
        self.recently_slept.clear();

        let mut sleeping = 0;
        let mut awake = 0;
        let mut waking = 0;

        for (chunk_pos, state) in &mut self.chunk_states {
            let was_asleep = state.is_asleep();
            state.update();
            let is_asleep = state.is_asleep();

            // Track state transitions
            if !was_asleep && is_asleep {
                self.recently_slept.insert(*chunk_pos);
            }

            // Count states
            match state.state {
                WaterSleepState::Asleep => sleeping += 1,
                WaterSleepState::Awake => awake += 1,
                WaterSleepState::Waking => waking += 1,
            }
        }

        // Update ticks saved (sleeping chunks × 1 tick each)
        self.stats.ticks_saved += sleeping as u64;
        self.stats.sleeping_chunks = sleeping;
        self.stats.awake_chunks = awake;
        self.stats.waking_chunks = waking;
    }

    /// Removes state for chunks that no longer have water.
    pub fn cleanup_empty_chunks(&mut self, world_map: &ServerWorldMap) {
        self.chunk_states.retain(|chunk_pos, _| {
            world_map
                .chunks
                .map
                .get(chunk_pos)
                .map(|c| !c.water.is_empty())
                .unwrap_or(false)
        });
    }

    /// Returns a summary of the sleep state (for debugging).
    pub fn summary(&self) -> String {
        format!(
            "Water Sleep: {} sleeping, {} awake, {} waking (saved {} ticks total)",
            self.stats.sleeping_chunks,
            self.stats.awake_chunks,
            self.stats.waking_chunks,
            self.stats.ticks_saved
        )
    }
}

// ============================================================================
// Bevy Systems
// ============================================================================

/// System to update water sleep states after simulation.
///
/// This runs after all water simulation systems to evaluate stability
/// and transition chunks between sleep states.
pub fn water_sleep_update_system(
    world_map: Res<ServerWorldMap>,
    mut sleep_manager: ResMut<WaterSleepManager>,
) {
    // Update all sleep states
    sleep_manager.update_all();

    // Cleanup chunks that no longer have water
    sleep_manager.cleanup_empty_chunks(&world_map);

    // Log transitions for debugging
    if !sleep_manager.recently_slept.is_empty() {
        log::debug!(
            "[WATER SLEEP] {} chunks went to sleep: {:?}",
            sleep_manager.recently_slept.len(),
            sleep_manager.recently_slept
        );
    }
    if !sleep_manager.recently_woken.is_empty() {
        log::debug!(
            "[WATER SLEEP] {} chunks woke up: {:?}",
            sleep_manager.recently_woken.len(),
            sleep_manager.recently_woken
        );
    }
}

/// System to wake water when terrain changes occur.
///
/// This monitors block placement/removal and wakes nearby sleeping water.
pub fn water_wake_on_terrain_change_system(
    world_map: Res<ServerWorldMap>,
    mut sleep_manager: ResMut<WaterSleepManager>,
) {
    // Wake water near recently removed blocks
    for pos in &world_map.chunks.recently_removed_blocks {
        sleep_manager.wake_near_position(*pos, "block removed");
    }

    // Wake water near recently placed blocks
    for pos in &world_map.chunks.recently_placed_blocks {
        sleep_manager.wake_near_position(*pos, "block placed");
    }
}

// ============================================================================
// Integration Helpers
// ============================================================================

/// Checks if a chunk should skip simulation due to sleeping.
/// Returns true if simulation should proceed, false if chunk is sleeping.
#[inline]
pub fn should_simulate_chunk(sleep_manager: &WaterSleepManager, chunk_pos: &IVec3) -> bool {
    sleep_manager.should_simulate(chunk_pos)
}

/// Records flow activity for sleep detection.
/// Call this after processing flow for a chunk.
#[inline]
pub fn record_chunk_flow_activity(
    sleep_manager: &mut WaterSleepManager,
    chunk_pos: IVec3,
    volume_delta: f32,
    flow_count: usize,
) {
    sleep_manager.record_activity(chunk_pos, volume_delta, flow_count);
}

/// Wakes water at a specific position (e.g., when water is added externally).
#[inline]
pub fn wake_water_at(sleep_manager: &mut WaterSleepManager, global_pos: IVec3) {
    sleep_manager.wake_near_position(global_pos, "external water change");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sleep_state_transitions() {
        let mut state = ChunkWaterSleepState::new();
        assert_eq!(state.state, WaterSleepState::Awake);

        // Record no activity for enough ticks to sleep
        for _ in 0..MIN_STABLE_TICKS_TO_SLEEP {
            state.record_activity(0.0, 0);
            state.update();
        }

        assert_eq!(state.state, WaterSleepState::Asleep);
    }

    #[test]
    fn test_activity_prevents_sleep() {
        let mut state = ChunkWaterSleepState::new();

        // Record activity (volume changes)
        for _ in 0..MIN_STABLE_TICKS_TO_SLEEP {
            state.record_activity(0.5, 5);
            state.update();
        }

        // Should still be awake due to activity
        assert_eq!(state.state, WaterSleepState::Awake);
    }

    #[test]
    fn test_wake_from_sleep() {
        let mut state = ChunkWaterSleepState::new();

        // Sleep the chunk
        for _ in 0..MIN_STABLE_TICKS_TO_SLEEP {
            state.record_activity(0.0, 0);
            state.update();
        }
        assert!(state.is_asleep());

        // Wake it
        let test_chunk_pos = IVec3::new(0, 0, 0);
        state.wake(test_chunk_pos, "test");
        assert_eq!(state.state, WaterSleepState::Waking);
        assert!(!state.is_asleep());
        assert!(state.should_simulate());
    }

    #[test]
    fn test_wake_cooldown() {
        let mut state = ChunkWaterSleepState::new();
        state.state = WaterSleepState::Waking;
        state.ticks_since_wake = 0;

        // Should stay waking until cooldown expires
        for _ in 0..MIN_AWAKE_TICKS_AFTER_WAKE {
            assert_eq!(state.state, WaterSleepState::Waking);
            state.update();
        }

        // After cooldown, should be awake
        assert_eq!(state.state, WaterSleepState::Awake);
    }

    #[test]
    fn test_manager_should_simulate() {
        let mut manager = WaterSleepManager::new();
        let chunk = IVec3::new(0, 0, 0);

        // Unknown chunk should be simulated (conservative)
        assert!(manager.should_simulate(&chunk));

        // Create state and sleep it
        let state = manager.get_or_create(chunk);
        state.state = WaterSleepState::Asleep;

        // Sleeping chunk should not be simulated
        assert!(!manager.should_simulate(&chunk));
    }

    #[test]
    fn test_manager_wake_neighbors() {
        let mut manager = WaterSleepManager::new();
        let center = IVec3::new(5, 5, 5);
        let neighbor = IVec3::new(6, 5, 5);

        // Set up both chunks as sleeping
        manager.get_or_create(center).state = WaterSleepState::Asleep;
        manager.get_or_create(neighbor).state = WaterSleepState::Asleep;

        // Wake center with neighbors
        manager.wake_chunk(center, "test", true);

        // Both should be waking
        assert_eq!(manager.get(&center).unwrap().state, WaterSleepState::Waking);
        assert_eq!(
            manager.get(&neighbor).unwrap().state,
            WaterSleepState::Waking
        );
    }
}

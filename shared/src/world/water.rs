//! Volume-based water storage system.
//!
//! This module implements water as a conserved volume rather than simple block types.
//! Water is stored sparsely per-voxel with volume and derived surface height.
//!
//! ## Design Principles
//! - Water is discrete in storage, continuous in behavior
//! - Simulation is local, bounded, and conservative
//! - Rendering is decoupled from simulation
//!
//! ## Data Model
//! - Each water cell stores a volume (0.0 to 1.0, where 1.0 = full voxel)
//! - Surface height is derived from volume relative to cell bottom
//! - Only voxels with water are stored (sparse representation)

use bevy::math::IVec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Maximum water volume a single voxel can contain.
/// This represents a completely full block of water.
pub const MAX_WATER_VOLUME: f32 = 1.0;

/// Minimum water volume threshold. Below this, water is considered empty
/// and removed from storage to maintain sparsity.
pub const MIN_WATER_VOLUME: f32 = 0.001;

/// Height of a full water block relative to the voxel bottom (in block units).
/// Water at MAX_WATER_VOLUME will have a surface at this height.
pub const FULL_WATER_HEIGHT: f32 = 0.875; // 14/16 of a block, matching previous visual

/// Represents water stored in a single voxel.
///
/// Water volume is normalized: 0.0 = empty, 1.0 = full voxel.
/// The surface_height is derived from volume and represents the visual
/// water level within the voxel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WaterCell {
    /// Water volume in this cell (0.0 to 1.0)
    /// 1.0 represents a full voxel of water
    volume: f32,
}

impl WaterCell {
    /// Creates a new water cell with the specified volume.
    ///
    /// # Arguments
    /// * `volume` - Water volume (clamped to 0.0..=1.0)
    ///
    /// # Returns
    /// A new WaterCell, or None if volume is below MIN_WATER_VOLUME
    pub fn new(volume: f32) -> Option<Self> {
        let clamped = volume.clamp(0.0, MAX_WATER_VOLUME);
        if clamped < MIN_WATER_VOLUME {
            None
        } else {
            Some(Self { volume: clamped })
        }
    }

    /// Creates a full water cell (volume = 1.0)
    pub fn full() -> Self {
        Self {
            volume: MAX_WATER_VOLUME,
        }
    }

    /// Returns the water volume in this cell
    #[inline]
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Returns the surface height relative to the bottom of this voxel.
    /// This is used for rendering the water surface.
    ///
    /// For a full cell, this returns FULL_WATER_HEIGHT.
    /// For partial cells, height scales linearly with volume.
    #[inline]
    pub fn surface_height(&self) -> f32 {
        self.volume * FULL_WATER_HEIGHT
    }

    /// Adds volume to this cell, returning any overflow.
    ///
    /// # Arguments
    /// * `amount` - Volume to add
    ///
    /// # Returns
    /// The amount of water that couldn't fit (overflow)
    pub fn add_volume(&mut self, amount: f32) -> f32 {
        let new_volume = self.volume + amount;
        if new_volume > MAX_WATER_VOLUME {
            let overflow = new_volume - MAX_WATER_VOLUME;
            self.volume = MAX_WATER_VOLUME;
            overflow
        } else {
            self.volume = new_volume;
            0.0
        }
    }

    /// Removes volume from this cell, returning the amount actually removed.
    ///
    /// # Arguments
    /// * `amount` - Volume to remove
    ///
    /// # Returns
    /// The amount of water actually removed (may be less than requested)
    pub fn remove_volume(&mut self, amount: f32) -> f32 {
        let removed = amount.min(self.volume);
        self.volume -= removed;
        removed
    }

    /// Returns true if this cell should be removed from storage (volume too low)
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.volume < MIN_WATER_VOLUME
    }

    /// Returns true if this cell is completely full
    #[inline]
    pub fn is_full(&self) -> bool {
        self.volume >= MAX_WATER_VOLUME - MIN_WATER_VOLUME
    }
}

impl Default for WaterCell {
    fn default() -> Self {
        Self::full()
    }
}

/// Sparse storage for water volumes within a chunk.
///
/// Uses a HashMap for O(1) access while only storing voxels that contain water.
/// This is memory-efficient since most voxels in a typical chunk don't contain water.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkWaterStorage {
    /// Maps local voxel position to water cell data
    /// Positions are relative to chunk origin (0..CHUNK_SIZE for each axis)
    cells: HashMap<IVec3, WaterCell>,
}

impl ChunkWaterStorage {
    /// Creates a new empty water storage
    pub fn new() -> Self {
        Self {
            cells: HashMap::new(),
        }
    }

    /// Gets the water cell at the specified local position, if any
    pub fn get(&self, pos: &IVec3) -> Option<&WaterCell> {
        self.cells.get(pos)
    }

    /// Gets a mutable reference to the water cell at the specified local position
    pub fn get_mut(&mut self, pos: &IVec3) -> Option<&mut WaterCell> {
        self.cells.get_mut(pos)
    }

    /// Sets water at the specified local position.
    ///
    /// # Arguments
    /// * `pos` - Local position within the chunk
    /// * `volume` - Water volume (0.0 to 1.0)
    ///
    /// If volume is below MIN_WATER_VOLUME, the cell is removed instead.
    pub fn set(&mut self, pos: IVec3, volume: f32) {
        if let Some(cell) = WaterCell::new(volume) {
            self.cells.insert(pos, cell);
        } else {
            self.cells.remove(&pos);
        }
    }

    /// Sets a full water cell at the specified position
    pub fn set_full(&mut self, pos: IVec3) {
        self.cells.insert(pos, WaterCell::full());
    }

    /// Removes water at the specified position
    pub fn remove(&mut self, pos: &IVec3) -> Option<WaterCell> {
        self.cells.remove(pos)
    }

    /// Returns true if there's water at the specified position
    pub fn has_water(&self, pos: &IVec3) -> bool {
        self.cells.contains_key(pos)
    }

    /// Returns the volume at the specified position (0.0 if no water)
    pub fn volume_at(&self, pos: &IVec3) -> f32 {
        self.cells.get(pos).map(|c| c.volume()).unwrap_or(0.0)
    }

    /// Returns the surface height at the specified position (0.0 if no water)
    pub fn surface_height_at(&self, pos: &IVec3) -> f32 {
        self.cells
            .get(pos)
            .map(|c| c.surface_height())
            .unwrap_or(0.0)
    }

    /// Returns an iterator over all water cells
    pub fn iter(&self) -> impl Iterator<Item = (&IVec3, &WaterCell)> {
        self.cells.iter()
    }

    /// Returns a mutable iterator over all water cells
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&IVec3, &mut WaterCell)> {
        self.cells.iter_mut()
    }

    /// Returns the number of water cells in this storage
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Returns true if there are no water cells
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Removes all empty cells (volume < MIN_WATER_VOLUME) from storage
    pub fn cleanup_empty_cells(&mut self) {
        self.cells.retain(|_, cell| !cell.is_empty());
    }

    /// Returns the total water volume in this chunk
    pub fn total_volume(&self) -> f32 {
        self.cells.values().map(|c| c.volume()).sum()
    }

    /// Clears all water from this storage
    pub fn clear(&mut self) {
        self.cells.clear();
    }
}

/// Utility functions for water-related calculations
pub mod water_utils {
    use super::*;
    use crate::CHUNK_SIZE;

    /// Converts global block coordinates to chunk-local water storage coordinates
    pub fn global_to_local(global_pos: IVec3, chunk_pos: IVec3) -> IVec3 {
        IVec3::new(
            global_pos.x - chunk_pos.x * CHUNK_SIZE,
            global_pos.y - chunk_pos.y * CHUNK_SIZE,
            global_pos.z - chunk_pos.z * CHUNK_SIZE,
        )
    }

    /// Checks if a local position is within valid chunk bounds
    pub fn is_valid_local_pos(pos: &IVec3) -> bool {
        pos.x >= 0
            && pos.x < CHUNK_SIZE
            && pos.y >= 0
            && pos.y < CHUNK_SIZE
            && pos.z >= 0
            && pos.z < CHUNK_SIZE
    }

    /// Calculates the global water surface height at a given global position
    /// Returns the Y coordinate of the water surface (block Y + surface height within block)
    pub fn global_surface_height(global_pos: IVec3, cell: &WaterCell) -> f32 {
        global_pos.y as f32 + cell.surface_height()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_water_cell_creation() {
        // Full cell
        let full = WaterCell::full();
        assert!(full.is_full());
        assert!(!full.is_empty());
        assert_eq!(full.volume(), MAX_WATER_VOLUME);

        // Partial cell
        let partial = WaterCell::new(0.5).unwrap();
        assert_eq!(partial.volume(), 0.5);
        assert!(!partial.is_full());
        assert!(!partial.is_empty());

        // Empty cell (should return None)
        let empty = WaterCell::new(0.0);
        assert!(empty.is_none());

        // Below threshold (should return None)
        let tiny = WaterCell::new(MIN_WATER_VOLUME / 2.0);
        assert!(tiny.is_none());
    }

    #[test]
    fn test_water_cell_volume_operations() {
        let mut cell = WaterCell::new(0.5).unwrap();

        // Add without overflow
        let overflow = cell.add_volume(0.3);
        assert_eq!(overflow, 0.0);
        assert!((cell.volume() - 0.8).abs() < f32::EPSILON);

        // Add with overflow
        let overflow = cell.add_volume(0.5);
        assert!((overflow - 0.3).abs() < 0.001);
        assert!(cell.is_full());

        // Remove partial
        let removed = cell.remove_volume(0.3);
        assert!((removed - 0.3).abs() < f32::EPSILON);
        assert!((cell.volume() - 0.7).abs() < 0.001);

        // Remove more than available
        let removed = cell.remove_volume(1.0);
        assert!((removed - 0.7).abs() < 0.001);
        assert!(cell.is_empty());
    }

    #[test]
    fn test_chunk_water_storage() {
        let mut storage = ChunkWaterStorage::new();
        let pos1 = IVec3::new(5, 10, 5);
        let pos2 = IVec3::new(6, 10, 5);

        // Initially empty
        assert!(storage.is_empty());
        assert!(!storage.has_water(&pos1));

        // Add water
        storage.set_full(pos1);
        assert!(storage.has_water(&pos1));
        assert_eq!(storage.len(), 1);

        // Add partial water
        storage.set(pos2, 0.5);
        assert!(storage.has_water(&pos2));
        assert_eq!(storage.len(), 2);
        assert!((storage.volume_at(&pos2) - 0.5).abs() < f32::EPSILON);

        // Remove water
        storage.remove(&pos1);
        assert!(!storage.has_water(&pos1));
        assert_eq!(storage.len(), 1);

        // Set to empty removes cell
        storage.set(pos2, 0.0);
        assert!(!storage.has_water(&pos2));
        assert!(storage.is_empty());
    }

    #[test]
    fn test_surface_height() {
        let full = WaterCell::full();
        assert!((full.surface_height() - FULL_WATER_HEIGHT).abs() < f32::EPSILON);

        let half = WaterCell::new(0.5).unwrap();
        assert!((half.surface_height() - FULL_WATER_HEIGHT * 0.5).abs() < f32::EPSILON);
    }
}

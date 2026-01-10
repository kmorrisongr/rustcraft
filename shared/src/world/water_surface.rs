//! Water surface detection system.
//!
//! This module identifies water surfaces and groups them into patches for
//! efficient simulation. A water cell is considered a "surface cell" if the
//! voxel directly above it is air (not solid and not water).
//!
//! ## Surface Patches
//! Connected surface cells are grouped into surface patches. Each patch is
//! approximately horizontal and treated as a local heightfield for simulation.
//! Multiple patches can exist in the same XZ column at different Y levels.
//!
//! ## Usage
//! Surface detection is used to:
//! 1. Optimize simulation (only simulate surfaces, not all water volume)
//! 2. Generate efficient meshes for rendering
//! 3. Enable shallow-water-style wave simulation

use bevy::math::IVec3;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

use super::water::ChunkWaterStorage;
use crate::CHUNK_SIZE;

/// A unique identifier for a surface patch within a chunk.
/// Patches are numbered starting from 0.
pub type SurfacePatchId = u32;

/// Bounding box for a patch, defined by minimum and maximum coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundingBox {
    /// Minimum corner (local coordinates)
    pub min: IVec3,
    /// Maximum corner (local coordinates)
    pub max: IVec3,
}

impl BoundingBox {
    /// Creates a new bounding box from a single point.
    pub fn from_point(pos: IVec3) -> Self {
        Self { min: pos, max: pos }
    }

    /// Expands the bounding box to include the given point.
    pub fn expand(&mut self, pos: IVec3) {
        self.min = self.min.min(pos);
        self.max = self.max.max(pos);
    }

    /// Returns the XZ extent of this bounding box (width, depth).
    pub fn xz_extent(&self) -> (i32, i32) {
        (
            self.max.x - self.min.x + 1,
            self.max.z - self.min.z + 1,
        )
    }
}

/// Represents a single water surface cell.
///
/// A surface cell is a water cell where the voxel directly above is air.
/// This is where waves propagate and where mesh vertices should be placed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WaterSurfaceCell {
    /// Local position within the chunk
    pub local_pos: IVec3,
    /// Water volume at this cell (determines surface height)
    pub volume: f32,
    /// The ID of the patch this cell belongs to
    pub patch_id: SurfacePatchId,
}

impl WaterSurfaceCell {
    /// Creates a new surface cell.
    pub fn new(local_pos: IVec3, volume: f32, patch_id: SurfacePatchId) -> Self {
        Self {
            local_pos,
            volume,
            patch_id,
        }
    }

    /// Returns the surface height relative to the bottom of this voxel.
    /// For a full cell (volume=1.0), this returns FULL_WATER_HEIGHT.
    #[inline]
    pub fn surface_height(&self) -> f32 {
        self.volume * super::FULL_WATER_HEIGHT
    }

    /// Returns the global Y coordinate of the water surface.
    #[inline]
    pub fn global_surface_y(&self, chunk_y: i32) -> f32 {
        let global_y = chunk_y * CHUNK_SIZE + self.local_pos.y;
        global_y as f32 + self.surface_height()
    }
}

/// A connected group of water surface cells at approximately the same elevation.
///
/// Surface patches are used for:
/// - Efficient shallow-water simulation (waves propagate within a patch)
/// - Mesh generation (one mesh per patch)
/// - Sleep detection (patches can sleep when stable)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterSurfacePatch {
    /// Unique ID of this patch within the chunk
    pub id: SurfacePatchId,
    /// All surface cells belonging to this patch
    pub cells: Vec<IVec3>,
    /// Bounding box (local coordinates), None if patch is empty
    pub bounds: Option<BoundingBox>,
    /// Average Y level of the patch (for quick filtering)
    pub avg_y: f32,
    /// Whether this patch is stable (no recent changes)
    pub is_stable: bool,
}

impl WaterSurfacePatch {
    /// Creates a new empty patch with the given ID.
    pub fn new(id: SurfacePatchId) -> Self {
        Self {
            id,
            cells: Vec::new(),
            bounds: None,
            avg_y: 0.0,
            is_stable: false,
        }
    }

    /// Adds a cell to this patch and updates bounds.
    pub fn add_cell(&mut self, local_pos: IVec3) {
        self.cells.push(local_pos);
        
        // Update bounds
        match &mut self.bounds {
            Some(bounds) => bounds.expand(local_pos),
            None => self.bounds = Some(BoundingBox::from_point(local_pos)),
        }
        
        // Update average Y incrementally
        let len = self.cells.len();
        debug_assert!(len > 0, "cells should not be empty after push");
        self.avg_y = (self.avg_y * (len - 1) as f32 + local_pos.y as f32) / len as f32;
    }

    /// Returns true if this patch contains the given position.
    #[inline]
    pub fn contains(&self, pos: &IVec3) -> bool {
        self.cells.contains(pos)
    }

    /// Returns the number of cells in this patch.
    #[inline]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Returns true if this patch has no cells.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Returns the XZ extent of this patch (for area estimation).
    /// Returns (0, 0) if the patch is empty.
    pub fn xz_extent(&self) -> (i32, i32) {
        self.bounds.map_or((0, 0), |b| b.xz_extent())
    }
}

impl Default for WaterSurfacePatch {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Storage for detected water surfaces within a chunk.
///
/// This structure caches surface detection results and is rebuilt when
/// the chunk's water state changes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkWaterSurfaces {
    /// All surface cells, keyed by local position for quick lookup
    surface_cells: HashMap<IVec3, WaterSurfaceCell>,
    /// Surface patches (connected groups of surface cells)
    patches: Vec<WaterSurfacePatch>,
    /// Maps cell position to patch ID for quick lookup
    cell_to_patch: HashMap<IVec3, SurfacePatchId>,
    /// Generation counter - incremented each time surfaces are rebuilt
    generation: u64,
}

impl ChunkWaterSurfaces {
    /// Creates a new empty surface storage.
    pub fn new() -> Self {
        Self {
            surface_cells: HashMap::new(),
            patches: Vec::new(),
            cell_to_patch: HashMap::new(),
            generation: 0,
        }
    }

    /// Returns the surface cell at the given position, if any.
    pub fn get(&self, pos: &IVec3) -> Option<&WaterSurfaceCell> {
        self.surface_cells.get(pos)
    }

    /// Returns true if there's a surface cell at the given position.
    #[inline]
    pub fn has_surface(&self, pos: &IVec3) -> bool {
        self.surface_cells.contains_key(pos)
    }

    /// Alias for has_surface - returns true if this is a surface cell.
    #[inline]
    pub fn is_surface(&self, pos: &IVec3) -> bool {
        self.surface_cells.contains_key(pos)
    }

    /// Returns all surface cells.
    pub fn cells(&self) -> impl Iterator<Item = &WaterSurfaceCell> {
        self.surface_cells.values()
    }

    /// Returns all surface cell positions.
    pub fn cell_positions(&self) -> impl Iterator<Item = &IVec3> {
        self.surface_cells.keys()
    }

    /// Returns the number of surface cells.
    #[inline]
    pub fn cell_count(&self) -> usize {
        self.surface_cells.len()
    }

    /// Returns all patches.
    pub fn patches(&self) -> &[WaterSurfacePatch] {
        &self.patches
    }

    /// Returns the patch containing the given position, if any.
    pub fn patch_at(&self, pos: &IVec3) -> Option<&WaterSurfacePatch> {
        self.cell_to_patch
            .get(pos)
            .and_then(|&id| self.patches.get(id as usize))
    }

    /// Returns the patch with the given ID.
    pub fn patch(&self, id: SurfacePatchId) -> Option<&WaterSurfacePatch> {
        self.patches.get(id as usize)
    }

    /// Returns the number of patches.
    #[inline]
    pub fn patch_count(&self) -> usize {
        self.patches.len()
    }

    /// Returns the generation counter (useful for cache invalidation).
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Clears all surface data.
    pub fn clear(&mut self) {
        self.surface_cells.clear();
        self.patches.clear();
        self.cell_to_patch.clear();
        self.generation += 1;
    }

    /// Detects water surfaces in the given chunk and groups them into patches.
    ///
    /// A cell is considered a surface cell if:
    /// 1. It contains water
    /// 2. The voxel directly above is not solid (air or water-passable)
    ///
    /// # Arguments
    /// * `water` - The chunk's water storage
    /// * `is_solid_above` - Callback to check if a position above is solid
    ///                      Returns true if the block at the given local position is solid
    ///
    /// # Note
    /// For boundary cells (y = CHUNK_SIZE - 1), the caller should provide
    /// cross-chunk lookups via `is_solid_above` for accurate detection.
    pub fn detect_surfaces<F>(&mut self, water: &ChunkWaterStorage, is_solid_above: F)
    where
        F: Fn(IVec3) -> bool,
    {
        self.clear();

        // First pass: identify all surface cells
        for (pos, cell) in water.iter() {
            let above_pos = *pos + IVec3::new(0, 1, 0);

            // Check if above is solid
            let above_is_solid = is_solid_above(above_pos);

            // Check if above has water (water on water is not a surface)
            let above_has_water = water.has_water(&above_pos);

            // A surface cell has air (non-solid, non-water) above
            if !above_is_solid && !above_has_water {
                self.surface_cells.insert(
                    *pos,
                    WaterSurfaceCell::new(*pos, cell.volume(), 0), // patch_id assigned later
                );
            }
        }

        // Second pass: group surface cells into connected patches using flood fill
        self.build_patches();
    }

    /// Groups surface cells into connected patches using flood fill.
    fn build_patches(&mut self) {
        let mut visited: HashSet<IVec3> = HashSet::new();
        let mut current_patch_id: SurfacePatchId = 0;

        // Get all surface cell positions
        let surface_positions: Vec<IVec3> = self.surface_cells.keys().copied().collect();

        for start_pos in surface_positions {
            if visited.contains(&start_pos) {
                continue;
            }

            // Start a new patch using flood fill
            let mut patch = WaterSurfacePatch::new(current_patch_id);
            let mut queue: VecDeque<IVec3> = VecDeque::new();
            queue.push_back(start_pos);
            visited.insert(start_pos);

            while let Some(pos) = queue.pop_front() {
                // Add cell to patch
                patch.add_cell(pos);
                self.cell_to_patch.insert(pos, current_patch_id);

                // Update cell's patch ID
                if let Some(cell) = self.surface_cells.get_mut(&pos) {
                    cell.patch_id = current_patch_id;
                }

                // Check 4-connected neighbors (lateral connectivity only)
                // Vertical connectivity would connect stacked water bodies
                let neighbors = [
                    pos + IVec3::new(1, 0, 0),
                    pos + IVec3::new(-1, 0, 0),
                    pos + IVec3::new(0, 0, 1),
                    pos + IVec3::new(0, 0, -1),
                ];

                for neighbor in neighbors {
                    if !visited.contains(&neighbor) && self.surface_cells.contains_key(&neighbor) {
                        // Check if neighbor is at similar Y level (within 1 block)
                        // This allows patches to flow over small height variations
                        let y_diff = (neighbor.y - pos.y).abs();
                        if y_diff <= 1 {
                            visited.insert(neighbor);
                            queue.push_back(neighbor);
                        }
                    }
                }
            }

            if !patch.is_empty() {
                self.patches.push(patch);
                current_patch_id += 1;
            }
        }
    }

    /// Invalidates surface data at a specific position.
    /// Call this when water at a position changes.
    pub fn invalidate_at(&mut self, _pos: &IVec3) {
        // For now, we just increment generation to signal that data may be stale.
        // A more sophisticated approach would do incremental updates.
        self.generation += 1;
    }
}

/// Summary statistics for water surfaces in a chunk.
#[derive(Debug, Clone, Copy, Default)]
pub struct WaterSurfaceStats {
    /// Total number of surface cells
    pub total_cells: usize,
    /// Number of patches
    pub patch_count: usize,
    /// Largest patch size (cells)
    pub largest_patch: usize,
    /// Smallest patch size (cells)
    pub smallest_patch: usize,
    /// Average patch size
    pub avg_patch_size: f32,
}

impl ChunkWaterSurfaces {
    /// Computes statistics about the detected surfaces.
    pub fn stats(&self) -> WaterSurfaceStats {
        if self.patches.is_empty() {
            return WaterSurfaceStats::default();
        }

        let sizes: Vec<usize> = self.patches.iter().map(|p| p.len()).collect();
        let largest = *sizes.iter().max().unwrap_or(&0);
        let smallest = *sizes.iter().min().unwrap_or(&0);
        let total: usize = sizes.iter().sum();
        let avg = total as f32 / sizes.len() as f32;

        WaterSurfaceStats {
            total_cells: self.surface_cells.len(),
            patch_count: self.patches.len(),
            largest_patch: largest,
            smallest_patch: smallest,
            avg_patch_size: avg,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_detection_single_cell() {
        let mut water = ChunkWaterStorage::new();
        water.set_full(IVec3::new(5, 10, 5));

        let mut surfaces = ChunkWaterSurfaces::new();
        // No solid blocks above
        surfaces.detect_surfaces(&water, |_| false);

        assert_eq!(surfaces.cell_count(), 1);
        assert_eq!(surfaces.patch_count(), 1);
        assert!(surfaces.has_surface(&IVec3::new(5, 10, 5)));
    }

    #[test]
    fn test_surface_detection_covered_water() {
        let mut water = ChunkWaterStorage::new();
        water.set_full(IVec3::new(5, 10, 5));

        let mut surfaces = ChunkWaterSurfaces::new();
        // Solid block above
        surfaces.detect_surfaces(&water, |pos| pos == IVec3::new(5, 11, 5));

        // Should not detect as surface because it's covered
        assert_eq!(surfaces.cell_count(), 0);
    }

    #[test]
    fn test_surface_detection_water_on_water() {
        let mut water = ChunkWaterStorage::new();
        water.set_full(IVec3::new(5, 10, 5));
        water.set_full(IVec3::new(5, 11, 5)); // Water above

        let mut surfaces = ChunkWaterSurfaces::new();
        surfaces.detect_surfaces(&water, |_| false);

        // Only the top cell should be a surface
        assert_eq!(surfaces.cell_count(), 1);
        assert!(surfaces.has_surface(&IVec3::new(5, 11, 5)));
        assert!(!surfaces.has_surface(&IVec3::new(5, 10, 5)));
    }

    #[test]
    fn test_patch_grouping_connected() {
        let mut water = ChunkWaterStorage::new();
        // Create a 2x2 water surface
        water.set_full(IVec3::new(5, 10, 5));
        water.set_full(IVec3::new(6, 10, 5));
        water.set_full(IVec3::new(5, 10, 6));
        water.set_full(IVec3::new(6, 10, 6));

        let mut surfaces = ChunkWaterSurfaces::new();
        surfaces.detect_surfaces(&water, |_| false);

        // All 4 cells should be in the same patch
        assert_eq!(surfaces.cell_count(), 4);
        assert_eq!(surfaces.patch_count(), 1);

        let patch = surfaces.patch(0).unwrap();
        assert_eq!(patch.len(), 4);
    }

    #[test]
    fn test_patch_grouping_disconnected() {
        let mut water = ChunkWaterStorage::new();
        // Create two separate water pools
        water.set_full(IVec3::new(2, 10, 2));
        water.set_full(IVec3::new(12, 10, 12));

        let mut surfaces = ChunkWaterSurfaces::new();
        surfaces.detect_surfaces(&water, |_| false);

        // Should create two separate patches
        assert_eq!(surfaces.cell_count(), 2);
        assert_eq!(surfaces.patch_count(), 2);
    }

    #[test]
    fn test_patch_grouping_different_heights() {
        let mut water = ChunkWaterStorage::new();
        // Create water at different Y levels
        water.set_full(IVec3::new(5, 10, 5));
        water.set_full(IVec3::new(5, 20, 5)); // Same XZ, different Y

        let mut surfaces = ChunkWaterSurfaces::new();
        surfaces.detect_surfaces(&water, |_| false);

        // Should be two separate patches (different Y levels)
        assert_eq!(surfaces.cell_count(), 2);
        assert_eq!(surfaces.patch_count(), 2);
    }

    #[test]
    fn test_surface_cell_height() {
        let cell = WaterSurfaceCell::new(IVec3::new(0, 10, 0), 1.0, 0);
        assert!((cell.surface_height() - super::super::FULL_WATER_HEIGHT).abs() < f32::EPSILON);

        let half_cell = WaterSurfaceCell::new(IVec3::new(0, 10, 0), 0.5, 0);
        assert!(
            (half_cell.surface_height() - super::super::FULL_WATER_HEIGHT * 0.5).abs()
                < f32::EPSILON
        );
    }

    #[test]
    fn test_stats() {
        let mut water = ChunkWaterStorage::new();
        // Create patches of different sizes
        water.set_full(IVec3::new(2, 10, 2));
        water.set_full(IVec3::new(3, 10, 2));
        water.set_full(IVec3::new(12, 10, 12));

        let mut surfaces = ChunkWaterSurfaces::new();
        surfaces.detect_surfaces(&water, |_| false);

        let stats = surfaces.stats();
        assert_eq!(stats.total_cells, 3);
        assert_eq!(stats.patch_count, 2);
        assert_eq!(stats.largest_patch, 2);
        assert_eq!(stats.smallest_patch, 1);
    }

    #[test]
    fn test_patch_bounds() {
        let mut water = ChunkWaterStorage::new();
        water.set_full(IVec3::new(2, 10, 3));
        water.set_full(IVec3::new(5, 10, 3));
        water.set_full(IVec3::new(3, 10, 3));
        water.set_full(IVec3::new(4, 10, 3));

        let mut surfaces = ChunkWaterSurfaces::new();
        surfaces.detect_surfaces(&water, |_| false);

        let patch = surfaces.patch(0).unwrap();
        let bounds = patch.bounds.expect("patch should have bounds");
        assert_eq!(bounds.min, IVec3::new(2, 10, 3));
        assert_eq!(bounds.max, IVec3::new(5, 10, 3));
    }
    
    #[test]
    fn test_empty_patch_bounds() {
        let patch = WaterSurfacePatch::new(0);
        assert_eq!(patch.bounds, None);
        assert_eq!(patch.xz_extent(), (0, 0));
        assert!(patch.is_empty());
    }
}

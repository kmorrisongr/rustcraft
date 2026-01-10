//! Water chunk boundary exchange system.
//!
//! This module handles water flow across chunk boundaries by maintaining
//! a cache of water state at chunk edges ("ghost cells" pattern).
//!
//! ## Design
//! - Each chunk edge has 4 faces (±X, ±Z at each Y level, ±Y at each XZ)
//! - We cache water volumes at boundary cells for neighbor lookups
//! - When water flows to a boundary, the neighbor chunk is notified
//! - Updates are propagated lazily (only when simulation needs them)
//!
//! ## Coordinate Convention
//! - Local coords: 0 to CHUNK_SIZE-1 for each axis
//! - Boundary cells: positions where any axis is 0 or CHUNK_SIZE-1

use bevy::prelude::*;
use shared::world::{ServerWorldMap, MIN_WATER_VOLUME};
use shared::CHUNK_SIZE;
use std::collections::{HashMap, HashSet};

/// Represents a face direction for chunk boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoundaryFace {
    /// Negative X face (local x = 0)
    NegX,
    /// Positive X face (local x = CHUNK_SIZE - 1)
    PosX,
    /// Negative Y face (local y = 0)
    NegY,
    /// Positive Y face (local y = CHUNK_SIZE - 1)
    PosY,
    /// Negative Z face (local z = 0)
    NegZ,
    /// Positive Z face (local z = CHUNK_SIZE - 1)
    PosZ,
}

impl BoundaryFace {
    /// Returns the offset to the neighboring chunk in this direction.
    pub fn neighbor_chunk_offset(&self) -> IVec3 {
        match self {
            BoundaryFace::NegX => IVec3::new(-1, 0, 0),
            BoundaryFace::PosX => IVec3::new(1, 0, 0),
            BoundaryFace::NegY => IVec3::new(0, -1, 0),
            BoundaryFace::PosY => IVec3::new(0, 1, 0),
            BoundaryFace::NegZ => IVec3::new(0, 0, -1),
            BoundaryFace::PosZ => IVec3::new(0, 0, 1),
        }
    }

    /// Returns the opposite face (what the neighbor sees this chunk as).
    pub fn opposite(&self) -> BoundaryFace {
        match self {
            BoundaryFace::NegX => BoundaryFace::PosX,
            BoundaryFace::PosX => BoundaryFace::NegX,
            BoundaryFace::NegY => BoundaryFace::PosY,
            BoundaryFace::PosY => BoundaryFace::NegY,
            BoundaryFace::NegZ => BoundaryFace::PosZ,
            BoundaryFace::PosZ => BoundaryFace::NegZ,
        }
    }

    /// All six boundary faces.
    pub const ALL: [BoundaryFace; 6] = [
        BoundaryFace::NegX,
        BoundaryFace::PosX,
        BoundaryFace::NegY,
        BoundaryFace::PosY,
        BoundaryFace::NegZ,
        BoundaryFace::PosZ,
    ];

    /// Horizontal faces only (for lateral flow).
    pub const HORIZONTAL: [BoundaryFace; 4] = [
        BoundaryFace::NegX,
        BoundaryFace::PosX,
        BoundaryFace::NegZ,
        BoundaryFace::PosZ,
    ];
}

/// Water state at a single boundary cell.
#[derive(Debug, Clone, Copy, Default)]
pub struct BoundaryWaterCell {
    /// Water volume at this cell
    pub volume: f32,
    /// Whether this cell is a surface (air above)
    pub is_surface: bool,
}

/// Cached water state for one face of a chunk boundary.
///
/// Stores water volumes for cells along one face of the chunk.
/// The 2D coordinates within the face depend on which face:
/// - X faces: keyed by (y, z)
/// - Y faces: keyed by (x, z)
/// - Z faces: keyed by (x, y)
#[derive(Debug, Clone, Default)]
pub struct BoundaryFaceData {
    /// Water cells on this face, keyed by 2D position within the face.
    /// The interpretation of the IVec2 depends on the face direction.
    pub cells: HashMap<IVec2, BoundaryWaterCell>,
}

impl BoundaryFaceData {
    /// Creates an empty boundary face.
    pub fn new() -> Self {
        Self {
            cells: HashMap::new(),
        }
    }

    /// Sets water data for a cell on this face.
    pub fn set(&mut self, face_pos: IVec2, volume: f32, is_surface: bool) {
        if volume >= MIN_WATER_VOLUME {
            self.cells
                .insert(face_pos, BoundaryWaterCell { volume, is_surface });
        } else {
            self.cells.remove(&face_pos);
        }
    }

    /// Gets water data for a cell on this face.
    pub fn get(&self, face_pos: &IVec2) -> Option<&BoundaryWaterCell> {
        self.cells.get(face_pos)
    }

    /// Returns true if there's any water on this face.
    pub fn has_water(&self) -> bool {
        !self.cells.is_empty()
    }
}

/// Complete boundary water data for a chunk (all 6 faces).
#[derive(Debug, Clone, Default)]
pub struct ChunkBoundaryWater {
    /// Water data for each face
    faces: HashMap<BoundaryFace, BoundaryFaceData>,
    /// Generation counter - incremented when boundary data changes
    pub generation: u64,
}

impl ChunkBoundaryWater {
    /// Creates new empty boundary data.
    pub fn new() -> Self {
        Self {
            faces: HashMap::new(),
            generation: 0,
        }
    }

    /// Gets the boundary data for a specific face.
    pub fn face(&self, face: BoundaryFace) -> Option<&BoundaryFaceData> {
        self.faces.get(&face)
    }

    /// Gets mutable boundary data for a specific face, creating if needed.
    pub fn face_mut(&mut self, face: BoundaryFace) -> &mut BoundaryFaceData {
        self.faces.entry(face).or_insert_with(BoundaryFaceData::new)
    }

    /// Returns true if there's any water on any boundary.
    pub fn has_boundary_water(&self) -> bool {
        self.faces.values().any(|f| f.has_water())
    }

    /// Clears all boundary data.
    pub fn clear(&mut self) {
        self.faces.clear();
        self.generation += 1;
    }
}

/// Resource storing boundary water data for all chunks.
///
/// This is separate from chunk storage to allow efficient cross-chunk lookups
/// without borrowing the entire world map.
#[derive(Resource, Default)]
pub struct WaterBoundaryCache {
    /// Boundary data per chunk
    boundaries: HashMap<IVec3, ChunkBoundaryWater>,
    /// Chunks whose boundaries have been modified and need neighbor notification
    pub dirty_chunks: HashSet<IVec3>,
}

impl WaterBoundaryCache {
    /// Creates a new empty cache.
    pub fn new() -> Self {
        Self {
            boundaries: HashMap::new(),
            dirty_chunks: HashSet::new(),
        }
    }

    /// Gets boundary data for a chunk.
    pub fn get(&self, chunk_pos: &IVec3) -> Option<&ChunkBoundaryWater> {
        self.boundaries.get(chunk_pos)
    }

    /// Gets mutable boundary data for a chunk, creating if needed.
    pub fn get_mut(&mut self, chunk_pos: IVec3) -> &mut ChunkBoundaryWater {
        self.boundaries
            .entry(chunk_pos)
            .or_insert_with(ChunkBoundaryWater::new)
    }

    /// Removes boundary data for a chunk.
    pub fn remove(&mut self, chunk_pos: &IVec3) {
        self.boundaries.remove(chunk_pos);
    }

    /// Marks a chunk's boundaries as dirty (needs neighbor notification).
    pub fn mark_dirty(&mut self, chunk_pos: IVec3) {
        self.dirty_chunks.insert(chunk_pos);
    }

    /// Takes all dirty chunks for processing.
    pub fn take_dirty(&mut self) -> HashSet<IVec3> {
        std::mem::take(&mut self.dirty_chunks)
    }
}

/// Converts a local position on a chunk boundary to the 2D face position.
fn local_to_face_pos(local_pos: &IVec3, face: BoundaryFace) -> IVec2 {
    match face {
        BoundaryFace::NegX | BoundaryFace::PosX => IVec2::new(local_pos.y, local_pos.z),
        BoundaryFace::NegY | BoundaryFace::PosY => IVec2::new(local_pos.x, local_pos.z),
        BoundaryFace::NegZ | BoundaryFace::PosZ => IVec2::new(local_pos.x, local_pos.y),
    }
}

/// Checks if a local position is on a specific boundary face.
fn is_on_boundary(local_pos: &IVec3, face: BoundaryFace) -> bool {
    match face {
        BoundaryFace::NegX => local_pos.x == 0,
        BoundaryFace::PosX => local_pos.x == CHUNK_SIZE - 1,
        BoundaryFace::NegY => local_pos.y == 0,
        BoundaryFace::PosY => local_pos.y == CHUNK_SIZE - 1,
        BoundaryFace::NegZ => local_pos.z == 0,
        BoundaryFace::PosZ => local_pos.z == CHUNK_SIZE - 1,
    }
}

/// System to update boundary cache when chunks are modified.
///
/// This extracts water cells at chunk boundaries and stores them in the cache
/// for efficient cross-chunk lookups during lateral flow simulation.
pub fn update_water_boundaries_system(
    world_map: Res<ServerWorldMap>,
    mut boundary_cache: ResMut<WaterBoundaryCache>,
    mut lateral_flow_queue: ResMut<super::water_flow::LateralFlowQueue>,
) {
    // Process chunks that were modified (marked for update)
    // We look at chunks_to_update as an indicator of recent changes
    let chunks_to_check: Vec<IVec3> = world_map.chunks.chunks_to_update.clone();

    for chunk_pos in chunks_to_check {
        let Some(chunk) = world_map.chunks.map.get(&chunk_pos) else {
            continue;
        };

        // Extract boundary water data from this chunk
        let boundary_data = extract_chunk_boundaries(chunk, &chunk.water_surfaces);

        // Check if boundaries changed
        let old_data = boundary_cache.get(&chunk_pos);
        let boundaries_changed = old_data
            .map(|old| {
                old.generation != boundary_data.generation || boundaries_differ(old, &boundary_data)
            })
            .unwrap_or(true);

        if boundaries_changed {
            // Update cache
            *boundary_cache.get_mut(chunk_pos) = boundary_data;
            boundary_cache.mark_dirty(chunk_pos);
        }
    }

    // Notify neighbors of dirty chunks
    let dirty_chunks = boundary_cache.take_dirty();
    for chunk_pos in dirty_chunks {
        // Queue neighbor chunks for lateral flow if they have water near the boundary
        for face in BoundaryFace::HORIZONTAL {
            let neighbor_pos = chunk_pos + face.neighbor_chunk_offset();

            // Check if neighbor exists and has boundary water
            if world_map.chunks.map.contains_key(&neighbor_pos) {
                // Queue neighbor for lateral flow simulation
                lateral_flow_queue.queue(neighbor_pos);
            }
        }
    }
}

/// Extracts boundary water data from a chunk.
fn extract_chunk_boundaries(
    chunk: &shared::world::ServerChunk,
    surfaces: &shared::world::ChunkWaterSurfaces,
) -> ChunkBoundaryWater {
    let mut boundary = ChunkBoundaryWater::new();

    for (pos, cell) in chunk.water.iter() {
        let volume = cell.volume();
        if volume < MIN_WATER_VOLUME {
            continue;
        }

        let is_surface = surfaces.is_surface(pos);

        // Check all 6 faces
        for face in BoundaryFace::ALL {
            if is_on_boundary(pos, face) {
                let face_pos = local_to_face_pos(pos, face);
                boundary.face_mut(face).set(face_pos, volume, is_surface);
            }
        }
    }

    boundary.generation += 1;
    boundary
}

/// Checks if two boundary data structures differ significantly.
fn boundaries_differ(a: &ChunkBoundaryWater, b: &ChunkBoundaryWater) -> bool {
    if a.has_boundary_water() != b.has_boundary_water() {
        return true;
    }

    for face in BoundaryFace::ALL {
        match (a.face(face), b.face(face)) {
            (None, None) => continue,
            (Some(_), None) | (None, Some(_)) => return true,
            (Some(af), Some(bf)) => {
                if af.cells.len() != bf.cells.len() {
                    return true;
                }
                for (pos, a_cell) in &af.cells {
                    if let Some(b_cell) = bf.cells.get(pos) {
                        if (a_cell.volume - b_cell.volume).abs() > MIN_WATER_VOLUME {
                            return true;
                        }
                    } else {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Result of a cross-chunk flow calculation.
#[derive(Debug, Clone)]
pub struct CrossChunkFlow {
    /// The source chunk position (where water is coming FROM)
    pub source_chunk: IVec3,
    /// Local position in the source chunk
    pub source_local_pos: IVec3,
    /// The neighbor chunk position (where water is going TO)
    pub neighbor_chunk: IVec3,
    /// Local position in the neighbor chunk
    pub neighbor_local_pos: IVec3,
    /// Amount of water to transfer
    pub flow_amount: f32,
}

/// Calculates potential flow from a boundary cell to a neighbor chunk.
///
/// This is called during lateral flow simulation when a cell is at a chunk edge.
///
/// # Arguments
/// * `chunk_pos` - Current chunk position
/// * `local_pos` - Local position of the source water cell
/// * `source_volume` - Water volume at source
/// * `source_height` - Surface height at source
/// * `boundary_cache` - The boundary cache for neighbor lookups
/// * `world_map` - World map for block lookups
///
/// # Returns
/// List of potential cross-chunk flows (may be empty if no valid flows)
pub fn calculate_cross_chunk_flows(
    chunk_pos: IVec3,
    local_pos: IVec3,
    source_volume: f32,
    source_height: f32,
    boundary_cache: &WaterBoundaryCache,
    world_map: &ServerWorldMap,
) -> Vec<CrossChunkFlow> {
    use shared::world::{BlockHitbox, BlockId, FULL_WATER_HEIGHT, MAX_WATER_VOLUME};

    let mut flows = Vec::new();

    // Check each cardinal direction for boundary crossing
    let neighbors = [
        (IVec3::new(1, 0, 0), BoundaryFace::PosX),
        (IVec3::new(-1, 0, 0), BoundaryFace::NegX),
        (IVec3::new(0, 0, 1), BoundaryFace::PosZ),
        (IVec3::new(0, 0, -1), BoundaryFace::NegZ),
    ];

    for (offset, face) in neighbors {
        let neighbor_local = local_pos + offset;

        // Skip if not crossing boundary
        if neighbor_local.x >= 0
            && neighbor_local.x < CHUNK_SIZE
            && neighbor_local.z >= 0
            && neighbor_local.z < CHUNK_SIZE
        {
            continue;
        }

        // Calculate neighbor chunk and local position
        let (neighbor_chunk_offset, neighbor_local_in_chunk) =
            wrap_to_neighbor_chunk(neighbor_local);
        let neighbor_chunk_pos = chunk_pos + neighbor_chunk_offset;

        // Determine which face of the neighbor chunk this position is on
        // (it's the opposite of the face we're crossing from our chunk)
        let neighbor_face = face.opposite();
        debug_assert_eq!(
            neighbor_face.opposite(),
            face,
            "BoundaryFace::opposite() must be bidirectional (face: {:?}, neighbor_face: {:?})",
            face,
            neighbor_face
        );

        // Check if neighbor chunk exists
        let Some(neighbor_chunk) = world_map.chunks.map.get(&neighbor_chunk_pos) else {
            continue;
        };

        // Check if neighbor position is blocked by solid block
        if let Some(block) = neighbor_chunk.map.get(&neighbor_local_in_chunk) {
            if block.id != BlockId::Water
                && matches!(
                    block.id.get_hitbox(),
                    BlockHitbox::FullBlock | BlockHitbox::Aabb(_)
                )
            {
                continue;
            }
        }

        // Pre-compute fallback value to avoid closure allocation
        let fallback_volume = neighbor_chunk.water.volume_at(&neighbor_local_in_chunk);

        // Try to get neighbor water info from boundary cache first, fall back to direct lookup
        let neighbor_volume = boundary_cache
            .get(&neighbor_chunk_pos)
            .and_then(|neighbor_boundary| neighbor_boundary.face(neighbor_face))
            .and_then(|face_data| {
                let face_pos = local_to_face_pos(&neighbor_local_in_chunk, neighbor_face);
                face_data.get(&face_pos).map(|cell| cell.volume)
            })
            .unwrap_or(fallback_volume);

        // Calculate neighbor surface height
        let neighbor_surface_height = if neighbor_volume > MIN_WATER_VOLUME {
            neighbor_local_in_chunk.y as f32 + neighbor_volume * FULL_WATER_HEIGHT
        } else if neighbor_local_in_chunk.y <= local_pos.y {
            neighbor_local_in_chunk.y as f32
        } else {
            continue; // Can't flow upward to empty cell
        };

        // Calculate height difference
        let height_diff = source_height - neighbor_surface_height;

        // Only flow downhill
        if height_diff < super::water_flow::MIN_HEIGHT_DIFF {
            continue;
        }

        // Calculate flow amount
        let mut flow_amount = height_diff * super::water_flow::FLOW_RATE;
        flow_amount *= 1.0 - super::water_flow::FLOW_DAMPING;
        flow_amount = flow_amount.min(source_volume * super::water_flow::MAX_FLOW_PER_TICK);

        // Limit to available space
        let neighbor_space = MAX_WATER_VOLUME - neighbor_volume;
        flow_amount = flow_amount.min(neighbor_space);

        if flow_amount >= MIN_WATER_VOLUME {
            flows.push(CrossChunkFlow {
                source_chunk: chunk_pos,
                source_local_pos: local_pos,
                neighbor_chunk: neighbor_chunk_pos,
                neighbor_local_pos: neighbor_local_in_chunk,
                flow_amount,
            });
        }
    }

    flows
}

/// Wraps a local position that's outside chunk bounds to the neighbor chunk offset
/// and the corresponding local position within that neighbor chunk.
fn wrap_to_neighbor_chunk(local_pos: IVec3) -> (IVec3, IVec3) {
    let mut chunk_offset = IVec3::ZERO;
    let mut wrapped_pos = local_pos;

    if local_pos.x < 0 {
        chunk_offset.x = -1;
        wrapped_pos.x = CHUNK_SIZE - 1;
    } else if local_pos.x >= CHUNK_SIZE {
        chunk_offset.x = 1;
        wrapped_pos.x = 0;
    }

    if local_pos.y < 0 {
        chunk_offset.y = -1;
        wrapped_pos.y = CHUNK_SIZE - 1;
    } else if local_pos.y >= CHUNK_SIZE {
        chunk_offset.y = 1;
        wrapped_pos.y = 0;
    }

    if local_pos.z < 0 {
        chunk_offset.z = -1;
        wrapped_pos.z = CHUNK_SIZE - 1;
    } else if local_pos.z >= CHUNK_SIZE {
        chunk_offset.z = 1;
        wrapped_pos.z = 0;
    }

    (chunk_offset, wrapped_pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boundary_face_properties() {
        assert_eq!(BoundaryFace::NegX.opposite(), BoundaryFace::PosX);
        assert_eq!(BoundaryFace::PosY.opposite(), BoundaryFace::NegY);
        assert_eq!(
            BoundaryFace::NegX.neighbor_chunk_offset(),
            IVec3::new(-1, 0, 0)
        );
    }

    #[test]
    fn test_wrap_to_neighbor_chunk() {
        // Negative X overflow
        let (offset, wrapped) = wrap_to_neighbor_chunk(IVec3::new(-1, 5, 5));
        assert_eq!(offset, IVec3::new(-1, 0, 0));
        assert_eq!(wrapped, IVec3::new(CHUNK_SIZE - 1, 5, 5));

        // Positive Z overflow
        let (offset, wrapped) = wrap_to_neighbor_chunk(IVec3::new(5, 5, CHUNK_SIZE));
        assert_eq!(offset, IVec3::new(0, 0, 1));
        assert_eq!(wrapped, IVec3::new(5, 5, 0));

        // Within bounds - no change
        let (offset, wrapped) = wrap_to_neighbor_chunk(IVec3::new(5, 5, 5));
        assert_eq!(offset, IVec3::ZERO);
        assert_eq!(wrapped, IVec3::new(5, 5, 5));
    }

    #[test]
    fn test_boundary_face_data() {
        let mut face_data = BoundaryFaceData::new();

        // Add water
        face_data.set(IVec2::new(5, 5), 0.5, true);
        assert!(face_data.has_water());

        let cell = face_data.get(&IVec2::new(5, 5));
        assert!(cell.is_some());
        assert_eq!(cell.unwrap().volume, 0.5);
        assert!(cell.unwrap().is_surface);

        // Remove water (volume too low)
        face_data.set(IVec2::new(5, 5), 0.0001, false);
        assert!(!face_data.has_water());
    }

    #[test]
    fn test_is_on_boundary() {
        assert!(is_on_boundary(&IVec3::new(0, 5, 5), BoundaryFace::NegX));
        assert!(!is_on_boundary(&IVec3::new(1, 5, 5), BoundaryFace::NegX));
        assert!(is_on_boundary(
            &IVec3::new(CHUNK_SIZE - 1, 5, 5),
            BoundaryFace::PosX
        ));
    }

    #[test]
    fn test_wrap_corner_cases() {
        // Corner case: position at (0, 0, 0) going negative on all axes
        // This would only happen with vertical flow, which we handle separately
        let (offset, wrapped) = wrap_to_neighbor_chunk(IVec3::new(-1, -1, -1));
        assert_eq!(offset, IVec3::new(-1, -1, -1));
        assert_eq!(
            wrapped,
            IVec3::new(CHUNK_SIZE - 1, CHUNK_SIZE - 1, CHUNK_SIZE - 1)
        );

        // Edge case: exactly at CHUNK_SIZE on multiple axes
        let (offset, wrapped) = wrap_to_neighbor_chunk(IVec3::new(CHUNK_SIZE, 5, CHUNK_SIZE));
        assert_eq!(offset, IVec3::new(1, 0, 1));
        assert_eq!(wrapped, IVec3::new(0, 5, 0));
    }

    #[test]
    fn test_boundary_face_symmetry() {
        // Verify that opposite faces are truly opposite
        for face in BoundaryFace::ALL {
            let opposite = face.opposite();
            assert_eq!(
                face.neighbor_chunk_offset(),
                -opposite.neighbor_chunk_offset(),
                "Face {:?} and its opposite {:?} should have negated offsets",
                face,
                opposite
            );
            assert_eq!(
                face,
                opposite.opposite(),
                "Double opposite should return original face"
            );
        }
    }

    #[test]
    fn test_local_to_face_pos_consistency() {
        // For X faces, face_pos should be (y, z)
        let pos = IVec3::new(0, 7, 11);
        let face_pos = local_to_face_pos(&pos, BoundaryFace::NegX);
        assert_eq!(face_pos, IVec2::new(7, 11));

        let pos = IVec3::new(CHUNK_SIZE - 1, 3, 9);
        let face_pos = local_to_face_pos(&pos, BoundaryFace::PosX);
        assert_eq!(face_pos, IVec2::new(3, 9));

        // For Y faces, face_pos should be (x, z)
        let pos = IVec3::new(5, 0, 8);
        let face_pos = local_to_face_pos(&pos, BoundaryFace::NegY);
        assert_eq!(face_pos, IVec2::new(5, 8));

        // For Z faces, face_pos should be (x, y)
        let pos = IVec3::new(4, 6, CHUNK_SIZE - 1);
        let face_pos = local_to_face_pos(&pos, BoundaryFace::PosZ);
        assert_eq!(face_pos, IVec2::new(4, 6));
    }

    #[test]
    fn test_cross_chunk_flow_volume_limits() {
        use super::*;

        // Test that CrossChunkFlow can represent edge cases
        let flow = CrossChunkFlow {
            source_chunk: IVec3::new(0, 0, 0),
            source_local_pos: IVec3::new(CHUNK_SIZE - 1, 5, 5),
            neighbor_chunk: IVec3::new(1, 0, 0),
            neighbor_local_pos: IVec3::new(0, 5, 5),
            flow_amount: MIN_WATER_VOLUME,
        };

        // Verify the flow crosses the +X boundary correctly
        assert_eq!(
            flow.source_chunk + IVec3::new(1, 0, 0),
            flow.neighbor_chunk
        );
        assert_eq!(flow.source_local_pos.x, CHUNK_SIZE - 1);
        assert_eq!(flow.neighbor_local_pos.x, 0);
    }

    #[test]
    fn test_chunk_boundary_water_generation_tracking() {
        let mut boundary = ChunkBoundaryWater::new();
        let initial_gen = boundary.generation;

        // Adding water should not change generation (only clear does)
        boundary.face_mut(BoundaryFace::PosX).set(IVec2::new(5, 5), 0.5, true);
        assert_eq!(boundary.generation, initial_gen);

        // Clear should increment generation
        boundary.clear();
        assert_eq!(boundary.generation, initial_gen + 1);
        assert!(!boundary.has_boundary_water());
    }
}

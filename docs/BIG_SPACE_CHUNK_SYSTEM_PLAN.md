# Big Space Chunk System Implementation Plan

**Goal**: Replace the existing chunk system with `big_space` crate to enable massive world scale with floating-point precision management.

## Overview

The `big_space` crate provides a floating origin system that uses integer grids to extend Bevy's `Transform` with up to 128 bits of precision. This eliminates floating-point jitter at large distances from the origin.

**Documentation**: https://docs.rs/big_space

| Current System | Big Space |
|----------------|-----------|
| 16×16×16 chunks with `IVec3` positions | `GridCell<i32>` (or larger) + `Transform` |
| Single origin at (0, 0, 0) | Floating origin follows camera |
| Precision loss at large distances | Uniform precision everywhere |
| ~4 billion blocks per axis (i32) | Up to `i128` for observable universe scale |

### Why Big Space?

1. **No floating-point jitter**: Objects far from origin render correctly
2. **Ecosystem compatibility**: Uses standard `Transform`, works with existing Bevy plugins
3. **Spatial hashing built-in**: Fast entity lookups and neighbor queries
4. **Nested grids**: Support for moving reference frames (vehicles, planets)
5. **No drift**: Absolute coordinates, unlike camera-relative solutions

---

## Value Proposition Assessment

### When is Big Space Worth It?

**Big Space is recommended when**:
- Players will regularly explore beyond ~10,000 blocks from spawn
- Visual fidelity at extreme distances matters (e.g., large structures, distant terrain)
- You need absolute coordinates for multiplayer synchronization at scale
- Future features require massive world support (space exploration, procedural universes)

**Big Space may be overkill when**:
- Gameplay is confined to a smaller area (< 50,000 blocks from origin)
- The game already uses workarounds (periodic recentering) that work well enough
- Team is unfamiliar with floating-origin concepts and needs to ship quickly

### Cost-Benefit Analysis

| Factor | Without Big Space | With Big Space |
|--------|-------------------|----------------|
| **Development Time** | 0 hours | 15-20 hours (implementation + integration testing) |
| **Maintenance Burden** | Low (known patterns) | Low-Medium (new abstraction layer) |
| **Code Complexity** | Simple coordinates | Additional coordinate conversions |
| **Runtime Overhead** | None | ~10% slower transform propagation (CPU time in spatial systems) |
| **Max Playable Area** | ~100K blocks (with jitter) | Effectively unlimited |

### Complexity Impact

**Files touched**: ~15-20 files across client, server, and shared
**New concepts introduced**:
- `CellCoord` vs `IVec3` (grid cell coordinates)
- `FloatingOrigin` component on camera
- `Grid` configuration and hierarchy
- Coordinate conversion functions

**Team onboarding**: ~2-4 hours to understand the new coordinate system

### Maintenance Considerations

| Area | Impact |
|------|--------|
| **New features** | Must consider grid-local vs global coordinates |
| **Debugging** | Coordinate space awareness required |
| **Network protocol** | Grid cell + local position vs absolute position |
| **Save/load** | Backward compatibility with existing saves |
| **Third-party plugins** | Most work unchanged (use `Transform`) |

### Break-Even Analysis

The migration investment pays off when:
- **Short-term**: If precision issues are already visible at current play distances
- **Medium-term**: If planned features require exploration beyond 50K blocks
- **Long-term**: If world generation or multiplayer scales require absolute positioning

**Recommendation**: For Rustcraft's current scope (typical voxel gameplay), this migration is **forward-looking infrastructure**. It provides headroom for future expansion but isn't immediately necessary if players stay within ~50K blocks of spawn.

### Alternative Approaches

| Alternative | Effort | Tradeoffs |
|-------------|--------|-----------|
| **Do nothing** | 0 hours | Jitter at large distances |
| **Periodic recentering** | 5-10 hours | Simple but causes discontinuities |
| **Camera-relative rendering** | 10-15 hours | Complex, affects all systems |
| **Big Space (this plan)** | 15-20 hours | Clean solution, ecosystem compatible |
| **chunky-bevy** | 8-12 hours | Simpler chunk management, no precision fix |

**Verdict**: Big Space is the cleanest long-term solution if massive world support is a goal. For a quick fix, periodic recentering is simpler but less robust.

### Big Space vs chunky-bevy Comparison

Both crates solve different problems in voxel game development:

| Aspect | big_space | chunky-bevy |
|--------|-----------|-------------|
| **Primary Purpose** | Floating-point precision at large distances | Chunk lifecycle management (load/unload/save) |
| **Problem Solved** | Eliminates jitter far from origin | Simplifies chunk spawning and streaming |
| **Bevy Compatibility** | Bevy 0.16 (crate v0.10) | Bevy 0.17 only (crate v0.2) |
| **Approach** | Grid cells + floating origin | ChunkLoader/ChunkPos components |
| **Precision Handling** | ✅ Up to 128-bit integer grids | ❌ Uses standard `IVec3` |
| **Chunk Loading** | Manual (bring your own logic) | ✅ Built-in `ChunkLoader` component |
| **Chunk Unloading** | Manual | ✅ Built-in strategies (distance, limit, hybrid) |
| **Persistence** | Manual | ✅ Built-in save/load with auto-save option |
| **Spatial Hashing** | ✅ Built-in `CellLookup` | ✅ HashMap-based O(1) lookup |
| **Nested Grids** | ✅ For moving reference frames | ❌ Single flat grid |
| **Debug Visualization** | ❌ Not included | ✅ Chunk boundary visualizer |
| **Dependencies** | No added deps | serde, postcard (for persistence) |
| **Maturity** | Established (326 stars) | Newer (10 stars, created Nov 2025) |

#### When to Choose Each

**Choose big_space when:**
- Players explore massive distances (>50K blocks from spawn)
- Floating-point precision is causing visible artifacts
- You need nested grids (e.g., moving vehicles, planets)
- Bevy 0.16 compatibility is required

**Choose chunky-bevy when:**
- You need turnkey chunk loading/unloading logic
- World persistence with auto-save is a priority
- Gameplay stays within moderate distances (<50K blocks)
- You can upgrade to Bevy 0.17

**Choose both when:**
- You want big_space's precision handling AND chunky-bevy's lifecycle management
- Note: This would require adapting chunky-bevy to use big_space's `CellCoord` instead of `IVec3`

#### For Rustcraft

| Factor | big_space | chunky-bevy |
|--------|-----------|-------------|
| **Solves current pain point?** | ✅ Precision at large distances | ❌ Doesn't address precision |
| **Bevy 0.16 compatible?** | ✅ Yes (v0.10) | ❌ No (requires 0.17) |
| **Replaces existing chunk system?** | Partially | Yes |
| **Effort to integrate** | 15-20 hours | 8-12 hours (after Bevy upgrade) |

**Recommendation for Rustcraft**: 
- **big_space** is the better choice because:
  1. It directly solves the floating-point precision problem
  2. It's compatible with current Bevy 0.16
  3. Rustcraft already has working chunk loading/unloading logic
  
- **chunky-bevy** would be useful if:
  1. Rustcraft upgrades to Bevy 0.17
  2. The goal is to simplify chunk management code (not fix precision)
  3. Built-in persistence is desired over the current RON-based save system

---

## Version Compatibility

| Bevy | big_space |
|------|-----------|
| 0.16 | **0.10** (current) |
| 0.15 | 0.8, 0.9 |
| 0.14 | 0.7 |

**Rustcraft uses Bevy 0.16**, so we need `big_space = "0.10"`.

---

## Implementation Phases

| Phase | Description | Effort | Prerequisite |
|-------|-------------|--------|--------------|
| **1** | Add dependency and core types | Low | — |
| **2** | Migrate chunk coordinate system | Medium | Phase 1 |
| **3** | Client rendering integration | High | Phase 2 |
| **4** | Server-side migration | High | Phase 2 |
| **5** | Network protocol updates | Medium | Phases 3 & 4 |
| **6** | Physics and collision | Medium | Phase 3 |
| **7** | Save/load system updates | Low | Phase 4 |

**MVP**: Phases 1–5. Phases 6–7 add completeness.

---

## Phase 1: Add Dependency and Core Types

> **Effort**: Low (~30 min)

Add `big_space` to the project and define precision type aliases.

### 1.1 Add Dependency

**File**: `shared/Cargo.toml`

```toml
[dependencies]
big_space = "0.10"
```

**File**: `client/Cargo.toml`

```toml
[dependencies]
big_space = "0.10"
```

### 1.2 Choose Grid Precision

Create precision type aliases for consistency across the codebase.

**File**: `shared/src/world/big_space_types.rs` (new)

```rust
use big_space::prelude::*;

// Note: GridPrecision is determined by big_space feature flags.
// Default is i64. To use i32, add feature = "i32" to big_space dependency.
// See: https://docs.rs/big_space/latest/big_space/precision/index.html

/// Grid cell size in world units (meters).
/// This matches our existing CHUNK_SIZE for consistency.
pub const GRID_CELL_SIZE: f32 = 16.0;

/// Grid switching threshold - how far entities can move before recentering.
/// Higher values reduce frequency of cell transitions but decrease precision.
pub const GRID_SWITCHING_THRESHOLD: f32 = 100.0;

/// Create a Grid configured for Rustcraft's chunk system.
pub fn create_chunk_grid() -> Grid {
    Grid::new(GRID_CELL_SIZE, GRID_SWITCHING_THRESHOLD)
}
```

**Note on GridPrecision**: The `big_space` crate uses feature flags to control grid precision:
- Default: `i64` (~19.5 million light years at 10km cell size)
- `i32`: ~0.0045 light years (4× solar system)
- `i128`: Observable universe scale

For Rustcraft with 16m cells (CHUNK_SIZE), `i64` provides approximately:
- 1.47 × 10^14 km total range
- More than sufficient for any practical voxel world

### 1.3 Export Types

**File**: `shared/src/world/mod.rs`

```rust
pub mod big_space_types;
pub use big_space_types::*;
```

**Note**: `CellCoord` from big_space has `.x`, `.y`, `.z` fields of type `GridPrecision` (default `i64`). It implements `Add`, `Sub`, `Hash`, and can be constructed with `CellCoord::new(x, y, z)`.

---

## Phase 2: Migrate Chunk Coordinate System

> **Effort**: Medium (~2 hours) | **Prerequisite**: Phase 1

Replace `IVec3` chunk positions with `big_space` grid cells.

### 2.1 Understanding the Current System

**Current chunk data structures**:

```rust
// shared/src/world/data.rs
pub struct ServerChunk {
    pub map: HashMap<IVec3, BlockData>,  // Local block positions
    pub ts: u64,
    pub sent_to_clients: HashSet<PlayerId>,
}

pub struct ServerChunkWorldMap {
    pub map: HashMap<IVec3, ServerChunk>,  // Chunk positions -> chunks
    pub chunks_to_update: Vec<IVec3>,
    pub generation_requests: HashMap<IVec3, Vec<FloraRequest>>,
}
```

### 2.2 New Chunk Data Structures

**File**: `shared/src/world/data.rs` (modified)

```rust
use crate::world::big_space_types::*;
use big_space::prelude::*;

/// Server chunk with big_space integration.
/// The chunk position is now implicit via the entity's GridCell component.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct ServerChunk {
    pub map: HashMap<IVec3, BlockData>,  // Local block positions (unchanged)
    pub ts: u64,
    pub sent_to_clients: HashSet<PlayerId>,
}

/// World map using big_space grid cells as keys.
/// CellCoord implements Hash, so it can be used directly in HashMap.
/// We keep IVec3 keys for backward compatibility and ease of serialization.
#[derive(Resource, Default, Clone, Serialize, Deserialize, Debug)]
pub struct ServerChunkWorldMap {
    // IVec3 keys for HashMap provide easy serialization
    // Convert to/from CellCoord when interfacing with big_space
    pub map: HashMap<IVec3, ServerChunk>,
    pub chunks_to_update: Vec<IVec3>,
    pub generation_requests: HashMap<IVec3, Vec<FloraRequest>>,
}
```

### 2.3 Coordinate Conversion Utilities

**File**: `shared/src/world/utils.rs` (additions)

```rust
use big_space::prelude::*;
use crate::world::big_space_types::*;

/// Convert a CellCoord to IVec3 for storage/hashing.
/// Warning: This truncates values if GridPrecision (i64) exceeds i32 range.
/// For most voxel games, chunk coordinates stay within i32 range.
pub fn cell_coord_to_ivec3(cell: &CellCoord) -> IVec3 {
    debug_assert!(
        cell.x >= i32::MIN as i64 && cell.x <= i32::MAX as i64 &&
        cell.y >= i32::MIN as i64 && cell.y <= i32::MAX as i64 &&
        cell.z >= i32::MIN as i64 && cell.z <= i32::MAX as i64,
        "CellCoord values exceed i32 range"
    );
    IVec3::new(cell.x as i32, cell.y as i32, cell.z as i32)
}

/// Convert IVec3 to CellCoord.
pub fn ivec3_to_cell_coord(pos: &IVec3) -> CellCoord {
    CellCoord::new(
        pos.x as GridPrecision,
        pos.y as GridPrecision,
        pos.z as GridPrecision,
    )
}

/// Convert a world position (Transform) and CellCoord to absolute position.
/// Uses the Grid to calculate precise world coordinates.
pub fn cell_to_absolute_position(grid: &Grid, cell: &CellCoord, local: Vec3) -> DVec3 {
    grid.grid_position_double(cell, &Transform::from_translation(local))
}

/// Convert absolute position to CellCoord and local Transform.
/// Uses the Grid to split into cell index and local offset.
pub fn absolute_to_cell_position(grid: &Grid, pos: DVec3) -> (CellCoord, Vec3) {
    grid.translation_to_grid(pos)
}
```

---

## Phase 3: Client Rendering Integration

> **Effort**: High (~4 hours) | **Prerequisite**: Phase 2

Integrate `big_space` into the client's entity hierarchy and rendering.

### 3.1 Setup BigSpace Root

**File**: `client/src/game.rs` (additions)

```rust
use big_space::prelude::*;
use shared::world::big_space_types::*;

/// System to initialize the big_space root entity.
pub fn setup_big_space(mut commands: Commands) {
    // Create the root BigSpace with our grid configuration
    commands.spawn_big_space(create_chunk_grid(), |root_grid| {
        // The root grid is the parent for all world entities.
        // Entities are added to this grid using root_grid.spawn_spatial()
        // or by manually adding CellCoord component to children.
    });
}
```

**Alternative**: Use the default spawn method if the default Grid configuration is acceptable:

```rust
commands.spawn_big_space_default(|root_grid| {
    // Default Grid: cell_edge_length = 2000, switching_threshold = 100
});
```

### 3.2 Camera as Floating Origin

The camera must be marked as the `FloatingOrigin` so that all `GlobalTransform`s are computed relative to it.

**File**: `client/src/camera/spawn.rs` (additions)

```rust
use big_space::prelude::*;

/// Add FloatingOrigin to camera entity.
fn spawn_camera(mut commands: Commands, /* ... */) {
    commands.spawn((
        Camera3d::default(),
        Transform::default(),
        FloatingOrigin,  // Mark camera as the origin for precision
        // ... other components
    ));
}
```

### 3.3 Chunk Entity Structure

Each chunk becomes a child entity within the BigSpace grid.

**File**: `client/src/world/rendering/render.rs` (modifications)

```rust
use big_space::prelude::*;

/// Spawn a chunk entity with BigSpace components.
fn spawn_chunk_entity(
    commands: &mut Commands,
    chunk_pos: IVec3,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    big_space_root: Entity,
) -> Entity {
    let cell_coord = ivec3_to_cell_coord(&chunk_pos);
    
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::default(),  // Local position within cell (centered)
        cell_coord,  // CellCoord component for big_space
    ))
    .set_parent(big_space_root)  // Parent to BigSpace root
    .id()
}
```

### 3.4 Block-to-World Coordinate Conversion

Update raycasting and block placement to account for floating origin.

**File**: `shared/src/world/raycast.rs` (modifications)

```rust
use big_space::prelude::*;

/// Convert a block's cell coordinate + local position to world-relative coordinates.
/// The result is relative to the floating origin for rendering/interaction.
pub fn block_to_origin_relative(
    block_cell: &CellCoord,
    local_block_pos: IVec3,
    origin_cell: &CellCoord,
    grid: &Grid,
) -> Vec3 {
    // Calculate cell offset from origin
    let cell_offset = *block_cell - *origin_cell;
    
    // Convert to world coordinates using the grid
    let cell_world_offset = grid.cell_to_float(&cell_offset);
    cell_world_offset.as_vec3() + local_block_pos.as_vec3()
}
```

---

## Phase 4: Server-Side Migration

> **Effort**: High (~4 hours) | **Prerequisite**: Phase 2

Migrate server world management to use `big_space` concepts (without rendering).

### 4.1 Server Does Not Need BigSpace Plugin

The server doesn't render, so it doesn't need the full `big_space` plugin. It only needs to track `CellCoord` positions for entities and chunks.

**File**: `server/Cargo.toml` (addition)

```toml
[dependencies]
# Server only needs big_space types, not the full Bevy plugin.
# The crate is no_std compatible and has no added dependencies.
big_space = "0.10"
```

**Note**: The `big_space` crate has no external dependencies and is no_std compatible. The server uses it only for the `CellCoord`, `Grid`, and `GridPrecision` types, not for any systems or plugins.

### 4.2 Player Position Tracking

Players now have a `CellCoord` component in addition to `Transform`.

**File**: `shared/src/players/data.rs` (modifications)

```rust
use big_space::prelude::*;
use crate::world::big_space_types::*;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Player {
    // ... existing fields ...
    
    /// High-precision grid cell position.
    /// Combined with `position` (local), gives absolute world position.
    /// Stored as [i64; 3] for serialization compatibility.
    pub grid_cell: [i64; 3],
    
    /// Local position within the grid cell.
    /// This replaces the previous absolute `position` field.
    pub position: Vec3,
}

impl Player {
    /// Get the CellCoord for this player.
    pub fn cell_coord(&self) -> CellCoord {
        CellCoord::new(
            self.grid_cell[0] as GridPrecision,
            self.grid_cell[1] as GridPrecision,
            self.grid_cell[2] as GridPrecision,
        )
    }
    
    /// Set the grid cell from a CellCoord.
    pub fn set_cell_coord(&mut self, cell: &CellCoord) {
        self.grid_cell = [cell.x as i64, cell.y as i64, cell.z as i64];
    }
    
    /// Get absolute position as double precision using the grid.
    pub fn absolute_position(&self, grid: &Grid) -> DVec3 {
        grid.grid_position_double(&self.cell_coord(), &Transform::from_translation(self.position))
    }
    
    /// Set absolute position, computing grid cell and local position.
    pub fn set_absolute_position(&mut self, grid: &Grid, pos: DVec3) {
        let (cell, local) = grid.translation_to_grid(pos);
        self.set_cell_coord(&cell);
        self.position = local;
    }
}
```

### 4.3 Chunk Loading Based on Player Cell

**File**: `server/src/world/background_generation.rs` (modifications)

```rust
use big_space::prelude::*;

/// Determine which chunks need generation based on player positions.
fn get_chunks_to_generate(
    players: &HashMap<PlayerId, Player>,
    render_distance: i32,
) -> Vec<IVec3> {
    let mut needed_chunks = HashSet::new();
    
    for player in players.values() {
        let player_cell = cell_coord_to_ivec3(&player.cell_coord());
        
        // Generate chunks in a radius around the player's cell
        for dx in -render_distance..=render_distance {
            for dy in -2..=2 {  // Vertical range
                for dz in -render_distance..=render_distance {
                    needed_chunks.insert(player_cell + IVec3::new(dx, dy, dz));
                }
            }
        }
    }
    
    needed_chunks.into_iter().collect()
}
```

---

## Phase 5: Network Protocol Updates

> **Effort**: Medium (~2 hours) | **Prerequisite**: Phases 3 & 4

Update network messages to use high-precision coordinates.

### 5.1 Position Serialization Strategy

**Option A**: Send `GridCell` + `Transform` separately (recommended)
- More efficient for nearby entities
- Natural fit with big_space model

**Option B**: Send absolute `DVec3` positions
- Simpler protocol
- Requires conversion on both ends

### 5.2 Message Updates

**File**: `shared/src/messages/player.rs` (modifications)

```rust
use big_space::prelude::*;

/// Player position update with high-precision grid cell.
#[derive(Clone, Serialize, Deserialize)]
pub struct PlayerPositionUpdate {
    pub player_id: PlayerId,
    /// Grid cell of the player (stored as i64 for serialization)
    pub grid_cell: [i64; 3],
    /// Local position within the grid cell
    pub local_position: [f32; 3],
    /// Local rotation
    pub rotation: [f32; 4],  // Quaternion
}

impl PlayerPositionUpdate {
    pub fn cell_coord(&self) -> CellCoord {
        CellCoord::new(
            self.grid_cell[0] as GridPrecision,
            self.grid_cell[1] as GridPrecision,
            self.grid_cell[2] as GridPrecision,
        )
    }
    
    pub fn local_position(&self) -> Vec3 {
        Vec3::from_array(self.local_position)
    }
}
```

### 5.3 Chunk Message Updates

**File**: `shared/src/messages/world.rs` (modifications)

```rust
/// Chunk data message - chunk position is now a grid cell coordinate.
#[derive(Clone, Serialize, Deserialize)]
pub struct ChunkData {
    /// Grid cell position of the chunk (same as chunk coordinate)
    pub cell: [i32; 3],
    /// Block data within the chunk
    pub chunk: ServerChunk,
    /// LOD level for rendering
    pub lod_level: LodLevel,
}
```

---

## Phase 6: Physics and Collision

> **Effort**: Medium (~2 hours) | **Prerequisite**: Phase 3

Update collision detection to work with big_space coordinates.

### 6.1 Collision in Grid-Local Coordinates

Collisions are computed in local coordinates relative to the player's grid cell.

**File**: `shared/src/players/collision.rs` (modifications)

```rust
use big_space::prelude::*;

/// Check collision with blocks, accounting for grid cell boundaries.
pub fn check_collision_at_position(
    world_map: &impl WorldMap,
    player_cell: &CellCoord,
    local_position: Vec3,
    hitbox: &Aabb3d,
) -> bool {
    // Convert hitbox to world coordinates
    let world_min = local_position + hitbox.min;
    let world_max = local_position + hitbox.max;
    
    // Check all potentially colliding blocks
    for x in (world_min.x.floor() as i32)..=(world_max.x.floor() as i32) {
        for y in (world_min.y.floor() as i32)..=(world_max.y.floor() as i32) {
            for z in (world_min.z.floor() as i32)..=(world_max.z.floor() as i32) {
                // Convert local block position to global
                let global_block = local_to_global_block(player_cell, IVec3::new(x, y, z));
                
                if let Some(block) = world_map.get_block_by_coordinates(&global_block) {
                    if block.id.has_collision() {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Convert a local block position to global coordinates.
fn local_to_global_block(cell: &CellCoord, local: IVec3) -> IVec3 {
    IVec3::new(
        (cell.x as i32) * CHUNK_SIZE + local.x,
        (cell.y as i32) * CHUNK_SIZE + local.y,
        (cell.z as i32) * CHUNK_SIZE + local.z,
    )
}
```

---

## Phase 7: Save/Load System Updates

> **Effort**: Low (~1 hour) | **Prerequisite**: Phase 4

Update world persistence to handle high-precision coordinates.

### 7.1 World Save Format

**File**: `server/src/world/save.rs` (modifications)

```rust
/// World save format with big_space support.
#[derive(Serialize, Deserialize)]
pub struct WorldSave {
    pub seed: u32,
    pub name: String,
    /// Chunks stored by grid cell position (as i32 array for compatibility).
    pub chunks: HashMap<[i32; 3], ChunkSave>,
    /// Player spawn point grid cell.
    pub spawn_cell: [i64; 3],
    /// Player spawn point local position within cell.
    pub spawn_local: [f32; 3],
    pub time: u64,
}
```

---

## Configuration Summary

| Constant | Location | Value | Notes |
|----------|----------|-------|-------|
| `GRID_CELL_SIZE` | shared | 16.0 | Matches `CHUNK_SIZE` |
| `GRID_SWITCHING_THRESHOLD` | shared | 100.0 | Entity recenter threshold |
| `GridPrecision` | big_space | `i64` (default) | Set via feature flags |

---

## Migration Checklist

### Phase 1
- [ ] Add `big_space = "0.10"` to shared/Cargo.toml
- [ ] Add `big_space = "0.10"` to client/Cargo.toml
- [ ] Create `shared/src/world/big_space_types.rs`
- [ ] Add big_space plugin to client App (see [big_space plugin docs](https://docs.rs/big_space/latest/big_space/plugin/))

### Phase 2
- [ ] Add coordinate conversion utilities (`cell_coord_to_ivec3`, `ivec3_to_cell_coord`)
- [ ] Update `block_to_chunk_coord` to work with `CellCoord`
- [ ] Ensure backward compatibility with existing `IVec3` usage

### Phase 3
- [ ] Create BigSpace root entity in client setup
- [ ] Add `FloatingOrigin` to camera
- [ ] Update chunk entity spawning to use `CellCoord`
- [ ] Update raycast to account for floating origin

### Phase 4
- [ ] Add `big_space` dependency to server
- [ ] Update `Player` struct with `grid_cell` field
- [ ] Update chunk generation to use cell coordinates

### Phase 5
- [ ] Update `PlayerPositionUpdate` message
- [ ] Update `ChunkData` message
- [ ] Test network synchronization with large coordinates

### Phase 6
- [ ] Update collision detection for grid-local coordinates
- [ ] Handle cross-cell collision detection
- [ ] Test player movement near cell boundaries

### Phase 7
- [ ] Update `WorldSave` format
- [ ] Add migration for existing save files
- [ ] Test save/load with high-precision positions

---

## Testing Strategy

### Unit Tests
- [ ] Grid cell to IVec3 conversion roundtrip
- [ ] Absolute position to grid position conversion
- [ ] Cross-cell coordinate calculations

### Integration Tests
- [ ] Player can move across grid cell boundaries smoothly
- [ ] Chunks render correctly at large distances from origin
- [ ] No jitter when player is far from (0, 0, 0)
- [ ] Network sync works with high-precision coordinates
- [ ] Save/load preserves high-precision positions

### Visual Tests
- [ ] Teleport player to coordinates > 10,000 blocks from origin
- [ ] Verify no visible jitter or precision issues
- [ ] Verify chunk loading/unloading works correctly

---

## Known Risks & Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| Breaking changes to network protocol | High | Version message format, add migration path |
| Existing save files incompatible | Medium | Automatic migration on load |
| Performance overhead | Low | Grid calculations are inexpensive |
| Plugin compatibility | Low | big_space uses standard `Transform` |
| Learning curve | Low | Document conventions, add examples |

---

## Performance Expectations

| Metric | Current | With Big Space |
|--------|---------|----------------|
| Memory per entity | Baseline | +24 bytes (`CellCoord` with i64) |
| Transform propagation | Baseline | ~10% slower (grid math) |
| Precision at 1M blocks | Poor (jitter) | Perfect |
| Max world size | ~2B blocks | ~10^18 blocks (i64 default) |

The overhead is negligible for voxel games. The main cost is the additional `GlobalTransform` recalculation when entities cross cell boundaries.

---

## Future Enhancements

| Enhancement | Effort | Impact |
|-------------|--------|--------|
| Nested grids for vehicles | Medium | High |
| Multi-origin split-screen | High | Medium |
| `i64` precision upgrade | Low | Medium |
| Spatial hashing for entities | Medium | High |

---

## Resources

- **Big Space Documentation**: https://docs.rs/big_space
- **Big Space GitHub**: https://github.com/aevyrie/big_space
- **Big Space Examples**: https://github.com/aevyrie/big_space/tree/main/examples
- **Bevy 0.16 Compatibility**: big_space 0.10

---

## Summary

Migrating to `big_space` provides Rustcraft with:
1. **Precision**: No jitter at any world scale
2. **Scale**: Support for virtually unlimited world sizes
3. **Compatibility**: Works with existing Bevy ecosystem
4. **Simplicity**: Most code continues to use `Transform` normally

The migration is incremental—each phase can be completed and tested independently. The critical path is Phases 1-5 for a functional MVP.

**Estimated Total Effort**: 15-20 hours across all phases.

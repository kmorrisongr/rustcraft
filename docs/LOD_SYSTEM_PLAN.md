# Level of Detail (LOD) System Implementation Plan

**Goal**: Reduce mesh complexity and bandwidth by rendering distant chunks at lower resolutions.

## Overview

The LOD system extends the render distance by allowing chunks beyond the normal render distance to be rendered at reduced detail. This enables players to see further while maintaining performance.

| LOD Level | Distance Range | Block Scale | Mesh Reduction |
|-----------|----------------|-------------|----------------|
| LOD 0 | 0 to 1× RD | 1:1 | None (full detail) |
| LOD 1 | 1× to 1.5× RD | 2:1 | ~87.5% fewer voxels |

*RD = Render Distance*

### Block Scale Explanation

- **LOD 0 (1:1)**: Each block in the chunk maps to one rendered block
- **LOD 1 (2:1)**: Each 2×2×2 group of blocks is represented by a single larger block

---

## Implementation Phases

This plan is organized into independent implementation phases. Each phase provides incremental value and can be merged separately. Complete phases in order for best results, though Phase 1 has no user-visible impact on its own.

| Phase | Description | Effort | Impact | Prerequisite |
|-------|-------------|--------|--------|--------------|
| **1** | LOD Infrastructure | Low | None (foundation) | — |
| **2** | LOD Meshing | Medium | High | Phase 1 |
| **3** | Render Integration | Medium | High | Phase 2 |
| **4** | LOD Transitions | Low | Medium | Phase 3 |
| **5** | Server Broadcast | Low | High | Phase 3 |

**Recommended MVP**: Phases 1–3 + 5 provide a working LOD system. Phase 4 adds polish.

---

## Architecture Design

### Key Principle: Client-Side LOD Meshing

The LOD system is implemented **entirely client-side** in the meshing pipeline. The server sends full-resolution chunk data, and the client generates appropriate meshes based on distance.

**Rationale**:
- Server already sends full chunk data efficiently
- Client can dynamically adjust LOD based on local render distance settings
- No protocol changes required
- Chunks can transition between LOD levels without re-fetching data

---

## Phase 1: LOD Infrastructure
> **Effort**: Low (~20 min) | **Impact**: None (foundation only) | **Prerequisite**: None

This phase adds the data structures and helper methods needed by later phases. No user-visible changes, but these additions are safe to merge independently.

### Existing Code to Leverage

| Component | Location | Reuse |
|-----------|----------|-------|
| `RenderDistance` resource | [render_distance.rs](../client/src/world/rendering/render_distance.rs) | Add helper methods |
| `ClientChunk` struct | [data.rs](../client/src/world/data.rs) | Add `current_lod` field |

### 1.1 Define LOD Level Enum

**File**: `shared/src/world/mod.rs`

```rust
/// Level of Detail for chunk rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LodLevel {
    /// Full detail: 1 block = 1 rendered block
    #[default]
    Lod0,
    /// Reduced detail: 2×2×2 blocks = 1 rendered block (8× reduction)
    Lod1,
}

impl LodLevel {
    /// Returns the block scale factor for this LOD level
    /// LOD 0 = 1, LOD 1 = 2 (each rendered block represents 2×2×2 source blocks)
    pub fn block_scale(&self) -> i32 {
        match self {
            LodLevel::Lod0 => 1,
            LodLevel::Lod1 => 2,
        }
    }
    
    /// Calculate LOD level from squared chunk distance to player.
    /// 
    /// All parameters are **squared** distances (avoids sqrt for performance).
    /// - `chunk_distance_sq`: squared distance from player chunk to target chunk
    /// - `lod0_distance_sq`: squared threshold for LOD 0 (full detail)
    /// - `lod1_distance_sq`: squared threshold for LOD 1 (reduced detail)
    /// 
    /// Chunks beyond `lod1_distance_sq` return LOD 1 but should be culled by caller.
    pub fn from_distance_squared(chunk_distance_sq: i32, lod0_distance_sq: i32, lod1_distance_sq: i32) -> Self {
        if chunk_distance_sq <= lod0_distance_sq {
            LodLevel::Lod0
        } else if chunk_distance_sq <= lod1_distance_sq {
            LodLevel::Lod1
        } else {
            // Caller should unload/cull beyond lod1 distance
            LodLevel::Lod1
        }
    }
}

#### 1.2 Extend Render Distance Configuration

**File**: `client/src/world/rendering/render_distance.rs`

Add these methods to the existing `RenderDistance` struct and a constant:

```rust
/// Multiplier for LOD 1 range (1.0 to 1.5× render distance)
pub const LOD1_DISTANCE_MULTIPLIER: f32 = 1.5;

impl RenderDistance {
    pub fn lod0_distance(&self) -> i32 { self.distance as i32 }
    pub fn lod0_distance_sq(&self) -> i32 { self.lod0_distance().pow(2) }
    pub fn lod1_distance(&self) -> i32 { (self.distance as f32 * LOD1_DISTANCE_MULTIPLIER) as i32 }
    pub fn lod1_distance_sq(&self) -> i32 { self.lod1_distance().pow(2) }
    pub fn total_distance(&self) -> i32 { self.lod1_distance() }
}
```

#### 1.3 Track LOD Level in Client Chunks

**File**: `client/src/world/data.rs`

```rust
use shared::world::LodLevel;

#[derive(Clone, Debug)]
pub struct ClientChunk {
    pub map: HashMap<IVec3, BlockData>,
    pub entity: Option<Entity>,
    pub last_mesh_ts: Instant,
    pub current_lod: LodLevel,  // Add this field
}
```

---

## Phase 2: LOD Meshing
> **Effort**: Medium (~45 min) | **Impact**: High | **Prerequisite**: Phase 1

This phase implements the core LOD mesh generation. After this phase, the codebase *can* generate LOD meshes, but they won't be used until Phase 3 wires them into the render system.

### Existing Code to Leverage

| Component | Location | Reuse Strategy |
|-----------|----------|----------------|
| `generate_chunk_mesh()` | [meshing.rs](../client/src/world/rendering/meshing.rs) | Mirror structure, delegate for LOD 0 |
| `MeshCreator`, `build_mesh()` | [meshing.rs](../client/src/world/rendering/meshing.rs) | Use unchanged |
| `is_block_surrounded()` | [meshing.rs](../client/src/world/rendering/meshing.rs) | Create scaled variant |
| `should_render_face()` | [meshing.rs](../client/src/world/rendering/meshing.rs) | Create scaled variant |
| `render_face()` | [meshing.rs](../client/src/world/rendering/meshing.rs) | Add `scale` parameter |

### 2.1 LOD Mesh Generation Function

**Core insight**: LOD meshing samples blocks at intervals. For LOD 1 (scale=2), sample every 2nd block in each dimension, then render each sampled block at 2× size.

**File**: `client/src/world/rendering/meshing.rs`

```rust
/// Generates a mesh for a chunk at the specified LOD level.
/// For LOD 0, delegates to generate_chunk_mesh(). For LOD 1+, samples blocks at intervals.
pub(crate) fn generate_chunk_mesh_lod(
    world_map: &ClientWorldMap,
    chunk: &ClientChunk,
    chunk_pos: &IVec3,
    uv_map: &HashMap<String, UvCoords>,
    lod_level: LodLevel,
) -> ChunkMeshResponse {
    if lod_level == LodLevel::Lod0 {
        return generate_chunk_mesh(world_map, chunk, chunk_pos, uv_map);
    }
    
    let scale = lod_level.block_scale();
    let mut solid_mesh_creator = MeshCreator::default();
    
    // Sample blocks at LOD intervals (for scale=2: 0, 2, 4, 6, 8, 10, 12, 14)
    for lod_x in 0..(CHUNK_SIZE / scale) {
        for lod_y in 0..(CHUNK_SIZE / scale) {
            for lod_z in 0..(CHUNK_SIZE / scale) {
                let local_block_pos = IVec3::new(lod_x * scale, lod_y * scale, lod_z * scale);
                
                let block = match chunk.map.get(&local_block_pos) {
                    Some(b) => b,
                    None => continue,
                };
                
                let global_block_pos = to_global_pos(chunk_pos, &local_block_pos);
                
                // Same logic as generate_chunk_mesh, but:
                // 1. Use is_lod_block_surrounded() with scaled neighbor checks
                // 2. Use should_render_lod_face() with scaled offsets
                // 3. Use render_face_scaled() to scale vertices by `scale`
                
                // ... (follows same pattern as generate_chunk_mesh)
            }
        }
    }
    
    // Skip tangent generation for LOD meshes (no normal mapping benefit at distance)
    ChunkMeshResponse { solid_mesh: build_mesh(&solid_mesh_creator) }
}
```

### 2.2 Helper Function Changes

The LOD variants are nearly identical to existing functions, with `scale` parameter added:

| Existing Function | LOD Variant | Key Change |
|-------------------|-------------|------------|
| `is_block_surrounded()` | `is_lod_block_surrounded(..., scale)` | Multiply neighbor offsets by `scale` |
| `should_render_face()` | `should_render_lod_face(..., scale)` | Multiply direction offset by `scale` |
| `render_face()` | `render_face_scaled(..., scale)` | Multiply vertex positions by `scale` |

**Example**: `render_face_scaled` differs only in vertex scaling:

```rust
// Original: local_vertices.extend(face.vertices.iter());
// Scaled:
local_vertices.extend(face.vertices.iter().map(|v| {
    [v[0] * scale, v[1] * scale, v[2] * scale]
}));
```

---

## Phase 3: Render System Integration
> **Effort**: Medium (~30 min) | **Impact**: High | **Prerequisite**: Phase 2

This phase wires LOD meshing into the render pipeline.

### 3.1 Update MeshingTask and Render System

**File**: `client/src/world/rendering/render.rs`


```rust
// Add lod_level field to MeshingTask:
pub struct MeshingTask {
    pub chunk_pos: IVec3,
    pub mesh_request_ts: Instant,
    pub thread: Task<ChunkMeshResponse>,
    pub lod_level: LodLevel,  // NEW
}

// In world_render_system, when spawning mesh tasks:
let chunk_distance_sq = pos.distance_squared(player_chunk_pos);
let lod_level = LodLevel::from_distance_squared(
    chunk_distance_sq,
    render_distance.lod0_distance_sq(),
    render_distance.lod1_distance_sq(),
);

let t = pool.spawn(async move {
    world::meshing::generate_chunk_mesh_lod(&map_clone, &ch, &pos, &uvs_clone, lod_level)
});

// After mesh is applied, update chunk's LOD tracking to prevent thrashing:
chunk.current_lod = task.lod_level;
```

---

## Phase 4: LOD Transition System
> **Effort**: Low (~20 min) | **Impact**: Medium | **Prerequisite**: Phase 3

Adds automatic LOD transitions as the player moves. Without this, chunks only get their LOD level set on initial load.

**New file**: `client/src/world/rendering/lod_transitions.rs`

```rust
const LOD_CHECK_INTERVAL: f32 = 0.5;

/// Periodically checks all chunks and triggers remesh when LOD level should change
pub fn lod_transition_system(
    time: Res<Time>,
    mut timer: ResMut<LodCheckTimer>,
    render_distance: Res<RenderDistance>,
    world_map: Res<ClientWorldMap>,
    player_query: Query<&Transform, With<CurrentPlayerMarker>>,
    mut render_events: EventWriter<WorldRenderRequestUpdateEvent>,
) {
    timer.timer += time.delta_secs();
    if timer.timer < LOD_CHECK_INTERVAL { return; }
    timer.timer = 0.0;
    
    let player_chunk = /* get player chunk position */;
    
    for (chunk_pos, chunk) in world_map.map.iter() {
        let expected_lod = LodLevel::from_distance_squared(
            chunk_pos.distance_squared(player_chunk),
            render_distance.lod0_distance_sq(),
            render_distance.lod1_distance_sq(),
        );
        
        if expected_lod != chunk.current_lod {
            render_events.send(WorldRenderRequestUpdateEvent::ChunkToReload(*chunk_pos));
        }
    }
}
```

---

## Phase 5: Server Broadcast Distance
> **Effort**: Low (~5 min) | **Impact**: High | **Prerequisite**: Phase 3

Without this phase, the server only sends chunks within the original render distance—LOD 1 zones will be empty.

**File**: `server/src/world/broadcast_world.rs`

```rust
const SERVER_LOD1_MULTIPLIER: f32 = 1.5;  // Must match client

let effective_render_distance = (config.broadcast_render_distance as f32 * SERVER_LOD1_MULTIPLIER) as i32;
```

**Bandwidth note**: 1.5× radius = ~3.4× chunk volume. Consider throttling LOD 1 chunk sends or adding a server config flag.

---

## Configuration

| Constant | Value | Description |
|----------|-------|-------------|
| `LOD1_DISTANCE_MULTIPLIER` | 1.5 | LOD 1 extends from 1× to 1.5× render distance |
| `LOD_CHECK_INTERVAL` | 0.5s | How often to check for LOD transitions (Phase 4) |
| `SERVER_LOD1_MULTIPLIER` | 1.5 | Server-side; must match client |

**Tuning**: Lower multiplier (1.25) = conservative; higher (2.0) = see further but more bandwidth/memory.

---

## Testing Checklist

- [ ] LOD 0 chunks render at full detail within render distance
- [ ] LOD 1 chunks render at 2× block scale beyond render distance  
- [ ] Block selection/interaction only works on LOD 0 chunks
- [ ] No visual artifacts at LOD boundaries
- [ ] LOD transitions complete within 500ms
- [ ] No mesh thrashing when player is stationary
- [ ] Teleporting long distances handles LOD correctly

---

## Known Risks

| Risk | Mitigation |
|------|------------|
| Texture stretching on high-contrast blocks | Acceptable at distance; can tile UVs if needed |
| LOD boundary seams | Only visible at chunk edges; consider crossfade in future |
| Block interactions on LOD 1 chunks | Ensure raycast respects LOD 0 boundary |
| Server bandwidth increase (~3.4×) | Throttle LOD 1 sends; add server config flag |

---

## Performance Expectations

| Metric | LOD 0 Only | With LOD 1 |
|--------|------------|------------|
| Visible Range | 1× RD | 1.5× RD |
| Chunk Count | ~πr³ | ~3.4× more chunks |
| Mesh Complexity | Baseline | ~70% (LOD 1 has 87.5% fewer voxels) |
| Expected FPS | Baseline | +10-20% (net gain from reduced detail) |

---

## Future Enhancements

| Enhancement | Effort | Impact |
|-------------|--------|--------|
| **LOD 2** (4:1 scale, 2–3× RD) | Medium | High |
| **Terrain-Only LOD** (skip flora) | Low | Medium |
| **Negotiate LOD in Auth** | Low | Medium |
| **Greedy Meshing for LOD** | High | Medium |
| **Smooth Transitions** (crossfade) | High | Low |

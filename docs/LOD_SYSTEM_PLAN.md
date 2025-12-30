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

This phase adds the data structures and helper methods needed by later phases. No user-visible changes, but these additions are safe to merge independently. Deliverables in this phase: `LodLevel` enum, render-distance helpers (lod0/lod1 thresholds), and `current_lod` on `ClientChunk`. While here, fix the two pre-existing correctness/perf issues (Arc cloning and distance typing) and add a simple memory readout so later phases have observability.

### Prerequisites / Standalone Fixes

Before or during this phase, address these foundation issues:

#### Gotcha: Arc Clone Performance in Render Loop

**Standalone PR**: Yes — Pre-existing issue; can be fixed before LOD implementation begins.

**Location**: `world_render_system` in [render.rs](../client/src/world/rendering/render.rs#L97-L101)

**Issue**: The current code clones the entire `ClientWorldMap` on every render pass. With LOD extending the active chunk count by ~3.4×, this clone becomes significantly more expensive.

```rust
// Current code - clones entire map every frame with pending events
let map_ptr = Arc::new(world_map.clone());
```

**Severity**: High — Could negate all LOD performance gains

**Mitigation (pick one)**
- Store `Arc<ClientWorldMap>` as the resource type, or
- Clone only on dirty map (dirty flag), or
- Batch LOD remesh requests to reduce clone frequency

**Verification**: `world_render_system` clone time < 1ms with LOD enabled

---

#### Gotcha: `distance_squared` Type Mismatch

**Standalone PR**: Yes — Pre-existing type inconsistency; can be fixed before LOD implementation.

**Issue**: The existing codebase uses `IVec3::distance_squared()` which returns `i32`, but distance thresholds from render distance are computed from `u32`. Mixing signed/unsigned may cause issues at extreme coordinates.

**Severity**: Low — Only affects edge cases at extreme coordinates

**Mitigation**
- Use `i32` consistently for positions/distances
- Add explicit casts with overflow checks in debug builds

---

#### Gotcha: Memory Pressure from Extended Chunk Cache

**Standalone PR**: Partial — Adding memory usage to F3 debug overlay can be done independently.

**Issue**: 3.4× more chunks in memory means 3.4× more `HashMap<IVec3, BlockData>` entries. Each chunk with 16³ blocks at ~40 bytes per `BlockData` = ~160KB per chunk. Memory usage could grow from ~500MB to ~1.7GB.

**Severity**: Medium — May make game unplayable on some systems

**Mitigation**
- Consider a compact format for LOD 1 chunks
- Add a config flag to disable LOD on low-memory systems
- Unload chunks beyond `lod1_distance`
- Add F3 memory readout early

**Verification**: Check memory usage with `top`/Activity Monitor at max render distance

---

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

This phase implements the core LOD mesh generation. After this phase, the codebase can generate LOD meshes, but they are not yet plugged into rendering. Scope: mirror `generate_chunk_mesh` with a `scale` parameter, add scaled variants of face-culling helpers, and enforce chunk-size divisibility plus boundary safety when sampling.

### Gotchas to Address in This Phase

#### Gotcha: LOD Block Sampling Alignment

**Issue**: When sampling blocks at `(lod_x * scale, lod_y * scale, lod_z * scale)`, if `CHUNK_SIZE` is not evenly divisible by `scale`, the sampling will miss edge blocks.

```rust
// If CHUNK_SIZE=16, scale=2: samples 0,2,4,6,8,10,12,14 ✓
// If CHUNK_SIZE=17, scale=2: samples 0,2,4,6,8,10,12,14,16 — misses block 16
for lod_x in 0..(CHUNK_SIZE / scale) { ... }
```

**Severity**: High — Will cause visible rendering artifacts (missing blocks at chunk edges, visible gaps)

**Mitigation**
- Assert `CHUNK_SIZE % scale == 0` at compile time
- Current `CHUNK_SIZE=16` is safe for scale=2, but document this constraint
- Add a static assertion:
  ```rust
  const_assert!(CHUNK_SIZE % 2 == 0, "CHUNK_SIZE must be divisible by LOD scales");
  ```

**Verification**: Verify no gaps at chunk edges in LOD 1 (fly along chunk boundaries)

---

#### Gotcha: Cross-Chunk Face Culling at LOD Boundaries

**Issue**: When checking if a face should render, the neighbor lookup uses `scale`-multiplied offsets. At chunk boundaries, this neighbor is in an adjacent chunk that may be:
1. At a different LOD level (LOD 0 vs LOD 1)
2. Not yet loaded
3. Using different sampling points

**Severity**: High — Will cause visible seams at every chunk boundary (holes, z-fighting)

**Mitigation**
- Always render faces at chunk boundaries (conservative)
- If neighbor chunk is missing/different LOD, render the face
- Add boundary check before cross-chunk neighbor lookup:
  ```rust
  fn is_at_chunk_boundary(local_pos: IVec3, scale: i32) -> bool {
      local_pos.x < scale || local_pos.x >= CHUNK_SIZE - scale ||
      local_pos.y < scale || local_pos.y >= CHUNK_SIZE - scale ||
      local_pos.z < scale || local_pos.z >= CHUNK_SIZE - scale
  }
  ```

**Verification**: Check chunk boundaries between LOD 0 and LOD 1 zones for holes

---

#### Gotcha: Tangent Generation Skip May Cause Warnings

**Issue**: The current `generate_chunk_mesh` calls `mesh.generate_tangents()` and logs warnings on failure. If LOD meshes skip this but the mesh attributes are inconsistent, Bevy may produce warnings or rendering artifacts.

**Severity**: Low-Medium — Console spam, subtle lighting artifacts on LOD 1 chunks

**Mitigation**
- Either generate tangents for LOD meshes or use a material that ignores them
- If skipping tangents, also disable normal mapping on LOD materials
- A separate material for LOD chunks is acceptable

**Verification**: Check console for tangent-related warnings with LOD chunks

---

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

This phase wires LOD meshing into the render pipeline. Scope: carry LOD level through meshing tasks and chunk state, pick LOD via squared-distance thresholds consistent with existing sorting, and keep the meshing priority order based on squared distances.

### Gotchas to Address in This Phase

#### Gotcha: Sorting by Distance Squared vs Linear Distance

**Issue**: The plan inherits the existing sorting approach which compares `distance_squared`. While correct for ordering, the LOD thresholds use linear distance multiplied then squared. Ensure consistency to avoid chunks at diagonal positions being assigned the wrong LOD level.

**Severity**: Low — May cause minor LOD boundary irregularities

**Mitigation**
- Use squared distances consistently for both comparison and thresholds
- Document that LOD boundaries are "squared distance" based (slightly circular boundaries)

---

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

Adds automatic LOD transitions as the player moves. Without this, chunks only get their LOD level set on initial load. Scope: periodic LOD checks with hysteresis and cooldown, remesh only when expected LOD differs from current, and rely on Bevy `Timer` to avoid drift.

### Gotchas to Address in This Phase

#### Gotcha: Mesh Thrashing During Player Movement

**Standalone PR**: Yes (follow-up) — Can be a separate polish PR after Phase 4 merges.

**Issue**: When a player stands near an LOD boundary, small movements can cause chunks to flip between LOD 0 and LOD 1 repeatedly. Each flip triggers an expensive remesh operation.

**Severity**: Medium — FPS drops when walking near LOD boundaries, visual flickering

**Mitigation**
- Add hysteresis to LOD transitions (different thresholds for upgrade vs downgrade):
  ```rust
  const LOD_HYSTERESIS: f32 = 0.1; // 10% buffer
  
  let upgrade_threshold = lod0_distance_sq;
  let downgrade_threshold = (lod0_distance * (1.0 + LOD_HYSTERESIS)).powi(2);
  ```
- Add minimum time between LOD changes per chunk (e.g., 2 seconds)
- Track `last_lod_change_time` in `ClientChunk`

**Verification**: Walk back and forth across LOD boundary; verify no mesh thrashing

---

#### Gotcha: LOD Check Timer Drift

**Issue**: Using manual timer accumulation (`timer += delta`) without reset can cause drift over time if delta varies significantly.

**Severity**: Low — Negligible gameplay impact

**Mitigation**: Use Bevy's built-in `Timer` resource:
```rust
#[derive(Resource)]
struct LodCheckTimer(Timer);

impl Default for LodCheckTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0.5, TimerMode::Repeating))
    }
}
```

---

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

Without this phase, the server only sends chunks within the original render distance—LOD 1 zones will be empty. Scope: increase broadcast radius to cover the LOD 1 zone, keep chunk-per-tick limits sane (throttle LOD 1 preferentially), and ensure the multiplier matches client config.

### Gotchas to Address in This Phase

#### Gotcha: Server Bandwidth Explosion

**Standalone PR**: Yes (follow-up) — Throttling improvements can be a separate PR after Phase 5.

**Issue**: Increasing `effective_render_distance` by 1.5× increases chunk *volume* by ~3.4× (cubic scaling). The server already has bandwidth throttling (`MAX_CHUNKS_PER_UPDATE = 50`), but initial chunk load will be significantly slower.

**Severity**: Medium — Extremely slow initial world load, server CPU spikes, network congestion in multiplayer

**Mitigation**:
- Consider sending LOD 1 chunks with lower priority than LOD 0
- Add separate throttling for LOD 1 chunks
- Or: Have server pre-compute downsampled LOD 1 data (future enhancement)
- Add server config flag to disable extended broadcast distance

**Verification**: Monitor network traffic during initial load in multiplayer

---

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

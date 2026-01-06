# Level of Detail (LOD) System Implementation Plan

**Goal**: Extend visible range by rendering distant chunks at lower resolution.

## Overview

| LOD Level | Distance Range | Block Scale | Mesh Reduction |
|-----------|----------------|-------------|----------------|
| LOD 0 | 0 to 1× RD | 1:1 | None (full detail) |
| LOD 1 | 1× to 1.5× RD | 2:1 | ~87.5% fewer voxels |

*RD = Render Distance*

**Block scale** means how many source blocks map to one rendered block:
- **LOD 0 (1:1)**: Each block renders individually
- **LOD 1 (2:1)**: Each 2×2×2 group becomes one larger block

**Key architectural decision**: LOD is implemented **entirely client-side**. The server sends full chunk data; the client generates simplified meshes based on distance. This avoids protocol changes and lets chunks transition LOD levels without re-fetching data.

---

## Implementation Phases

| Phase | Description | Effort | Prerequisite |
|-------|-------------|--------|--------------|
| **1** | LOD data structures | Low | — |
| **2** | LOD mesh generation | Medium | Phase 1 |
| **3** | Render integration | Medium | Phase 2 |
| **4** | LOD transitions | Low | Phase 3 |
| **5** | Server broadcast | Low | Phase 3 |

**MVP**: Phases 1–3 + 5. Phase 4 adds polish. Phase 0 can be done anytime before Phase 3.

## Phase 1: LOD Data Structures

> **Effort**: Low (~20 min)

Add the types and helpers needed by later phases. No user-visible changes.

**Deliverables**:
Phase 1 (LOD data structures) has been implemented in the codebase (types and helpers added).

### 1.1 LodLevel Enum

**File**: `shared/src/world/mod.rs`

```rust
/// Level of Detail for chunk rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LodLevel {
    #[default]
    Lod0,  // Full detail: 1 block = 1 rendered block
    Lod1,  // Reduced: 2×2×2 blocks = 1 rendered block
}

impl LodLevel {
    /// Block scale factor (LOD 0 = 1, LOD 1 = 2)
    pub fn block_scale(&self) -> i32 {
        match self {
            LodLevel::Lod0 => 1,
            LodLevel::Lod1 => 2,
        }
    }
    
    /// Determine LOD from squared distance. All params are squared (avoids sqrt).
    pub fn from_distance_squared(
        chunk_distance_sq: i32,
        lod0_threshold_sq: i32,
        lod1_threshold_sq: i32,
    ) -> Self {
        if chunk_distance_sq <= lod0_threshold_sq {
            LodLevel::Lod0
        } else {
            LodLevel::Lod1  // Caller should cull beyond lod1_threshold
        }
    }
}
```

### 1.2 RenderDistance Helpers

**File**: `client/src/world/rendering/render_distance.rs`

```rust
pub const LOD1_DISTANCE_MULTIPLIER: f32 = 1.5;

impl RenderDistance {
    pub fn lod0_distance(&self) -> i32 { self.distance as i32 }
    pub fn lod0_distance_sq(&self) -> i32 { self.lod0_distance().pow(2) }
    pub fn lod1_distance(&self) -> i32 { (self.distance as f32 * LOD1_DISTANCE_MULTIPLIER) as i32 }
    pub fn lod1_distance_sq(&self) -> i32 { self.lod1_distance().pow(2) }
}
```

### 1.3 ClientChunk LOD Tracking

**File**: `client/src/world/data.rs`

```rust
use shared::world::LodLevel;

pub struct ClientChunk {
    pub map: HashMap<IVec3, BlockData>,
    pub entity: Option<Entity>,
    pub last_mesh_ts: Instant,
    pub current_lod: LodLevel,  // NEW
}
```

---

## Phase 2: LOD Mesh Generation

> **Effort**: Medium (~45 min) | **Prerequisite**: Phase 1

Implement LOD meshing logic. After this phase, LOD meshes can be generated but aren't yet used by rendering.

**Core insight**: LOD 1 samples every 2nd block in each dimension, then renders each at 2× size.

### 2.1 Main LOD Meshing Function

**File**: `client/src/world/rendering/meshing.rs`

```rust
/// Generate mesh at specified LOD level. Delegates to generate_chunk_mesh() for LOD 0.
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
    let mut mesh_creator = MeshCreator::default();
    
    // Sample at LOD intervals: for scale=2, positions 0,2,4,6,8,10,12,14
    for lod_x in 0..(CHUNK_SIZE / scale) {
        for lod_y in 0..(CHUNK_SIZE / scale) {
            for lod_z in 0..(CHUNK_SIZE / scale) {
                let local_pos = IVec3::new(lod_x * scale, lod_y * scale, lod_z * scale);
                let Some(block) = chunk.map.get(&local_pos) else { continue };
                
                // Same logic as generate_chunk_mesh, but using scaled variants:
                // - is_lod_block_surrounded(..., scale)
                // - should_render_lod_face(..., scale)  
                // - render_face_scaled(..., scale)
            }
        }
    }
    
    ChunkMeshResponse { solid_mesh: build_mesh(&mesh_creator) }
}
```

### 2.2 Scaled Helper Functions

Create LOD variants of existing helpers:

| Existing | LOD Variant | Change |
|----------|-------------|--------|
| `is_block_surrounded()` | `is_lod_block_surrounded(..., scale)` | Neighbor offsets × scale |
| `should_render_face()` | `should_render_lod_face(..., scale)` | Direction offset × scale |
| `render_face()` | `render_face_scaled(..., scale)` | Vertex positions × scale |

**Example** — vertex scaling in `render_face_scaled`:
```rust
local_vertices.extend(face.vertices.iter().map(|v| {
    [v[0] * scale, v[1] * scale, v[2] * scale]
}));
```

### Phase 2 Gotchas

<details>
<summary><strong>⚠️ Chunk Size Alignment</strong></summary>

`CHUNK_SIZE` must be divisible by the LOD scale. Current `CHUNK_SIZE=16` is safe for scale=2.

Add compile-time assertion:
```rust
const_assert!(CHUNK_SIZE % 2 == 0, "CHUNK_SIZE must be divisible by LOD scales");
```
</details>

<details>
<summary><strong>⚠️ Cross-Chunk Face Culling</strong></summary>

At chunk boundaries, the neighbor block may be in a chunk at different LOD or not yet loaded.

**Solution**: Always render faces at chunk boundaries (conservative). Check if position is within `scale` of any edge before doing cross-chunk lookups.
</details>

<details>
<summary><strong>⚠️ Tangent Generation</strong></summary>

Skip `generate_tangents()` for LOD meshes—no benefit at distance. If this causes console warnings, use a separate material that ignores tangents.
</details>

---

## Phase 3: Render Integration

> **Effort**: Medium (~30 min) | **Prerequisite**: Phase 2

Wire LOD meshing into the render pipeline.

### 3.1 Update MeshingTask

**File**: `client/src/world/rendering/render.rs`

```rust
pub struct MeshingTask {
    pub chunk_pos: IVec3,
    pub mesh_request_ts: Instant,
    pub thread: Task<ChunkMeshResponse>,
    pub lod_level: LodLevel,  // NEW
}
```

### 3.2 Spawn Tasks with LOD Level

In `world_render_system`, when spawning mesh tasks:

```rust
let chunk_distance_sq = pos.distance_squared(player_chunk_pos);
let lod_level = LodLevel::from_distance_squared(
    chunk_distance_sq,
    render_distance.lod0_distance_sq(),
    render_distance.lod1_distance_sq(),
);

let task = pool.spawn(async move {
    meshing::generate_chunk_mesh_lod(&map, &chunk, &pos, &uvs, lod_level)
});
```

### 3.3 Track LOD After Mesh Applied

After mesh is applied to chunk:
```rust
chunk.current_lod = task.lod_level;
```

---

## Phase 4: LOD Transitions

> **Effort**: Low (~20 min) | **Prerequisite**: Phase 3

Add automatic LOD transitions as player moves. Without this, chunks only get their LOD level on initial load.

### 4.1 Transition System

**New file**: `client/src/world/rendering/lod_transitions.rs`

```rust
#[derive(Resource)]
pub struct LodCheckTimer(pub Timer);

impl Default for LodCheckTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0.5, TimerMode::Repeating))
    }
}

/// Check all chunks and trigger remesh when LOD should change
pub fn lod_transition_system(
    time: Res<Time>,
    mut timer: ResMut<LodCheckTimer>,
    render_distance: Res<RenderDistance>,
    world_map: Res<ClientWorldMap>,
    player_query: Query<&Transform, With<CurrentPlayerMarker>>,
    mut render_events: EventWriter<WorldRenderRequestUpdateEvent>,
) {
    if !timer.0.tick(time.delta()).just_finished() { return; }
    
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

### Phase 4 Gotcha: Mesh Thrashing

When player oscillates near LOD boundary, chunks can flip repeatedly.

**Mitigations** (implement as follow-up polish):
- **Hysteresis**: Use different thresholds for upgrade vs downgrade (10% buffer)
- **Cooldown**: Minimum 2 seconds between LOD changes per chunk

---

## Phase 5: Server Broadcast Distance

> **Effort**: Low (~5 min) | **Prerequisite**: Phase 3

Without this, the server only sends chunks within original render distance—LOD 1 zones will be empty.

**File**: `server/src/world/broadcast_world.rs`

```rust
const SERVER_LOD1_MULTIPLIER: f32 = 1.5;  // Must match client

let effective_render_distance = 
    (config.broadcast_render_distance as f32 * SERVER_LOD1_MULTIPLIER) as i32;
```

### Phase 5 Note: Bandwidth Impact

1.5× radius = ~3.4× chunk volume. The existing `MAX_CHUNKS_PER_UPDATE` throttling helps, but initial load will be slower.

**Future improvements** (not required for MVP):
- Prioritize LOD 0 chunks over LOD 1
- Add server config flag to disable extended distance

---

## Configuration Summary

| Constant | Location | Value | Notes |
|----------|----------|-------|-------|
| `LOD1_DISTANCE_MULTIPLIER` | client | 1.5 | LOD 1 zone: 1×–1.5× RD |
| `SERVER_LOD1_MULTIPLIER` | server | 1.5 | **Must match client** |

**Tuning**: 1.25 = conservative, 2.0 = see further but more memory/bandwidth.

---

## Testing Checklist

- [ ] LOD 0 chunks render at full detail within render distance
- [ ] LOD 1 chunks render at 2× block scale beyond render distance
- [ ] Block selection/interaction only works on LOD 0 chunks
- [ ] No visible holes at chunk boundaries
- [ ] No visible holes between LOD 0 and LOD 1 zones
- [ ] LOD transitions occur as player moves
- [ ] No mesh thrashing when player is stationary
- [ ] Teleporting handles LOD correctly

---

## Known Risks & Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| Memory increase (~3.4×) | Medium | Add F3 readout; consider config flag to disable LOD |
| Bandwidth increase (~3.4×) | Medium | Existing throttling; prioritize LOD 0 sends |
| LOD boundary seams | Low | Conservative face rendering at boundaries |
| Block interactions on LOD 1 | Low | Raycast should respect LOD 0 boundary |

---

## Performance Expectations

| Metric | Without LOD | With LOD |
|--------|-------------|----------|
| Visible Range | 1× RD | 1.5× RD |
| Chunk Count | Baseline | ~3.4× |
| Mesh Complexity | Baseline | ~70% (LOD 1 has 87.5% fewer voxels) |
| Expected FPS | Baseline | +10-20% net gain |

---

## Future Enhancements

| Enhancement | Effort | Impact |
|-------------|--------|--------|
| LOD 2 (4:1 scale, 2–3× RD) | Medium | High |
| Terrain-only LOD (skip flora) | Low | Medium |
| Greedy meshing for LOD | High | Medium |
| Smooth LOD transitions (crossfade) | High | Low |

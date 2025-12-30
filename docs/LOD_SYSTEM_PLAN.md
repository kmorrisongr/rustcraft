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

### Data Flow

```
Server                              Client
  │                                   │
  │  Full-resolution ServerChunk      │
  │ ─────────────────────────────────>│
  │                                   │
  │                                   ├── Calculate LOD level based on distance
  │                                   │
  │                                   ├── LOD 0: generate_chunk_mesh() [existing]
  │                                   │
  │                                   └── LOD 1: generate_lod_chunk_mesh()
```

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

```rust
use crate::{
    constants::DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS,
    input::{data::GameAction, keyboard::is_action_just_pressed},
    KeyMap,
};
use bevy::prelude::*;

/// Multiplier for LOD 1 range (1.0 to 1.5× render distance)
pub const LOD1_DISTANCE_MULTIPLIER: f32 = 1.5;

#[derive(Resource, Default, Reflect)]
pub struct RenderDistance {
    pub distance: u32,
}

impl RenderDistance {
    /// Returns the maximum distance (in chunks) for LOD 0 rendering
    pub fn lod0_distance(&self) -> i32 {
        self.distance as i32
    }
    
    /// Returns the squared LOD 0 distance (for use with from_distance_squared)
    pub fn lod0_distance_sq(&self) -> i32 {
        let d = self.lod0_distance();
        d * d
    }
    
    /// Returns the maximum distance (in chunks) for LOD 1 rendering
    pub fn lod1_distance(&self) -> i32 {
        (self.distance as f32 * LOD1_DISTANCE_MULTIPLIER) as i32
    }
    
    /// Returns the squared LOD 1 distance (for use with from_distance_squared)
    pub fn lod1_distance_sq(&self) -> i32 {
        let d = self.lod1_distance();
        d * d
    }
    
    /// Returns the total effective render distance including all LOD levels
    pub fn total_distance(&self) -> i32 {
        self.lod1_distance()
    }
}

pub fn render_distance_update_system(
    mut render_distance: ResMut<RenderDistance>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
) {
    if render_distance.distance == 0 {
        render_distance.distance = DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS;
    }

    if is_action_just_pressed(GameAction::RenderDistanceMinus, &keyboard_input, &key_map) {
        render_distance.distance = render_distance.distance.saturating_sub(1).max(1);
    }

    if is_action_just_pressed(GameAction::RenderDistancePlus, &keyboard_input, &key_map) {
        render_distance.distance += 1;
    }
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
    /// Current LOD level this chunk is rendered at
    pub current_lod: LodLevel,
}

impl Default for ClientChunk {
    fn default() -> Self {
        Self {
            map: HashMap::new(),
            entity: None,
            last_mesh_ts: Instant::now(),
            current_lod: LodLevel::Lod0,
        }
    }
}
```

If stretched textures look blurry at distance, tile UVs by scaling them with `scale` instead of reusing the base range.

---

## Phase 2: LOD Meshing
> **Effort**: Medium (~45 min) | **Impact**: High | **Prerequisite**: Phase 1

This phase implements the core LOD mesh generation. After this phase, the codebase *can* generate LOD meshes, but they won't be used until Phase 3 wires them into the render system.

### Existing Code to Leverage

| Component | Location | Reuse Strategy |
|-----------|----------|----------------|
| `generate_chunk_mesh()` | [meshing.rs#L50](../client/src/world/rendering/meshing.rs#L50) | Mirror structure, delegate for LOD 0 |
| `MeshCreator` struct | [meshing.rs#L27](../client/src/world/rendering/meshing.rs#L27) | Use unchanged |
| `build_mesh()` | [meshing.rs#L35](../client/src/world/rendering/meshing.rs#L35) | Use unchanged |
| `is_block_surrounded()` | [meshing.rs#L172](../client/src/world/rendering/meshing.rs#L172) | Create scaled variant |
| `should_render_face()` | [meshing.rs#L262](../client/src/world/rendering/meshing.rs#L262) | Create scaled variant |
| `render_face()` | [meshing.rs#L224](../client/src/world/rendering/meshing.rs#L224) | Add `scale` parameter |
| `VoxelShape` | [voxel.rs](../client/src/world/rendering/voxel.rs) | Use unchanged |

### 2.1 LOD Mesh Generation Function

The core insight is that LOD meshing samples blocks at intervals determined by the LOD level's block scale. For LOD 1 (scale=2), we sample every 2nd block in each dimension. Assumes `CHUNK_SIZE` is divisible by the LOD scale (16 works for 2 and future 4).

**File**: `client/src/world/rendering/meshing.rs`

Add the following function alongside the existing `generate_chunk_mesh`:

```rust
use shared::world::LodLevel;
use shared::constants::CHUNK_SIZE; // 16 - must be divisible by LOD scale (2, 4, etc.)

/// Generates a mesh for a chunk at the specified LOD level
/// 
/// For LOD 0, this delegates to the standard generate_chunk_mesh.
/// For LOD 1+, blocks are sampled at intervals and rendered at larger scales.
pub(crate) fn generate_chunk_mesh_lod(
    world_map: &ClientWorldMap,
    chunk: &ClientChunk,
    chunk_pos: &IVec3,
    uv_map: &HashMap<String, UvCoords>,
    lod_level: LodLevel,
) -> ChunkMeshResponse {
    // LOD 0 uses standard meshing
    if lod_level == LodLevel::Lod0 {
        return generate_chunk_mesh(world_map, chunk, chunk_pos, uv_map);
    }
    
    let start = Instant::now();
    let scale = lod_level.block_scale();
    let scale_f32 = scale as f32;
    
    let mut solid_mesh_creator = MeshCreator::default();
    
    // Sample blocks at LOD intervals
    // For LOD 1 (scale=2): sample at 0, 2, 4, 6, 8, 10, 12, 14
    for lod_x in 0..(CHUNK_SIZE / scale) {
        for lod_y in 0..(CHUNK_SIZE / scale) {
            for lod_z in 0..(CHUNK_SIZE / scale) {
                let local_block_pos = IVec3::new(
                    lod_x * scale,
                    lod_y * scale,
                    lod_z * scale,
                );
                
                // Get the representative block for this LOD cell
                // Use the block at the sample position, or find dominant block in the cell
                let block = match get_representative_block(chunk, &local_block_pos, scale) {
                    Some(b) => b,
                    None => continue, // All air in this LOD cell
                };
                
                let x = local_block_pos.x as f32;
                let y = local_block_pos.y as f32;
                let z = local_block_pos.z as f32;
                
                let global_block_pos = &to_global_pos(chunk_pos, &local_block_pos);
                let visibility = block.id.get_visibility();
                
                // Skip if surrounded (check at LOD scale)
                if is_lod_block_surrounded(world_map, global_block_pos, &visibility, &block.id, scale) {
                    continue;
                }
                
                let mut local_vertices: Vec<[f32; 3]> = vec![];
                let mut local_indices: Vec<u32> = vec![];
                let mut local_normals: Vec<[f32; 3]> = vec![];
                let mut local_uvs: Vec<[f32; 2]> = vec![];
                let mut local_colors: Vec<[f32; 4]> = vec![];
                
                // Create a scaled voxel shape
                let voxel = VoxelShape::create_from_block(&block);
                
                for face in voxel.faces.iter() {
                    let uv_coords = uv_map.get(&face.texture)
                        .unwrap_or_else(|| uv_map.get("_Default").unwrap());
                    
                    let alpha = match visibility {
                        BlockTransparency::Liquid => 0.5,
                        _ => 1.0,
                    };
                    
                    if should_render_lod_face(world_map, global_block_pos, &face.direction, &visibility, scale) {
                        render_face_scaled(
                            &mut local_vertices,
                            &mut local_indices,
                            &mut local_normals,
                            &mut local_uvs,
                            &mut local_colors,
                            &mut solid_mesh_creator.indices_offset,
                            face,
                            uv_coords,
                            1.0,
                            alpha,
                            scale_f32,
                        );
                    }
                }
                
                // Translate vertices to block position
                let local_vertices: Vec<[f32; 3]> = local_vertices
                    .iter()
                    .map(|v| {
                        let v = rotate_vertices(v, &block.direction);
                        [v[0] + x, v[1] + y, v[2] + z]
                    })
                    .collect();
                
                solid_mesh_creator.vertices.extend(local_vertices);
                solid_mesh_creator.indices.extend(local_indices);
                solid_mesh_creator.normals.extend(local_normals);
                solid_mesh_creator.uvs.extend(local_uvs);
                solid_mesh_creator.colors.extend(local_colors);
            }
        }
    }
    
    let should_return_solid = !solid_mesh_creator.vertices.is_empty();
    let mut solid_mesh = build_mesh(&solid_mesh_creator);
    
    if should_return_solid {
        // Optional: skip tangents for LOD>0 to save CPU if distant materials do not need normal maps
        if let Err(e) = solid_mesh.generate_tangents() {
            warn!("Error generating tangents for LOD mesh: {:?}", e);
        }
    }
    
    trace!("LOD {} render time: {:?}", scale, Instant::now() - start);
    
    ChunkMeshResponse {
        solid_mesh: if should_return_solid { Some(solid_mesh) } else { None },
    }
}

/// Get the representative block for an LOD cell.
/// 
/// Uses simple corner sampling for performance. At LOD distances, the visual
/// difference from priority-based selection is negligible.
/// 
/// Future enhancement: scan all blocks in the cell and pick by priority
/// (solid > transparent > liquid > decoration) for better tree/structure preservation.
fn get_representative_block(chunk: &ClientChunk, base_pos: &IVec3, _scale: i32) -> Option<BlockData> {
    chunk.map.get(base_pos).copied()
}

/// Check if an LOD block is surrounded (at LOD scale). Uses scaled offsets intentionally, ignoring sub-cell detail to avoid leaks between LOD cells.
fn is_lod_block_surrounded(
    world_map: &ClientWorldMap,
    global_block_pos: &IVec3,
    block_visibility: &BlockTransparency,
    block_id: &BlockId,
    scale: i32,
) -> bool {
    let offset_scale = IVec3::splat(scale);
    
    for offset in &shared::world::SIX_OFFSETS {
        let neighbor_pos = *global_block_pos + (*offset * offset_scale);
        
        if let Some(block) = world_map.get_block_by_coordinates(&neighbor_pos) {
            let vis = block.id.get_visibility();
            match vis {
                BlockTransparency::Solid => {}
                BlockTransparency::Decoration => return false,
                BlockTransparency::Liquid => {
                    if vis != *block_visibility {
                        return false;
                    }
                }
                BlockTransparency::Transparent => {
                    if *block_id != block.id {
                        return false;
                    }
                }
            }
        } else {
            return false;
        }
    }
    
    true
}

/// Check if a face should render at LOD scale
fn should_render_lod_face(
    world_map: &ClientWorldMap,
    global_block_pos: &IVec3,
    direction: &FaceDirection,
    block_visibility: &BlockTransparency,
    scale: i32,
) -> bool {
    let offset = match *direction {
        FaceDirection::Front => IVec3::new(0, 0, -scale),
        FaceDirection::Back => IVec3::new(0, 0, scale),
        FaceDirection::Top => IVec3::new(0, scale, 0),
        FaceDirection::Bottom => IVec3::new(0, -scale, 0),
        FaceDirection::Left => IVec3::new(-scale, 0, 0),
        FaceDirection::Right => IVec3::new(scale, 0, 0),
        FaceDirection::Inset => return true,
    };
    
    if let Some(block) = world_map.get_block_by_coordinates(&(*global_block_pos + offset)) {
        let vis = block.id.get_visibility();
        match vis {
            BlockTransparency::Solid => false,
            BlockTransparency::Decoration => true,
            BlockTransparency::Transparent | BlockTransparency::Liquid => *block_visibility != vis,
        }
    } else {
        true
    }
}

/// Render a face scaled by the LOD factor
fn render_face_scaled(
    local_vertices: &mut Vec<[f32; 3]>,
    local_indices: &mut Vec<u32>,
    local_normals: &mut Vec<[f32; 3]>,
    local_uvs: &mut Vec<[f32; 2]>,
    local_colors: &mut Vec<[f32; 4]>,
    indices_offset: &mut u32,
    face: &Face,
    uv_coords: &UvCoords,
    color_multiplier: f32,
    alpha: f32,
    scale: f32,
) {
    // Scale vertices by LOD factor
    local_vertices.extend(face.vertices.iter().map(|v| {
        [v[0] * scale, v[1] * scale, v[2] * scale]
    }));
    
    local_indices.extend(face.indices.iter().map(|x| x + *indices_offset));
    *indices_offset += face.vertices.len() as u32;
    
    local_normals.extend(face.normals.iter());
    
    let colors = face.colors.iter();
    let mut new_colors = vec![];
    for color in colors {
        new_colors.push([
            color[0] * color_multiplier,
            color[1] * color_multiplier,
            color[2] * color_multiplier,
            alpha,
        ]);
    }
    local_colors.extend(new_colors);
    
    // UVs remain the same - this stretches the texture over the larger face
    local_uvs.extend(face.uvs.iter().map(|uv| {
        [
            (uv[0] + uv_coords.u0 + 0.001).min(uv_coords.u1 - 0.001),
            (uv[1] + uv_coords.v0 + 0.001).min(uv_coords.v1 - 0.001),
        ]
    }));
}
```

### 2.2 Performance Optimization: Skip Tangents for LOD Meshes

Distant chunks don't benefit from normal mapping. Skip tangent generation to save CPU:

```rust
// In generate_chunk_mesh_lod, after build_mesh():
if should_return_solid && lod_level == LodLevel::Lod0 {
    // Only generate tangents for full-detail meshes
    if let Err(e) = solid_mesh.generate_tangents() {
        warn!("Error generating tangents for LOD mesh: {:?}", e);
    }
}
```

---

## Phase 3: Render System Integration
> **Effort**: Medium (~30 min) | **Impact**: High | **Prerequisite**: Phase 2

This phase wires LOD meshing into the render pipeline. After this phase, chunks will render at the appropriate LOD level based on distance.

### Existing Code to Leverage

| Component | Location | Reuse Strategy |
|-----------|----------|----------------|
| `MeshingTask` struct | [render.rs#L27](../client/src/world/rendering/render.rs#L27) | Add `lod_level` field |
| `world_render_system()` | [render.rs#L78](../client/src/world/rendering/render.rs#L78) | Add LOD calculation |
| Distance sorting | [render.rs#L140](../client/src/world/rendering/render.rs#L140) | Already computes `distance_squared` |
| `AsyncComputeTaskPool` | [render.rs#L100](../client/src/world/rendering/render.rs#L100) | Use unchanged |

### 3.1 Update Mesh Generation to Use LOD

The render system needs to:
1. Calculate the LOD level for each chunk based on player distance
2. Track the current LOD level of each chunk
3. Re-mesh chunks when their LOD level changes

**Modifications to `render.rs`**:

```rust
use shared::world::{global_block_to_chunk_pos, LodLevel};
use crate::world::rendering::render_distance::RenderDistance;

// Add to MeshingTask struct
#[derive(Debug)]
pub struct MeshingTask {
    pub chunk_pos: IVec3,
    pub mesh_request_ts: Instant,
    pub thread: Task<ChunkMeshResponse>,
    pub lod_level: LodLevel, // Track LOD level for this mesh task
}

// In world_render_system, add render_distance resource and player query
pub fn world_render_system(
    mut world_map: ResMut<ClientWorldMap>,
    material_resource: Res<MaterialResource>,
    render_distance: Res<RenderDistance>,
    mut ev_render: EventReader<WorldRenderRequestUpdateEvent>,
    player_query: Query<&Transform, With<CurrentPlayerMarker>>,
    // ... rest of parameters
) {
    // ... existing code ...
    
    // Get player chunk position for LOD calculations
    let player_chunk_pos = player_query.get_single().map(|t| {
        global_block_to_chunk_pos(&IVec3::new(
            t.translation.x as i32,
            t.translation.y as i32,
            t.translation.z as i32,
        ))
    }).unwrap_or(IVec3::ZERO);
    
    // When spawning mesh tasks:
    for pos in chunks_to_reload {
        if let Some(chunk) = world_map.map.get(&pos) {
            if chunk.map.is_empty() {
                continue;
            }
            
            // Calculate LOD level for this chunk (uses squared distances)
            let chunk_distance_sq = pos.distance_squared(player_chunk_pos);
            let lod_level = LodLevel::from_distance_squared(
                chunk_distance_sq,
                render_distance.lod0_distance_sq(),
                render_distance.lod1_distance_sq(),
            );
            
            // Note: LOD transitions are handled by lod_transition_system (Stage 4).
            // This system only processes explicit ChunkToReload events.
            if events.contains(&WorldRenderRequestUpdateEvent::ChunkToReload(pos)) {
                let map_clone = Arc::clone(&map_ptr);
                let uvs_clone = Arc::clone(&uvs);
                let ch = chunk.clone();
                let lod = lod_level;
                
                let t = pool.spawn(async move {
                    world::meshing::generate_chunk_mesh_lod(&map_clone, &ch, &pos, &uvs_clone, lod)
                });
                
                queued_meshes.meshes.push(MeshingTask {
                    chunk_pos: pos,
                    mesh_request_ts: Instant::now(),
                    thread: t,
                    lod_level,
                });
            }
        }
    }
    
    // ... rest of existing code ...
}
```

When applying a completed mesh, update the chunk's LOD tracking:

```rust
// After mesh is applied to entity:
if let Some(chunk) = world_map.map.get_mut(&task.chunk_pos) {
    chunk.current_lod = task.lod_level;
}
```

This prevents remesh thrashing as the player moves.

---

## Phase 4: LOD Transition System
> **Effort**: Low (~20 min) | **Impact**: Medium | **Prerequisite**: Phase 3

This phase adds automatic LOD transitions as the player moves. Without this phase, chunks only get their LOD level set when first loaded or explicitly reloaded—moving toward/away from a chunk won't update its LOD.

### Existing Code to Leverage

| Component | Location | Reuse Strategy |
|-----------|----------|----------------|
| `WorldRenderRequestUpdateEvent` | [data.rs#L89](../client/src/world/data.rs#L89) | Emit to trigger remesh |
| `CurrentPlayerMarker` | [player/mod.rs](../client/src/player/mod.rs) | Query for player position |
| Bevy `Time` resource | Built-in | For timer-based checks |

### 4.1 Create LOD Transition System

**New file**: `client/src/world/rendering/lod_transitions.rs`

This system is the **sole authority** for LOD-based remeshing. It periodically checks all loaded chunks and triggers re-meshing when their LOD level should change based on player movement. The render system (Phase 3) only responds to these events—it does not independently check for LOD changes.

```rust
use bevy::prelude::*;
use bevy::math::IVec3;
use shared::world::{global_block_to_chunk_pos, LodLevel};
use crate::player::CurrentPlayerMarker;
use crate::world::{ClientWorldMap, WorldRenderRequestUpdateEvent};
use super::RenderDistance;

/// How often to check for LOD transitions (in seconds)
const LOD_CHECK_INTERVAL: f32 = 0.5;

#[derive(Resource, Default)]
pub struct LodCheckTimer {
    pub timer: f32,
}

/// System that checks if chunks need LOD transitions
pub fn lod_transition_system(
    time: Res<Time>,
    mut timer: ResMut<LodCheckTimer>,
    render_distance: Res<RenderDistance>,
    world_map: Res<ClientWorldMap>,
    player_query: Query<&Transform, With<CurrentPlayerMarker>>,
    mut render_events: EventWriter<WorldRenderRequestUpdateEvent>,
) {
    timer.timer += time.delta_secs();
    
    if timer.timer < LOD_CHECK_INTERVAL {
        return;
    }
    timer.timer = 0.0;
    
    let player_transform = match player_query.get_single() {
        Ok(t) => t,
        Err(_) => return,
    };
    
    let player_pos = player_transform.translation;
    let player_chunk = global_block_to_chunk_pos(&IVec3::new(
        player_pos.x as i32,
        player_pos.y as i32,
        player_pos.z as i32,
    ));
    
    let lod0_dist_sq = render_distance.lod0_distance_sq();
    let lod1_dist_sq = render_distance.lod1_distance_sq();
    
    for (chunk_pos, chunk) in world_map.map.iter() {
        let distance_sq = chunk_pos.distance_squared(player_chunk);
        let expected_lod = LodLevel::from_distance_squared(distance_sq, lod0_dist_sq, lod1_dist_sq);
        
        if expected_lod != chunk.current_lod {
            render_events.send(WorldRenderRequestUpdateEvent::ChunkToReload(*chunk_pos));
        }
    }
}
```

---

## Phase 5: Server Broadcast Distance
> **Effort**: Low (~5 min) | **Impact**: High | **Prerequisite**: Phase 3

Without this phase, the server only sends chunks within the original render distance—LOD 1 zones will be empty. This is a critical piece for the system to function.

### Existing Code to Leverage

| Component | Location | Reuse Strategy |
|-----------|----------|----------------|
| `broadcast_render_distance` | [broadcast_world.rs#L156](../server/src/world/broadcast_world.rs#L156) | Multiply by LOD factor |
| `get_world_map_chunks_to_send()` | [broadcast_world.rs#L153](../server/src/world/broadcast_world.rs#L153) | Use unchanged |

### 5.1 Expand Server Broadcast Distance

**File**: `server/src/world/broadcast_world.rs`

```rust
/// Must match LOD1_DISTANCE_MULTIPLIER in client render_distance.rs
const SERVER_LOD1_MULTIPLIER: f32 = 1.5;

// In broadcast_world_state or get_world_map_chunks_to_send:
let effective_render_distance = (config.broadcast_render_distance as f32 * SERVER_LOD1_MULTIPLIER) as i32;
```

### Bandwidth Consideration

Expanding broadcast to 1.5× render distance increases chunk volume by ~3.4× (sphere volume scales with r³). Consider:
- Throttling LOD 1 chunk sends to lower priority
- Sending LOD 1 chunks at reduced frequency
- Adding a server config flag to disable extended broadcast

**Future enhancement**: Negotiate LOD distance in the authentication handshake instead of hardcoding.

---

## Configuration

### Constants Summary

| Constant | Value | Location | Description |
|----------|-------|----------|-------------|
| `LOD1_DISTANCE_MULTIPLIER` | 1.5 | Client | LOD 1 extends from 1× to 1.5× render distance |
| `LOD_CHECK_INTERVAL` | 0.5s | Client | How often to check for LOD transitions |
| `SERVER_LOD1_MULTIPLIER` | 1.5 | Server | Must match client multiplier |

### Tuning Recommendations

1. **LOD1_DISTANCE_MULTIPLIER**: 
   - Lower values (1.25) = Subtle extension, conservative memory use
   - Higher values (2.0) = See further, but more memory and potential pop-in

2. **LOD_CHECK_INTERVAL**:
   - Lower values (0.1) = Smoother transitions, more CPU overhead
   - Higher values (1.0) = Less CPU, but transitions may be noticeable

---

## Testing Checklist

### Functional Tests
- [ ] LOD 0 chunks render at full detail within render distance
- [ ] LOD 1 chunks render at 2× block scale beyond render distance  
- [ ] Block selection/interaction only works on LOD 0 chunks
- [ ] Chunk unloading works correctly for chunks beyond LOD 1 distance
- [ ] No visual artifacts at LOD boundaries (seams, holes, z-fighting)

### Performance Tests
- [ ] LOD transitions complete within 500ms of crossing threshold
- [ ] FPS with LOD at 1.5× RD ≥ 90% of FPS without LOD at 1× RD
- [ ] Memory usage at 1.5× RD with LOD ≤ 150% of memory at 1× RD without LOD
- [ ] No mesh thrashing (repeated remesh of same chunk) when player is stationary

### Edge Cases
- [ ] Rapidly changing render distance doesn't cause crashes
- [ ] Teleporting long distances handles LOD correctly
- [ ] Chunks at exactly LOD boundary distance behave consistently

---

## Known Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **Texture stretching** looks blurry on high-contrast blocks | Medium | Low | Acceptable at distance; can tile UVs if needed |
| **LOD boundary seams** where LOD 0/1 chunks meet | Medium | Medium | Only visible at chunk edges; consider crossfade in future |
| **Block interactions** on LOD 1 chunks confuse players | Low | Medium | Ensure raycast respects LOD 0 boundary |
| **Memory spike** during LOD transitions | Low | Low | Current despawn-before-spawn pattern handles this |
| **Server bandwidth** increase (~3.4× chunk volume) | High | Medium | Throttle LOD 1 sends; add server config flag |

---

## Performance Expectations

| Metric | LOD 0 Only | With LOD 1 |
|--------|------------|------------|
| Visible Range | 1× RD | 1.5× RD |
| Chunk Count | ~4/3πr³ | ~4/3π(1.5r)³ = 3.4× |
| Mesh Complexity | Baseline | ~70% (LOD 1 has 87.5% fewer voxels) |
| Expected FPS Impact | Baseline | +10-20% (net positive due to reduced detail) |

---

## Future Enhancements

| Enhancement | Effort | Impact | Description |
|-------------|--------|--------|-------------|
| **LOD 2** | Medium | High | 4:1 scale for very distant terrain (2× to 3× RD) |
| **Greedy Meshing for LOD** | High | Medium | Apply greedy meshing optimization to LOD meshes |
| **Terrain-Only LOD** | Low | Medium | Only render terrain blocks (no flora/decorations) at LOD 1+ |
| **Smooth Transitions** | High | Low | Fade/blend between LOD levels to reduce pop-in |
| **Configurable Per-Biome** | Medium | Low | Different LOD settings for different biomes |
| **Negotiate LOD in Auth** | Low | Medium | Client/server agree on LOD multiplier during handshake |

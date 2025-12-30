# Frustum Culling Implementation Plan

## Overview

This document outlines a plan to implement frustum culling for Rustcraft, enabling efficient chunk visibility management. The goal is to ensure only chunks within the player's field of view (FOV) are:
1. Transmitted from server to client (bandwidth optimization)
2. Rendered by the client (GPU optimization)

## Recommended Approach: Staged Implementation

**Key Insight**: The existing view-direction prioritization system can be extended to perform culling with minimal changes. A full frustum implementation is optional and provides diminishing returns.

### Stage 1: Cone Culling (Recommended First Step)

Add a hard cull threshold to the existing dot-product system. This gets ~30-40% bandwidth reduction with ~10 lines of code changes.

### Stage 2: Full Frustum (Optional Enhancement)

Implement proper 6-plane frustum culling with fixed conservative parameters on the server. Skip client synchronization—use a 90° FOV assumption that covers all reasonable client configurations.

### Stage 3: Client Sync (Only If Needed)

Only implement if players with custom FOV settings report issues. This adds significant complexity for marginal benefit.

| Approach | Bandwidth Reduction | Implementation Effort |
|----------|--------------------|-----------------------|
| Current (prioritize only) | 0% | N/A |
| Cone cull (dot threshold) | ~30-40% | 2-3 hours |
| Full frustum (fixed params) | ~50-60% | 1-2 days |
| Full frustum (synced params) | ~55-65% | 3-4 days |

---

## Current Architecture Analysis

### Existing Chunk Prioritization

The server already has a sophisticated chunk prioritization system in [server/src/world/broadcast_world.rs](../server/src/world/broadcast_world.rs):

- **`get_chunk_render_score()`**: Calculates a priority score based on distance and view direction using dot product
- **`get_player_chunks_prioritized()`**: Returns chunks sorted by priority within render distance
- **`FORWARD_DOT_THRESHOLD`**: Currently set to `-0.3` (~108° from center), meaning chunks behind the player are deprioritized but still sent

**Limitation**: The current system *prioritizes* chunks in the player's view direction but does not *cull* chunks outside the actual camera frustum. All chunks within render distance are eventually sent.

**Opportunity**: The existing dot-product infrastructure can be extended to perform hard culling with minimal code changes.

### Current Data Flow

```
Server:
1. Get player position and camera_transform
2. Get all chunks within spherical render distance
3. Prioritize by view direction (dot product scoring)
4. Send up to MAX_CHUNKS_PER_UPDATE chunks per tick

Client:
1. Receive chunks via WorldUpdate message
2. Store in ClientWorldMap
3. Generate meshes for all received chunks
4. Render all meshed chunks (Bevy handles basic frustum culling)
```

### Key Components

| Component | Location | Purpose |
|-----------|----------|---------|
| `Player.camera_transform` | [shared/src/players/data.rs](../shared/src/players/data.rs#L129) | Stores player's view direction |
| `broadcast_world_state()` | [server/src/world/broadcast_world.rs](../server/src/world/broadcast_world.rs#L83) | Main chunk broadcasting system |
| `get_player_nearby_chunks_coords()` | [server/src/world/broadcast_world.rs](../server/src/world/broadcast_world.rs#L272) | Gets chunks in spherical radius |
| `ClientWorldMap` | [client/src/world/data.rs](../client/src/world/data.rs#L38) | Client-side chunk storage |
| Camera FOV | [client/src/camera/spawn.rs](../client/src/camera/spawn.rs#L44) | Currently 60° FOV |

## Frustum Culling Design

### Understanding the Frustum

A view frustum is a truncated pyramid defined by six planes:
- **Near plane**: Closest visible distance
- **Far plane**: Farthest visible distance (render distance × chunk size)
- **Left/Right planes**: Horizontal FOV boundaries
- **Top/Bottom planes**: Vertical FOV boundaries

```
        Far Plane
    _______________
   /               \
  /                 \    ← Frustum Volume
 /                   \
/_____________________\
|                     |
|     Camera/Player   |
        Near Plane
```

### Frustum Parameters Required

To construct a frustum on the server, we need:

| Parameter | Current Source | Notes |
|-----------|----------------|-------|
| Camera Position | `Player.position` + eye offset | Need to add ~0.8 for eye level |
| View Direction | `Player.camera_transform.forward()` | Already synchronized |
| FOV (Horizontal) | Hardcoded 60° on client | **Need to synchronize** |
| Aspect Ratio | Window dimensions | **Need to synchronize or assume** |
| Near Distance | ~0.1 (default) | Not critical for chunk culling |
| Far Distance | `render_distance × CHUNK_SIZE` | Already available |

### Critical Insight: Chunk AABB Testing

Chunks are axis-aligned boxes. A chunk is visible if **any part** of its volume intersects the frustum. This requires:

1. Computing the chunk's world-space bounding box:
   ```rust
   let chunk_min = chunk_pos * CHUNK_SIZE;
   let chunk_max = chunk_min + IVec3::splat(CHUNK_SIZE);
   ```

2. Testing the AABB against all 6 frustum planes using the "separating axis theorem":
   - A chunk is **outside** if it's entirely on the negative side of any plane
   - A chunk is **inside or intersecting** otherwise

## Implementation Plan

### Phase 0: Cone Culling (Minimal Change)

**Modify**: `server/src/world/broadcast_world.rs`

This is the simplest approach that provides significant benefit with minimal risk:

```rust
/// Dot product threshold for hard culling chunks definitely behind the player.
/// -0.7 corresponds to ~135° from forward direction.
/// Chunks beyond this angle are culled entirely, not just deprioritized.
const CULL_DOT_THRESHOLD: f32 = -0.7;

/// Get chunk coordinates around a player, culling those behind and prioritizing by view direction
fn get_player_chunks_prioritized(player: &Player, radius: i32, max_chunks: usize) -> Vec<IVec3> {
    let player_chunk_pos = world_position_to_chunk_position(player.position);
    let forward = player.camera_transform.forward();

    let mut chunks: Vec<IVec3> = get_player_nearby_chunks_coords(player_chunk_pos, radius)
        .into_iter()
        // Hard cull chunks definitely behind player
        .filter(|chunk_pos| {
            let direction = (*chunk_pos - player_chunk_pos).as_vec3().normalize_or_zero();
            forward.dot(direction) > CULL_DOT_THRESHOLD
        })
        .collect();

    // Prioritize remaining chunks by view direction (existing logic)
    let sort_count = chunks.len().min(max_chunks);
    if chunks.len() > 1 {
        chunks.select_nth_unstable_by(sort_count - 1, |a, b| {
            order_chunks_by_render_score(a, b, player_chunk_pos, *forward)
        });
    }

    chunks
}
```

**Benefits**:
- ~10 lines of changes to existing code
- No new files or structs needed
- Easy to tune `CULL_DOT_THRESHOLD` based on testing
- Fallback is trivial (set threshold to -1.0 to disable)

**Testing**: After implementing, monitor chunk send rates and check for pop-in when turning quickly. Adjust threshold as needed.

---

### Phase 1: Shared Frustum Types (shared/) — Optional Enhancement

**Create**: `shared/src/frustum.rs`

```rust
use bevy::math::{Vec3, Mat4};
use serde::{Deserialize, Serialize};

/// A plane in 3D space represented by normal and distance from origin
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Plane {
    pub normal: Vec3,
    pub d: f32,
}

impl Plane {
    /// Create a plane from a normal and a point on the plane
    pub fn from_normal_and_point(normal: Vec3, point: Vec3) -> Self {
        let normal = normal.normalize();
        Self {
            normal,
            d: -normal.dot(point),
        }
    }

    /// Signed distance from a point to the plane
    /// Positive = in front, Negative = behind
    pub fn distance_to_point(&self, point: Vec3) -> f32 {
        self.normal.dot(point) + self.d
    }
}

/// View frustum represented by 6 planes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewFrustum {
    pub planes: [Plane; 6],
}

/// Frustum configuration sent from client to server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrustumConfig {
    pub fov_radians: f32,
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for FrustumConfig {
    fn default() -> Self {
        Self {
            fov_radians: 60.0_f32.to_radians(),
            aspect_ratio: 16.0 / 9.0, // Common default
            near: 0.1,
            far: 256.0, // Will be overridden by render distance
        }
    }
}

impl ViewFrustum {
    /// Construct frustum from camera position, rotation, and config
    pub fn from_camera(
        position: Vec3,
        forward: Vec3,
        up: Vec3,
        config: &FrustumConfig,
    ) -> Self {
        let right = forward.cross(up).normalize();
        let up = right.cross(forward).normalize();

        let half_v_side = config.far * (config.fov_radians / 2.0).tan();
        let half_h_side = half_v_side * config.aspect_ratio;

        let far_center = position + forward * config.far;

        // Near plane
        let near_plane = Plane::from_normal_and_point(forward, position + forward * config.near);

        // Far plane
        let far_plane = Plane::from_normal_and_point(-forward, far_center);

        // Calculate frustum corner for side planes
        let far_top_right = far_center + up * half_v_side + right * half_h_side;
        let far_top_left = far_center + up * half_v_side - right * half_h_side;
        let far_bottom_right = far_center - up * half_v_side + right * half_h_side;
        let far_bottom_left = far_center - up * half_v_side - right * half_h_side;

        // Left plane (position, far_top_left, far_bottom_left)
        let left_normal = (far_top_left - position).cross(far_bottom_left - position).normalize();
        let left_plane = Plane::from_normal_and_point(left_normal, position);

        // Right plane (position, far_bottom_right, far_top_right)
        let right_normal = (far_bottom_right - position).cross(far_top_right - position).normalize();
        let right_plane = Plane::from_normal_and_point(right_normal, position);

        // Top plane (position, far_top_right, far_top_left)
        let top_normal = (far_top_right - position).cross(far_top_left - position).normalize();
        let top_plane = Plane::from_normal_and_point(top_normal, position);

        // Bottom plane (position, far_bottom_left, far_bottom_right)
        let bottom_normal = (far_bottom_left - position).cross(far_bottom_right - position).normalize();
        let bottom_plane = Plane::from_normal_and_point(bottom_normal, position);

        Self {
            planes: [near_plane, far_plane, left_plane, right_plane, top_plane, bottom_plane],
        }
    }

    /// Test if an AABB is at least partially inside the frustum
    pub fn intersects_aabb(&self, min: Vec3, max: Vec3) -> bool {
        for plane in &self.planes {
            // Find the corner most in direction of plane normal (P-vertex)
            let p = Vec3::new(
                if plane.normal.x >= 0.0 { max.x } else { min.x },
                if plane.normal.y >= 0.0 { max.y } else { min.y },
                if plane.normal.z >= 0.0 { max.z } else { min.z },
            );

            // If the P-vertex is behind the plane, AABB is outside
            if plane.distance_to_point(p) < 0.0 {
                return false;
            }
        }
        true
    }

    /// Test if a chunk at the given position is visible
    pub fn is_chunk_visible(&self, chunk_pos: bevy::math::IVec3, chunk_size: i32) -> bool {
        let min = (chunk_pos * chunk_size).as_vec3();
        let max = min + Vec3::splat(chunk_size as f32);
        self.intersects_aabb(min, max)
    }
}
```

### Phase 2: Server-Side Frustum with Fixed Parameters — Optional Enhancement

**Skip client synchronization**. Use conservative fixed parameters on the server:

```rust
// In shared/src/frustum.rs or directly in broadcast_world.rs

/// Conservative frustum parameters that cover all reasonable client configurations.
/// Using 90° FOV (vs typical 60°) provides ~15° margin on each side.
/// Using 2.0 aspect ratio covers ultrawide monitors.
const SERVER_CULL_FOV: f32 = std::f32::consts::FRAC_PI_2; // 90°
const SERVER_CULL_ASPECT: f32 = 2.0;
```

This approach:
- Avoids all client→server synchronization complexity
- Handles players with different FOV settings (up to 90°)
- Handles different aspect ratios (up to 21:9 ultrawide)
- Trades ~5-10% potential bandwidth savings for zero sync complexity

**Only implement Phase 3 (client sync) if**:
- Players with >90° FOV report chunk pop-in
- Bandwidth savings from tighter culling are critically needed

### Phase 3: Client Synchronization — Only If Needed

<details>
<summary>Click to expand (not recommended for initial implementation)</summary>

**Modify**: `shared/src/players/data.rs`

Add frustum configuration to the Player struct:

```rust
#[derive(Component, Clone, Serialize, Deserialize, Debug)]
pub struct Player {
    // ... existing fields ...
    pub frustum_config: FrustumConfig,
}
```

**Modify**: `shared/src/messages/player.rs`

Add frustum config to the player input message:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInputAction {
    // ... existing fields ...
    pub frustum_config: Option<FrustumConfig>, // Send when changed
}
```

**Modify**: `client/src/camera/spawn.rs`

Store FOV as a resource for synchronization:

```rust
#[derive(Resource)]
pub struct CameraConfig {
    pub fov_radians: f32,
    pub aspect_ratio: f32,
}
```

**Create**: `client/src/network/frustum_sync.rs`

System to synchronize frustum config when it changes:

```rust
pub fn sync_frustum_config_system(
    camera_config: Res<CameraConfig>,
    windows: Query<&Window, With<PrimaryWindow>>,
    // ... network client ...
) {
    if camera_config.is_changed() || window_size_changed {
        // Send updated FrustumConfig to server
    }
}
```

</details>

### Phase 4: Server-Side Frustum Culling (server/)

**Modify**: `server/src/world/broadcast_world.rs`

Replace or augment the current prioritization with frustum culling:

```rust
use shared::frustum::ViewFrustum;

/// Filter chunks to only those visible in the player's frustum
fn get_frustum_visible_chunks(
    all_chunks: Vec<IVec3>,
    player: &Player,
    render_distance: i32,
) -> Vec<IVec3> {
    // Build frustum from player state
    let eye_position = player.position + Vec3::Y * 0.8; // Eye level offset
    let forward = player.camera_transform.forward().as_vec3();
    let up = Vec3::Y;

    let mut config = player.frustum_config.clone();
    config.far = (render_distance * CHUNK_SIZE) as f32;

    let frustum = ViewFrustum::from_camera(eye_position, forward, up, &config);

    // Filter chunks
    all_chunks
        .into_iter()
        .filter(|chunk_pos| frustum.is_chunk_visible(*chunk_pos, CHUNK_SIZE))
        .collect()
}

/// Updated chunk retrieval with frustum culling
fn get_player_chunks_prioritized_with_culling(
    player: &Player,
    radius: i32,
    max_chunks: usize,
) -> Vec<IVec3> {
    let player_chunk_pos = world_position_to_chunk_position(player.position);

    // Step 1: Get all chunks in render distance (spherical)
    let nearby_chunks = get_player_nearby_chunks_coords(player_chunk_pos, radius);

    // Step 2: Filter by frustum visibility
    let visible_chunks = get_frustum_visible_chunks(nearby_chunks, player, radius);

    // Step 3: Prioritize by distance and view direction
    let forward = player.camera_transform.forward();
    let mut chunks = visible_chunks;

    let sort_count = chunks.len().min(max_chunks);
    if chunks.len() > 1 {
        chunks.select_nth_unstable_by(sort_count - 1, |a, b| {
            order_chunks_by_render_score(a, b, player_chunk_pos, *forward)
        });
    }

    chunks
}
```

### Phase 5: Hybrid Mode (Recommended Initial Approach)

For initial implementation, use a **hybrid approach** that combines frustum culling with a small margin for chunks just outside the view:

```rust
/// Expanded frustum check with margin for player turning
fn is_chunk_visible_with_margin(
    frustum: &ViewFrustum,
    chunk_pos: IVec3,
    chunk_size: i32,
    margin_chunks: i32,
) -> bool {
    // First check exact frustum
    if frustum.is_chunk_visible(chunk_pos, chunk_size) {
        return true;
    }

    // Check expanded area for near-view chunks
    // This prevents pop-in when player turns quickly
    let expanded_min = (chunk_pos - IVec3::splat(margin_chunks)) * chunk_size;
    let expanded_max = ((chunk_pos + IVec3::splat(margin_chunks + 1)) * chunk_size);

    // Use a simplified cone check for margin area
    // ... implementation details ...

    false
}
```

### Phase 6: Client-Side Visibility Optimization — Not Needed

**Bevy handles this automatically.** Bevy's rendering pipeline performs frustum culling on all meshes with `Visibility` and `Aabb` components. No custom implementation is required.

The chunk meshes spawned by the client already benefit from Bevy's built-in culling. Adding a custom system would be redundant and potentially slower than Bevy's optimized implementation.

**Only consider custom client-side culling if**:
- Profiling shows Bevy's culling is a bottleneck
- You need culling at a coarser granularity (e.g., entire chunk columns)

## Implementation Order

### Stage 1: Cone Culling (Low Risk, High Value) ⭐ Start Here
1. Add `CULL_DOT_THRESHOLD` constant to `broadcast_world.rs`
2. Add `.filter()` to `get_player_chunks_prioritized()` (see Phase 0 code)
3. Test in single-player and multiplayer
4. Tune threshold if needed (-0.7 is a good starting point)
5. Measure bandwidth reduction

**Expected outcome**: ~30-40% reduction in chunks sent with minimal code changes.

### Stage 2: Full Frustum (Medium Risk, Only If Needed)
6. Create `shared/src/frustum.rs` with frustum types and AABB tests
7. Add unit tests for frustum-AABB intersection
8. Use fixed conservative parameters (90° FOV, 2.0 aspect ratio)
9. Create `get_frustum_visible_chunks()` helper function
10. Add feature flag to toggle frustum culling:
    ```rust
    const ENABLE_FRUSTUM_CULLING: bool = true;
    ```
11. Integrate into `get_player_chunks_prioritized()`

**Expected outcome**: ~50-60% reduction in chunks sent.

### Stage 3: Polish & Optimization
12. Add debug visualization (F-key toggle to show frustum wireframe)
13. Tune margin/buffer values to prevent pop-in
14. Profile and optimize frustum tests

### Stage 4: Client Sync (Low Priority)
15. Only implement if players with custom FOV >90° report issues
16. See collapsed Phase 3 section for implementation details

## Edge Cases & Considerations

### 1. Quick Player Rotation
**Problem**: Player turns quickly, chunks behind new view direction aren't loaded.

**Solution**: Use expanded frustum (add ~30° margin) or send extra chunks in a ring around frustum.

### 2. Vertical Look (Up/Down)
**Problem**: Looking straight up/down dramatically changes visible chunks.

**Solution**: Since Rustcraft uses 16×16×16 chunks (uniform cubes), frustum culling works identically in all directions. No special handling needed for vertical vs horizontal culling.

### 3. Multiplayer - Different FOV Settings
**Problem**: Each player may have different FOV settings.

**Solution**: Store `FrustumConfig` per-player on server; synchronize when changed.

### 4. Initial Spawn / Teleportation
**Problem**: Player spawns/teleports to new location; no chunks loaded.

**Solution**: Always include chunks in a small radius around the player regardless of frustum. This is simpler than tracking spawn state:

```rust
fn get_frustum_visible_chunks(
    all_chunks: Vec<IVec3>,
    player: &Player,
    player_chunk_pos: IVec3,
    render_distance: i32,
) -> Vec<IVec3> {
    let frustum = /* build frustum */;
    
    all_chunks
        .into_iter()
        .filter(|chunk_pos| {
            // Always include chunks within 2 chunks of player (spawn safety)
            let distance = (*chunk_pos - player_chunk_pos).abs();
            if distance.x <= 2 && distance.y <= 2 && distance.z <= 2 {
                return true;
            }
            // Otherwise apply frustum culling
            frustum.is_chunk_visible(*chunk_pos, CHUNK_SIZE)
        })
        .collect()
}
```

This ensures players always have chunks around them without needing to track connection time or chunk counts.

### 5. Already-Sent Chunks
**Problem**: `sent_to_clients` tracking assumes chunks stay relevant.

**Solution**: Frustum culling only affects *new* chunk sends. Already-sent chunks remain on client until:
- Client explicitly unloads them (render distance system)
- Server marks them as modified (handled by existing `chunks_to_update`)

## Performance Analysis

### Expected Benefits

| Metric | Before Frustum Culling | After Frustum Culling |
|--------|------------------------|----------------------|
| Chunks sent per tick | All in render distance | ~40-60% of render distance |
| Bandwidth usage | Higher | Reduced by ~40-60% |
| Server CPU (chunk selection) | O(n) | O(n) + frustum test overhead |
| Client mesh generation | All received chunks | Same (no change) |
| Client GPU rendering | Bevy frustum culling | Same (Bevy handles) |

### Frustum Test Performance

Each chunk requires 6 plane-AABB tests. For a render distance of 8:
- Spherical volume: ~2,145 chunks
- Frustum tests: ~12,870 plane comparisons

This is negligible compared to network I/O and mesh generation costs.

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_in_front_of_camera_visible() {
        let frustum = ViewFrustum::from_camera(
            Vec3::ZERO,
            Vec3::Z, // Looking forward (+Z)
            Vec3::Y,
            &FrustumConfig::default(),
        );

        // Chunk directly in front
        assert!(frustum.is_chunk_visible(IVec3::new(0, 0, 2), 16));
    }

    #[test]
    fn test_chunk_behind_camera_not_visible() {
        let frustum = ViewFrustum::from_camera(
            Vec3::ZERO,
            Vec3::Z,
            Vec3::Y,
            &FrustumConfig::default(),
        );

        // Chunk behind camera
        assert!(!frustum.is_chunk_visible(IVec3::new(0, 0, -5), 16));
    }

    #[test]
    fn test_chunk_at_edge_of_fov_visible() {
        // Test chunks at the boundary of the FOV
    }
}
```

### Integration Testing

1. Start server with frustum culling enabled
2. Connect client and verify chunks load correctly
3. Rotate player 360° and verify all chunks eventually load
4. Look up/down and verify vertical chunks load correctly
5. Teleport player and verify chunks at new location load

### Performance Testing

```bash
# Profile with flamegraph
cargo flamegraph --bin server -- --render_distance 16

# Compare chunk send rates with/without frustum culling
```

## Configuration Options

Add to `GameServerConfig`:

```rust
pub struct GameServerConfig {
    // ... existing fields ...
    pub enable_frustum_culling: bool,
    pub frustum_margin_degrees: f32, // Extra FOV margin
    pub initial_load_chunks: usize,   // Chunks to send before enabling culling
}
```

## Debug Visualization

Add F-key toggle to visualize frustum:

```rust
// client/src/ui/hud/debug/frustum.rs

pub fn draw_frustum_debug(
    mut gizmos: Gizmos,
    camera_query: Query<(&GlobalTransform, &Projection), With<Camera3d>>,
    debug_options: Res<DebugOptions>,
) {
    if !debug_options.show_frustum {
        return;
    }

    // Draw frustum wireframe using gizmos
    // ...
}
```

## Rollback Plan

If frustum culling causes issues:

1. Set `ENABLE_FRUSTUM_CULLING = false` on server
2. Revert to existing prioritization-only system
3. Analyze logs/metrics to identify issues

## Future Enhancements

### Occlusion Culling (Advanced)
After frustum culling, implement occlusion culling to skip chunks blocked by terrain:
- Use hierarchical Z-buffer
- Implement software rasterization for coarse occlusion

### Predictive Loading
Send chunks where player is *likely* to look:
- Track player rotation velocity
- Pre-load chunks in predicted view direction

### Adaptive Culling
Adjust culling aggressiveness based on:
- Network bandwidth availability
- Client FPS performance
- Server load

## References

- [Bevy Frustum Culling](https://bevyengine.org/learn/book/getting-started/rendering/)
- [Real-Time Rendering - Frustum Culling](http://www.lighthouse3d.com/tutorials/view-frustum-culling/)
- [Fast Extraction of Viewing Frustum Planes](https://www.gamedevs.org/uploads/fast-extraction-viewing-frustum-planes-from-world-view-projection-matrix.pdf)

---

## Summary

This plan provides a **staged approach** to implementing view-based chunk culling in Rustcraft:

### Recommended Path

1. **Stage 1 (Start Here)**: Add cone culling via dot-product threshold
   - ~10 lines of code changes
   - ~30-40% bandwidth reduction
   - 2-3 hours implementation time

2. **Stage 2 (If Needed)**: Full frustum culling with fixed server parameters
   - ~50-60% bandwidth reduction
   - 1-2 days implementation time
   - No client synchronization needed

### What to Skip

- **Client→Server frustum sync**: Use conservative fixed parameters instead
- **Client-side visibility system**: Bevy handles this automatically
- **Complex spawn tracking**: Use simple distance-based inclusion instead

### Key Advantages of 16×16×16 Chunks

Rustcraft's uniform cubic chunks simplify the implementation:
- Frustum culling works identically in all directions
- No special handling for vertical vs horizontal
- AABB tests are straightforward

The expected outcome is a **30-60% reduction in chunk network traffic** (depending on which stage you implement) while maintaining smooth gameplay through the hybrid margin approach and spawn safety radius.

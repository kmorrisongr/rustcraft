# Frustum Culling Implementation Plan

## Overview

This document outlines a plan to implement frustum culling for Rustcraft, enabling efficient chunk visibility management. The goal is to ensure only chunks within the player's field of view (FOV) are:
1. Transmitted from server to client (bandwidth optimization)
2. Rendered by the client (GPU optimization)

## Current Architecture Analysis

### Existing Chunk Prioritization

The server already has a sophisticated chunk prioritization system in [server/src/world/broadcast_world.rs](../server/src/world/broadcast_world.rs):

- **`get_chunk_render_score()`**: Calculates a priority score based on distance and view direction using dot product
- **`get_player_chunks_prioritized()`**: Returns chunks sorted by priority within render distance
- **`FORWARD_DOT_THRESHOLD`**: Currently set to `-0.3` (~108° from center), meaning chunks behind the player are deprioritized but still sent

**Limitation**: The current system *prioritizes* chunks in the player's view direction but does not *cull* chunks outside the actual camera frustum. All chunks within render distance are eventually sent.

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

### Phase 1: Shared Frustum Types (shared/)

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

### Phase 2: Player Data Extension (shared/)

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

Add frustum config to the player input message (if not already sending camera state):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInputAction {
    // ... existing fields ...
    pub frustum_config: Option<FrustumConfig>, // Send when changed
}
```

### Phase 3: Client-Side Changes (client/)

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

### Phase 6: Client-Side Visibility Optimization

**Modify**: `client/src/world/rendering/render.rs`

Add frustum-based visibility toggling for already-loaded chunks:

```rust
use bevy::render::primitives::Frustum;

pub fn chunk_visibility_system(
    camera_query: Query<(&GlobalTransform, &Projection), With<Camera3d>>,
    mut chunk_query: Query<(&Transform, &mut Visibility), With<ChunkMesh>>,
) {
    let Ok((camera_transform, projection)) = camera_query.single() else {
        return;
    };

    // Build frustum from camera
    let frustum = /* construct from camera_transform and projection */;

    for (chunk_transform, mut visibility) in chunk_query.iter_mut() {
        let chunk_min = chunk_transform.translation;
        let chunk_max = chunk_min + Vec3::splat(CHUNK_SIZE as f32);

        *visibility = if frustum.intersects_aabb(chunk_min, chunk_max) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}
```

**Note**: Bevy already performs frustum culling on meshes automatically. This system would be for any custom optimization beyond Bevy's built-in culling.

## Implementation Order

### Stage 1: Foundation (Low Risk)
1. Create `shared/src/frustum.rs` with frustum types and AABB tests
2. Add unit tests for frustum-AABB intersection
3. Add `FrustumConfig` to shared types

### Stage 2: Server Integration (Medium Risk)
4. Add `frustum_config` to `Player` struct
5. Create `get_frustum_visible_chunks()` helper function
6. Add feature flag to toggle frustum culling:
   ```rust
   const ENABLE_FRUSTUM_CULLING: bool = true;
   ```
7. Integrate into `get_player_chunks_prioritized()`

### Stage 3: Client Synchronization (Medium Risk)
8. Create `CameraConfig` resource on client
9. Synchronize frustum config when changed (window resize, FOV settings)
10. Update player input messages to include frustum config

### Stage 4: Polish & Optimization
11. Add debug visualization (F-key toggle to show frustum wireframe)
12. Tune margin/buffer values to prevent pop-in
13. Profile and optimize frustum tests

## Edge Cases & Considerations

### 1. Quick Player Rotation
**Problem**: Player turns quickly, chunks behind new view direction aren't loaded.

**Solution**: Use expanded frustum (add ~30° margin) or send extra chunks in a ring around frustum.

### 2. Vertical Look (Up/Down)
**Problem**: Looking straight up/down dramatically changes visible chunks.

**Solution**: Frustum naturally handles this; ensure Y-axis chunks are properly considered.

### 3. Multiplayer - Different FOV Settings
**Problem**: Each player may have different FOV settings.

**Solution**: Store `FrustumConfig` per-player on server; synchronize when changed.

### 4. Initial Spawn / Teleportation
**Problem**: Player spawns/teleports to new location; no chunks loaded.

**Solution**: For initial spawn, temporarily disable culling or use spherical loading until first chunks arrive.

```rust
fn get_world_map_chunks_to_send(...) {
    if player.chunks_received_count < INITIAL_LOAD_THRESHOLD {
        // Use spherical loading for initial spawn
        return get_player_chunks_prioritized(player, radius, chunk_limit);
    }

    // Use frustum culling for normal gameplay
    return get_player_chunks_prioritized_with_culling(player, radius, chunk_limit);
}
```

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

This plan provides a phased approach to implementing frustum culling in Rustcraft:

1. **Phase 1-2**: Create shared frustum types and integrate with player data
2. **Phase 3-4**: Client synchronization and server-side culling logic
3. **Phase 5**: Hybrid approach with margin for smooth experience
4. **Phase 6**: Client-side visibility optimization (optional, Bevy handles this)

The expected outcome is a **40-60% reduction in chunk network traffic** while maintaining smooth gameplay through careful handling of edge cases like player rotation and initial spawn.

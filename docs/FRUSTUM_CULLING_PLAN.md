# Frustum Culling Implementation Plan

**Goal**: Reduce bandwidth by only sending chunks within the player's field of view.

## Recommended Approach

| Stage | Approach | Bandwidth Reduction | Effort |
|-------|----------|--------------------|---------|
| 1 | Cone Culling (dot threshold) | ~30-40% | 2-3 hours |
| 2 | Full frustum (fixed params) | ~50-60% | 1-2 days |
| 3 | Full frustum (synced params) | ~55-65% | 3-4 days |

**Start with Stage 1.** The existing dot-product prioritization can be extended to perform hard culling with ~10 lines of code.

---

## Current Architecture

The server already prioritizes chunks by view direction in [broadcast_world.rs](../server/src/world/broadcast_world.rs):

- `get_chunk_render_score()` — scores chunks by distance + dot product with view direction
- `FORWARD_DOT_THRESHOLD = -0.3` — deprioritizes (but still sends) chunks behind player

**Limitation**: All chunks within render distance are eventually sent. The system *prioritizes* but doesn't *cull*.

## Frustum Culling Design

A view frustum is defined by 6 planes (near, far, left, right, top, bottom). A chunk is visible if any part of its AABB intersects the frustum.

**Key parameters** (already available on server):
- Camera position: `Player.position + Vec3::Y * 0.8`
- View direction: `Player.camera_transform.forward()`
- Far distance: `render_distance × CHUNK_SIZE`

**Not available** (would require sync): FOV, aspect ratio. Stage 2 uses conservative fixed values instead.

## Implementation

### Stage 1: Cone Culling

**File**: `server/src/world/broadcast_world.rs`

```rust
/// -0.7 ≈ 135° from forward. Chunks beyond this are culled entirely.
const CULL_DOT_THRESHOLD: f32 = -0.7;

fn get_player_chunks_prioritized(player: &Player, radius: i32, max_chunks: usize) -> Vec<IVec3> {
    let player_chunk_pos = world_position_to_chunk_position(player.position);
    let forward = player.camera_transform.forward();

    let mut chunks: Vec<IVec3> = get_player_nearby_chunks_coords(player_chunk_pos, radius)
        .into_iter()
        .filter(|chunk_pos| {
            let direction = (*chunk_pos - player_chunk_pos).as_vec3().normalize_or_zero();
            forward.dot(direction) > CULL_DOT_THRESHOLD
        })
        .collect();

    let sort_count = chunks.len().min(max_chunks);
    if chunks.len() > 1 {
        chunks.select_nth_unstable_by(sort_count - 1, |a, b| {
            order_chunks_by_render_score(a, b, player_chunk_pos, *forward)
        });
    }
    chunks
}
```

**To disable**: Set `CULL_DOT_THRESHOLD = -1.0`.

---

### Stage 2: Full Frustum Culling

<details>
<summary>Click to expand frustum implementation code</summary>

**Create**: `shared/src/frustum.rs`

```rust
use bevy::math::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Plane {
    pub normal: Vec3,
    pub d: f32,
}

impl Plane {
    pub fn from_normal_and_point(normal: Vec3, point: Vec3) -> Self {
        let normal = normal.normalize();
        Self { normal, d: -normal.dot(point) }
    }

    pub fn distance_to_point(&self, point: Vec3) -> f32 {
        self.normal.dot(point) + self.d
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewFrustum {
    pub planes: [Plane; 6],
}

impl ViewFrustum {
    pub fn from_camera(position: Vec3, forward: Vec3, up: Vec3, fov: f32, aspect: f32, far: f32) -> Self {
        let right_vec = forward.cross(up).normalize();
        let up = right_vec.cross(forward).normalize();
        let half_v = far * (fov / 2.0).tan();
        let half_h = half_v * aspect;
        let far_center = position + forward * far;

        let near_plane = Plane::from_normal_and_point(forward, position + forward * 0.1);
        let far_plane = Plane::from_normal_and_point(-forward, far_center);

        let ftl = far_center + up * half_v - right_vec * half_h;
        let ftr = far_center + up * half_v + right_vec * half_h;
        let fbl = far_center - up * half_v - right_vec * half_h;
        let fbr = far_center - up * half_v + right_vec * half_h;

        let left = Plane::from_normal_and_point((fbl - position).cross(ftl - position).normalize(), position);
        let right = Plane::from_normal_and_point((ftr - position).cross(fbr - position).normalize(), position);
        let top = Plane::from_normal_and_point((ftl - position).cross(ftr - position).normalize(), position);
        let bottom = Plane::from_normal_and_point((fbr - position).cross(fbl - position).normalize(), position);

        Self { planes: [near_plane, far_plane, left, right, top, bottom] }
    }

    pub fn intersects_aabb(&self, min: Vec3, max: Vec3) -> bool {
        for plane in &self.planes {
            let p = Vec3::new(
                if plane.normal.x >= 0.0 { max.x } else { min.x },
                if plane.normal.y >= 0.0 { max.y } else { min.y },
                if plane.normal.z >= 0.0 { max.z } else { min.z },
            );
            if plane.distance_to_point(p) < 0.0 { return false; }
        }
        true
    }

    pub fn is_chunk_visible(&self, chunk_pos: bevy::math::IVec3, chunk_size: i32) -> bool {
        let min = (chunk_pos * chunk_size).as_vec3();
        let max = min + Vec3::splat(chunk_size as f32);
        self.intersects_aabb(min, max)
    }
}
```

</details>

Use conservative fixed parameters (no client sync needed):

```rust
const SERVER_CULL_FOV: f32 = std::f32::consts::FRAC_PI_2; // 90° (conservative; client uses 60°)
const SERVER_CULL_ASPECT: f32 = 2.0; // Covers ultrawide
```

**Server integration** in `broadcast_world.rs`:

```rust
fn get_frustum_visible_chunks(chunks: Vec<IVec3>, player: &Player, render_distance: i32) -> Vec<IVec3> {
    let eye = player.position + Vec3::Y * 0.8;
    let forward = player.camera_transform.forward();
    let far = (render_distance * CHUNK_SIZE) as f32;
    
    let frustum = ViewFrustum::from_camera(eye, *forward, Vec3::Y, SERVER_CULL_FOV, SERVER_CULL_ASPECT, far);
    chunks.into_iter().filter(|pos| frustum.is_chunk_visible(*pos, CHUNK_SIZE)).collect()
}
```

### Stage 3: Client Sync (Low Priority)

Only implement if players with >90° FOV report pop-in. Requires adding `FrustumConfig` to `Player` struct and syncing on change.

---

**Note**: Bevy handles client-side frustum culling automatically. No custom client implementation needed.

## Checklist

### Stage 1: Cone Culling
- [ ] Add `CULL_DOT_THRESHOLD = -0.7` to `broadcast_world.rs`
- [ ] Add `.filter()` to `get_player_chunks_prioritized()`
- [ ] Test: verify no pop-in when turning
- [ ] Measure chunk send rate reduction

### Stage 2: Full Frustum (if Stage 1 insufficient)
- [ ] Create `shared/src/frustum.rs`
- [ ] Add unit tests for AABB intersection
- [ ] Integrate `get_frustum_visible_chunks()`
- [ ] Add `ENABLE_FRUSTUM_CULLING` toggle

## Edge Cases

| Case | Solution |
|------|----------|
| Quick rotation | Use ~30° margin beyond actual FOV |
| Vertical look | 16×16×16 chunks handle uniformly—no special case |
| Spawn/teleport | Always include 2-chunk radius around player |
| Different FOVs | Conservative 90° server-side covers most clients |

**Spawn safety** (add to frustum filter):
```rust
// Insert this at the beginning of the `.filter()` closure in the Stage 1 implementation
// (around line 53), before the normal culling check.
let distance = (*chunk_pos - player_chunk_pos).abs();
if distance.x <= 2 && distance.y <= 2 && distance.z <= 2 { return true; }
```

## Performance

Frustum test overhead is negligible (~12k simple comparisons for render distance 8) compared to network I/O and mesh generation.

## Testing

**Unit tests** (for Stage 2): Test `is_chunk_visible()` with chunks in front, behind, and at FOV edges.

**Integration**: Rotate 360°, look up/down, teleport—verify no missing chunks.

## Rollback

Set `CULL_DOT_THRESHOLD = -1.0` (Stage 1) or `ENABLE_FRUSTUM_CULLING = false` (Stage 2) to disable.

## Future Ideas

- **Occlusion culling**: Skip chunks blocked by terrain
- **Predictive loading**: Pre-send chunks in rotation direction
- **Adaptive culling**: Adjust aggressiveness based on bandwidth/FPS

---

## Summary

**Start with Stage 1** (cone culling). It's ~10 lines of code for ~30-40% bandwidth reduction.

Only proceed to Stage 2 (full frustum) if more savings are needed. Skip client sync—use fixed 90° FOV.

Rustcraft's 16x16x16 chunks make this straightforward: uniform cubes, no vertical/horizontal special cases.

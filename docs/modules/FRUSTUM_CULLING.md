# View Frustum Culling Implementation

## Overview

This implementation adds view frustum culling to Rustcraft's chunk rendering system, significantly improving rendering performance by hiding chunks outside the camera's view. The frustum culling works as a **visibility system** that runs every frame, controlling which chunk entities are rendered.

## Architecture

The frustum culling is implemented as a **separate visibility system** rather than filtering during mesh generation. This is important because:

1. **All chunks within render distance are meshed** - ensures chunks are ready when they come into view
2. **Visibility is updated every frame** - chunks are shown/hidden based on current camera view
3. **No pop-in artifacts** - chunks are already loaded, just hidden until needed

## Implementation Details

### 1. Frustum Extraction (Gribb-Hartmann Method)

Located in [client/src/world/rendering/frustum.rs](../../client/src/world/rendering/frustum.rs)

The frustum is extracted directly from the combined view-projection matrix using the **Gribb-Hartmann method**, as described in "Fast Extraction of Viewing Frustum Planes from the World-View-Projection Matrix" by Gil Gribb and Klaus Hartmann.

**Key features:**
- Extracts 6 frustum planes (left, right, top, bottom, near, far) from the view-projection matrix
- Each plane is **normalized** during construction to prevent floating-point precision issues
- Normalization is critical to avoid incorrect culling at screen edges

**The method works by combining rows of the view-projection matrix:**
```
Left plane:   row4 + row1
Right plane:  row4 - row1
Bottom plane: row4 + row2
Top plane:    row4 - row2
Near plane:   row4 + row3
Far plane:    row4 - row3
```

### 2. AABB-Frustum Intersection (P-vertex/N-vertex Optimization)

The implementation uses the **P-vertex/N-vertex approach** for efficient AABB-frustum intersection testing:

**P-vertex (Positive vertex):** The corner of the AABB that is furthest along the plane's normal direction.

**N-vertex (Negative vertex):** The opposite corner of the AABB.

**Optimization:** For culling purposes, we only need to test the P-vertex. If the P-vertex is on the negative side of any plane, the entire AABB is outside the frustum and can be culled. This avoids testing all 8 corners of the bounding box.

```rust
// For each plane, find the P-vertex based on the plane's normal direction
let p_vertex = Vec3::new(
    if plane.normal.x >= 0.0 { max.x } else { min.x },
    if plane.normal.y >= 0.0 { max.y } else { min.y },
    if plane.normal.z >= 0.0 { max.z } else { min.z },
);

// If P-vertex is outside, the entire AABB is outside
if plane.distance_to_point(p_vertex) < 0.0 {
    return false;
}
```

### 3. Floating-Point Precision Handling

To prevent precision issues with large world coordinates, chunk positions are handled using double precision:

```rust
pub fn intersects_chunk(&self, chunk_pos: IVec3, chunk_size: i32) -> bool {
    // Use f64 for intermediate calculations to avoid precision loss
    let chunk_world_x = chunk_pos.x as f64 * chunk_size as f64;
    let chunk_world_y = chunk_pos.y as f64 * chunk_size as f64;
    let chunk_world_z = chunk_pos.z as f64 * chunk_size as f64;
    
    // Convert to f32 only for final AABB test
    let min = Vec3::new(
        chunk_world_x as f32,
        chunk_world_y as f32,
        chunk_world_z as f32,
    );
    // ...
}
```

This approach:
- Uses double precision (`f64`) for chunk position multiplication
- Minimizes floating-point error accumulation
- Converts to `f32` only for the final intersection test

### 4. Integration into Rendering Pipeline

The frustum culling is integrated as a **visibility system** in [client/src/world/rendering/render.rs](../../client/src/world/rendering/render.rs):

**ChunkEntity Marker Component:**
```rust
/// Marker component for chunk entities to enable frustum culling visibility updates
#[derive(Component)]
pub struct ChunkEntity {
    pub chunk_pos: IVec3,
}
```

**Visibility System (runs every frame):**
```rust
pub fn frustum_cull_chunks_system(
    camera_query: Query<(&Transform, &Projection), With<Camera3d>>,
    mut chunk_query: Query<(&ChunkEntity, &mut Visibility)>,
) {
    // Get the camera transform and projection
    let Ok((camera_transform, projection)) = camera_query.single() else {
        return;
    };

    // Build view-projection matrix and extract frustum
    let view_matrix = camera_transform.compute_matrix().inverse();
    let projection_matrix = match projection { ... };
    let view_projection = projection_matrix * view_matrix;
    let frustum = Frustum::from_view_projection_matrix(&view_projection);

    // Update visibility for each chunk entity
    for (chunk_entity, mut visibility) in chunk_query.iter_mut() {
        let is_visible = frustum.intersects_chunk(chunk_entity.chunk_pos, CHUNK_SIZE);
        *visibility = if is_visible { Visibility::Visible } else { Visibility::Hidden };
    }
}
```

**Key Design Decisions:**
- Mesh generation is **not** affected by frustum culling - all chunks in render distance are meshed
- Visibility is toggled via Bevy's `Visibility` component - GPU efficiently skips hidden entities
- System runs in `PostUpdate` schedule alongside the render system

### 5. Debug Visualization (Optional)

A debug module is provided in [client/src/world/rendering/frustum_debug.rs](../../client/src/world/rendering/frustum_debug.rs) with:

- **F8 key:** Toggle frustum debug visualization
- **F9 key:** Toggle showing culled chunk boundaries
- Visual feedback with green wireframes for visible chunks, red for culled chunks
- Console logging of culling statistics (visible/culled/percentage)

**To enable debug visualization**, add to your game setup:
```rust
app.init_resource::<FrustumDebugSettings>();
app.add_systems(Update, (
    toggle_frustum_debug,
    display_frustum_stats,
));
```

## Benefits

1. **Performance:** Avoids submitting and rendering chunk meshes that are outside the camera view, reducing GPU work
2. **Scalability:** Performance improvement scales with view distance
3. **Precision:** Normalized planes and double-precision chunk positions prevent culling errors
4. **Efficiency:** P-vertex optimization minimizes per-chunk computation

## Testing

Unit tests are included in [frustum.rs](../../client/src/world/rendering/frustum.rs):

```bash
cargo test --bin client frustum
```

Tests verify:
- Plane normalization correctness
- AABB intersection detection
- Chunk-specific intersection with various positions

## Expected Performance Impact

- **Typical case:** 50-70% of chunks culled (depending on view direction and terrain)
- **Best case:** Up to 80-90% culling when looking at horizon/sky
- **Worst case:** Minimal culling when surrounded by terrain in all directions

The actual performance gain depends on:
- Render distance setting
- Player position (underground vs. surface)
- Camera direction (looking at terrain vs. sky)
- Terrain complexity

## Technical Considerations

### Why Gribb-Hartmann?

The Gribb-Hartmann method is preferred because:
- Direct extraction from view-projection matrix (no separate calculations)
- Computationally efficient (simple matrix row operations)
- Well-tested and widely used in graphics applications
- Naturally handles all projection types (perspective, orthographic, custom)

### Why P-vertex/N-vertex?

The P-vertex/N-vertex approach is optimal for culling because:
- Early rejection: Single vertex test per plane can reject entire AABB
- Cache-friendly: Minimal data accessed per test
- Branchless implementation possible: Improves SIMD optimization
- Suitable for batch processing: Can test many chunks efficiently

### Normalized Planes

Plane normalization is **critical** for correctness:
- Without normalization, distance calculations are scaled incorrectly
- Can cause false positives (culling visible chunks) or false negatives (rendering invisible chunks)
- Particularly important at screen edges where precision matters most

## Future Enhancements

Potential optimizations for future development:

1. **Hierarchical culling:** Test groups of chunks with bounding volumes
2. **Occlusion culling:** Cull chunks hidden behind other chunks
3. **Portal culling:** For underground/cave systems
4. **Temporal coherence:** Reuse culling results for static chunks across frames
5. **Multi-threading:** Parallelize frustum tests across chunks

## References

- **Gribb-Hartmann Method:** "Fast Extraction of Viewing Frustum Planes from the World-View-Projection Matrix"
- **P-vertex/N-vertex:** Described in Real-Time Rendering, 4th Edition (Akenine-Möller et al.)
- **Frustum Culling:** Game Programming Gems series, various volumes

## Code Locations

- **Core Implementation:** [client/src/world/rendering/frustum.rs](../../client/src/world/rendering/frustum.rs)
- **Integration:** [client/src/world/rendering/render.rs](../../client/src/world/rendering/render.rs)
- **Debug Tools:** [client/src/world/rendering/frustum_debug.rs](../../client/src/world/rendering/frustum_debug.rs)
- **Module Export:** [client/src/world/rendering/mod.rs](../../client/src/world/rendering/mod.rs)

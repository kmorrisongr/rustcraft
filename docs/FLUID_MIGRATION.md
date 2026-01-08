# Water Mesh to Fluid Simulation Migration

## Summary

This document describes the migration from a mesh-based water rendering system to a particle-based fluid simulation system using Salva3D.

## What Has Been Implemented

### 1. Fluid Simulation Infrastructure (`shared/src/fluid/`)

Three core modules have been created:

#### `config.rs` - Compile-Time Configuration
- **Constants for tuning performance and behavior:**
  - `PARTICLES_PER_WATER_BLOCK`: 27 particles (3x3x3 grid)
  - `PARTICLE_RADIUS`: 0.15 meters
  - `SMOOTHING_FACTOR`: 2.0 (SPH kernel radius multiplier)
  - `VISCOSITY`: 0.02 (water-like fluid behavior)
  - `REST_DENSITY`: 1000 kg/m³ (water density)
  - `FLUID_TIME_STEP`: 1/120 seconds
  - `MAX_FLUID_PARTICLES_WARNING`: 50,000 particles
  - `CHUNK_BARRIER_MARGIN`: 0.5 blocks

- **Runtime FluidConfig Resource:**
  - Enable/disable simulation
  - Toggle particle rendering
  - Debug barrier visualization
  - Particle render scale

####  `spawning.rs` - Fluid Particle Management
- **FluidWorld Resource:**
  - Manages Salva3D `LiquidWorld` with DFSPH solver
  - Tracks fluid handles per chunk
  - Spawns particles in 3x3x3 grid within each water block
  - Applies gravity (9.81 m/s²)
  - Steps simulation with configurable timestep

- **Key Methods:**
  - `spawn_fluids_for_chunk()`: Create fluid particles for water blocks
  - `remove_fluids_for_chunk()`: Clean up when chunks unload
  - `step()`: Advance simulation
  - `get_all_particle_positions()`: For rendering

#### `plugin.rs` - Bevy Integration
- **FluidPlugin:**
  - Initializes `FluidWorld` and `FluidConfig` resources
  - Runs `step_fluid_simulation` system in `FixedUpdate`
  - Ensures consistent physics timing

### 2. Water Block Property Updates (`shared/src/world/blocks.rs`)
- Water blocks maintain `Hitbox::Pathable` with `BlockHitbox::None`
- No collision or ray-hit detection
- Marked as `BlockTransparency::Liquid` for identification
- Water blocks serve only as spawn indicators for fluid particles

### 3. Client Rendering Changes

#### Removed:
- `WaterSurface` component
- `WaterEntities` resource  
- `WaterMaterialHandle` resource
- `generate_water_mesh_for_chunk()` function
- `water_render_system()` - mesh generation
- `water_cleanup_system()` - mesh cleanup

#### Added:
- `render_fluid_particles()` system
  - Currently uses debug gizmos (spheres) as placeholder
  - Renders up to 1000 particles for performance
  - TODO: Replace with efficient particle rendering (instancing, billboards)

#### Modified (`client/src/game.rs`):
- Added `FluidPlugin` to app
- Removed old water resource initialization
- Replaced water mesh systems with `render_fluid_particles`

## What Needs To Be Implemented

### 1. Chunk Integration (High Priority)
Currently, fluid particles are not spawned when chunks load. Need to:
- Hook into chunk loading events
- Call `FluidWorld::spawn_fluids_for_chunk()` when chunks with water blocks load
- Call `FluidWorld::remove_fluids_for_chunk()` when chunks unload
- Handle chunk updates (e.g., player breaking/placing water blocks)

**Suggested Implementation:**
```rust
// In client or server world systems
fn sync_fluids_with_chunks(
    mut fluid_world: ResMut<FluidWorld>,
    world_map: Res<WorldMap>,
    mut chunk_events: EventReader<ChunkLoadEvent>,
) {
    for event in chunk_events.read() {
        match event {
            ChunkLoadEvent::Loaded(chunk_pos) => {
                if let Some(chunk) = world_map.get(chunk_pos) {
                    fluid_world.spawn_fluids_for_chunk(&chunk.map, chunk_pos);
                }
            }
            ChunkLoadEvent::Unloaded(chunk_pos) => {
                fluid_world.remove_fluids_for_chunk(chunk_pos);
            }
        }
    }
}
```

### 2. Chunk Boundary Protection (Medium Priority)
Prevent fluid from spilling into ungenerated chunks:
- Create collision boundaries at chunk edges where adjacent chunks aren't loaded
- Use Salva's `Boundary` objects or collision system
- Dynamically add/remove barriers as chunks load/unload

**Approach:**
- Check adjacent chunks (6 faces: ±X, ±Y, ±Z)
- For each missing chunk, create a thin collision plane at boundary
- Store barriers in `FluidWorld` resource
- Remove barriers when adjacent chunk loads

### 3. Rapier-Salva Coupling (High Priority for Gameplay)
Currently using `()` (unit type) as coupling manager. Need to:
- Implement proper `CouplingManager` to connect Rapier physics with Salva fluids
- Make player and other Rapier bodies displace water
- Enable realistic water interactions (swimming, displacement)

**Reference:**
Look at `bevy_salva/src/rapier_integration.rs` for guidance on:
- Sampling Rapier colliders into Salva boundary
- Applying fluid forces back to Rapier bodies
- Syncing physics state between systems

### 4. Proper Particle Rendering (Medium Priority)
Replace debug gizmos with efficient rendering:
- **Option A:** Instanced mesh rendering
  - Create single sphere/icosphere mesh
  - Use instancing to render thousands efficiently
  - Update instance transforms each frame

- **Option B:** Billboard particles
  - Render as camera-facing quads
  - Cheaper than 3D meshes
  - Better for large particle counts

- **Option C:** Point sprites / Compute shader particles
  - Most efficient for very large counts
  - GPU-driven rendering
  - More complex implementation

### 5. Multiplayer Synchronization (Low Priority Initially)
- Decide: Server-authoritative or client-predicted fluids?
- Network protocol for fluid state
- Handle latency and synchronization

### 6. Performance Optimization
- Spatial partitioning for particle queries
- LOD system (fewer particles at distance)
- Particle pooling/culling
- Tune `PARTICLES_PER_WATER_BLOCK` based on profiling

### 7. Visual Polish
- Particle color/transparency based on depth
- Surface tension effects
- Foam/splash particles
- Integration with existing water shaders (optional)

## Testing Checklist

- [ ] Fluids spawn when chunks with water blocks load
- [ ] Fluids despawn when chunks unload
- [ ] No fluid spillage into ungenerated chunks
- [ ] Player can interact with water (displacement)
- [ ] Acceptable performance with >10,000 particles
- [ ] Multiplayer synchronization works correctly
- [ ] Visual quality meets standards

## Configuration Tuning Guide

### For Better Performance:
- Reduce `PARTICLES_PER_WATER_BLOCK` (e.g., 8 instead of 27)
- Increase `PARTICLE_RADIUS` (larger particles, fewer needed)
- Increase `FLUID_TIME_STEP` (less frequent simulation)
- Lower `MAX_FLUID_PARTICLES_WARNING` threshold

### For Better Quality:
- Increase `PARTICLES_PER_WATER_BLOCK` (e.g., 64)
- Decrease `PARTICLE_RADIUS` (finer detail)
- Decrease `FLUID_TIME_STEP` (more accurate simulation)
- Increase `SMOOTHING_FACTOR` (smoother fluid surface)

### For Different Fluid Types:
- **Honey/Lava:** Increase `VISCOSITY` (e.g., 0.5-1.0)
- **Thin fluids:** Decrease `VISCOSITY` (e.g., 0.01)
- **Heavy fluids:** Increase `REST_DENSITY` (e.g., 2000)
- **Light fluids:** Decrease `REST_DENSITY` (e.g., 500)

## Migration Notes

### Breaking Changes:
- Water no longer renders as meshes
- Water blocks have no collision (intentional)
- `WaterEntities`, `WaterMaterialHandle` resources removed
- `WaterSurface` component removed

### Backwards Compatibility:
- `BlockId::Water` still exists and generates
- Water generation in world gen unchanged
- Water shader infrastructure still present (can be removed if desired)

## Next Steps

1. **Immediate:** Implement chunk integration to actually spawn fluids
2. **Short-term:** Add chunk boundary protection
3. **Medium-term:** Implement Rapier-Salva coupling for gameplay
4. **Long-term:** Optimize particle rendering and add visual polish

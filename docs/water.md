Water System Implementation Summary (Conceptual Plan)

1. Core Principle

Model water as conserved volume, not blocks or particles.
	•	Water is discrete in storage, continuous in behavior.
	•	Simulation is local, bounded, and conservative.
	•	Rendering is decoupled from simulation.

⸻

2. Data Representation

World Storage
	•	Water exists per voxel, but is not voxel-shaped.
	•	Only store water where it exists (sparse).

Each water cell stores:
	•	volume (scalar)
	•	surface height (relative to cell bottom, derived)

Buckets / Items
	•	Store water as volume.
	•	Picking up / placing water transfers volume between item and world.

⸻

3. Spatial Organization

Chunks
	•	16×16×16 chunks remain unchanged.
	•	Water simulation is chunk-local.
	•	Neighbor chunk access is required for boundary flow.

Water Regions
	•	Group connected water cells into water bodies.
	•	Regions are recomputed locally when terrain or water changes.
	•	Regions allow simulation to be surface-based instead of fully 3D.

⸻

4. Surface Identification

Within a water region:
	•	A water cell is part of a surface if the voxel above is air.
	•	Connected surface cells form surface patches.
	•	Each patch is approximately horizontal and treated as a local heightfield.

Multiple surface patches can exist in the same XZ column at different Y levels.

⸻

5. Simulation Model

Lateral Flow (Waves & Spread)
	•	Use a shallow-water-style solver on each surface patch.
	•	Flow is driven by surface height differences.
	•	Simulation operates in 2D per patch (cheap, stable).

Vertical Flow (Waterfalls / Drains)
	•	Handled separately and event-driven:
	•	If air exists below a water cell, spill volume downward.
	•	Triggered by terrain edits or volume changes.
	•	No continuous 3D fluid simulation.

⸻

6. Terrain Interaction

> **Implementation**: `server/src/world/terrain_mutation.rs`

Removing Blocks
	•	Water volume becomes unstable.
	•	Attempt downward flow first (water from above flows into vacated space).
	•	Lateral neighbors are queued for potential inflow.
	•	Recompute affected surface patches and queue for simulation.

Adding Blocks
	•	Displace overlapping water volume using a 3-tier strategy:
		1.	Push water upward first (if air above) - most natural behavior
		2.	Distribute remaining water to lateral neighbors proportionally
		3.	Force overflow upward under pressure if needed
	•	Volume is conserved - only lost in extreme edge cases with no available space
	•	Surface patches are automatically recomputed

All updates are local and incremental. The `handle_block_removal` and `handle_block_placement` 
functions queue positions for the simulation system rather than processing immediately.

⸻

7. Chunk Boundaries
	•	Water does not ignore chunk edges.
	•	Exchange boundary data with neighboring chunks (ghost cells or lookups).
	•	Only active chunks simulate.
	•	Inactive chunks can “sleep” their water state.

⸻

8. Rendering

> **Implementation**: `client/src/world/rendering/water.rs`, `client/src/world/rendering/water_mesh.rs`, `client/src/world/rendering/water_material.rs`, `data/shaders/water.wgsl`

Mesh Generation
	•	Generate meshes only from water cells with exposed surfaces.
	•	Vertex heights come from simulated surface height (volume-based).
	•	Top faces rendered when air above; side faces when air adjacent.
	•	UV coordinates based on world position for consistent wave patterns.

Gerstner Wave Shader
	•	Four wave layers with configurable direction, steepness, and wavelength.
	•	Vertex displacement in shader for smooth animation.
	•	Fresnel-based reflectivity for realistic water appearance.
	•	Depth-based color blending (deep blue to shallow turquoise).

LOD System
	•	Full detail waves (4 layers) at close range.
	•	Reduced wave layers (2) at medium distance.
	•	Flat surfaces at far distance (simplified mesh).
	•	Toggle with F9 key for debugging.

Rendering never affects simulation state.

⸻

9. Performance Strategy
	•	Sparse storage: most voxels contain no water.
	•	Simulation cost scales with surface area, not volume.
	•	Vertical flow only occurs when topology changes.
	•	Water bodies and surface patches can sleep when stable.

⸻

10. Implementation Order (Recommended)
	1.	✅ Volume-per-voxel water storage (`shared/src/world/water.rs`)
	2.	✅ Downward-only flow (`server/src/world/water_simulation.rs`)
	3.	✅ Surface detection (`shared/src/world/water_surface.rs`)
	4.	✅ Lateral shallow-water simulation (`server/src/world/water_flow.rs`)
	5.	✅ Chunk boundary exchange (`server/src/world/water_boundary.rs`)
	6.	✅ Terrain mutation handling (`server/src/world/terrain_mutation.rs`)
	7.	✅ Visual wave rendering (`client/src/world/rendering/water*.rs`, `data/shaders/water.wgsl`)
	8.	Optimization & sleeping

⸻

11. Mental Model to Keep

Water is “stored” in 3D, “moves” mostly in 2D, and “looks” continuous.

Keeping those layers separate is what makes the system tractable, stable, and scalable.

⸻

If you want, next we can:
	•	formalize surface patch detection
	•	talk numerical stability and timesteps
	•	design the water-region bookkeeping
	•	compare this directly to Minecraft’s internal water logic

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

Removing Blocks
	•	Water volume becomes unstable.
	•	Attempt downward flow first.
	•	Otherwise spread laterally within the region.
	•	Recompute affected surface patches.

Adding Blocks
	•	Displace overlapping water volume.
	•	Push volume to neighbors.
	•	Potentially split a water region.

All updates are local and incremental.

⸻

7. Chunk Boundaries
	•	Water does not ignore chunk edges.
	•	Exchange boundary data with neighboring chunks (ghost cells or lookups).
	•	Only active chunks simulate.
	•	Inactive chunks can “sleep” their water state.

⸻

8. Rendering
	•	Generate meshes only from surface patches.
	•	Vertex heights come from simulated surface height.
	•	Apply GPU wave shaders (noise, Gerstner waves, etc.) for detail.
	•	LOD:
	•	Distant water collapses to flat surfaces.
	•	Simulation disabled beyond a radius.

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
	5.	Chunk boundary exchange
	6.	Terrain mutation handling
	7.	Visual wave rendering
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

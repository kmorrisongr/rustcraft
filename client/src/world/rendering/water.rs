//! Dedicated water rendering system.
//!
//! This module handles water rendering independently from chunk meshing.
//! Water entities are top-level (not chunk children), making it easier to
//! transition to physics-based water in the future.
//!
//! ## Performance Optimizations
//! - Mesh handles are reused and updated in-place to avoid GPU resource churn
//! - HashMap allocations are pooled using `Local<>` to avoid per-frame heap allocations
//! - Early returns prevent work when no updates are pending
//!
//! ## Future Migration Path
//! When implementing physics-based water:
//! 1. Replace `WaterSurface` mesh generation with physics simulation output
//! 2. Add `WaterVolume` component for physics interactions (bucket, player effects)
//! 3. Water blocks (`BlockId::Water`) can remain for placement/removal via bucket

use bevy::{
    color::LinearRgba,
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
};
use std::collections::{hash_map::Entry, HashMap, HashSet};

use crate::shaders::water::{StandardWaterMaterial, WaterMaterial, WaterMesh};
use crate::world::{ClientWorldMap, WorldRenderRequestUpdateEvent};
use crate::GameState;
use bevy::pbr::{ExtendedMaterial, NotShadowCaster, NotShadowReceiver};
use shared::world::{to_global_pos, BlockId, WorldMap};
use shared::CHUNK_SIZE;

/// Marker component for water surface entities.
/// Each water surface corresponds to water within a specific chunk.
///
/// ## Future Use
/// When implementing physics-based water, this component can be extended with:
/// - `chunk_pos`: For cleanup when chunks unload (currently tracked in WaterEntities)
/// - Flow direction vectors for river currents
/// - Volume data for player interactions (swimming, bucket filling)
#[derive(Component)]
pub struct WaterSurface;

/// Resource tracking which chunks have water entities spawned.
/// Used to avoid duplicate spawning and to clean up when chunks change.
#[derive(Resource, Default)]
pub struct WaterEntities {
    /// Maps chunk position to the water entity and its mesh handle for that chunk
    pub entities: HashMap<IVec3, WaterEntityData>,
}

/// Data stored for each water entity, enabling in-place mesh updates.
pub struct WaterEntityData {
    pub entity: Entity,
    pub mesh_handle: Handle<Mesh>,
}

/// Resource to store water material handle.
/// Using a single material with ocean amplitude for seamless cross-chunk rendering.
#[derive(Resource, Default)]
pub struct WaterMaterialHandle {
    pub handle: Option<Handle<StandardWaterMaterial>>,
}

impl WaterMaterialHandle {
    pub fn get(&self) -> Handle<StandardWaterMaterial> {
        self.handle
            .clone()
            .expect("Water material should be initialized before use")
    }

    pub fn is_initialized(&self) -> bool {
        self.handle.is_some()
    }
}

/// Create the water material with ocean-level amplitude.
/// Using consistent amplitude across all water for seamless chunk boundaries.
fn create_water_material(
    materials: &mut Assets<StandardWaterMaterial>,
) -> Handle<StandardWaterMaterial> {
    materials.add(ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::srgba(0.1, 0.3, 0.5, 0.8),
            alpha_mode: AlphaMode::Blend,
            ..default()
        },
        extension: WaterMaterial {
            amplitude: 0.5, // Ocean amplitude for consistent cross-chunk waves
            clarity: 0.3,
            deep_color: LinearRgba::new(0.05, 0.15, 0.25, 0.9),
            shallow_color: LinearRgba::new(0.15, 0.35, 0.45, 0.75),
            edge_color: LinearRgba::new(0.8, 0.9, 1.0, 0.5),
            edge_scale: 0.1,
            coord_scale: Vec2::new(1.0, 1.0),
            coord_offset: Vec2::ZERO,
        },
    })
}

/// Pooled allocations for water mesh generation to avoid per-frame heap allocations.
#[derive(Default)]
pub struct WaterMeshGenPool {
    /// Reusable storage for water surface positions grouped by Y level
    water_surfaces: HashMap<i32, HashSet<(i32, i32)>>,
    /// Reusable storage for vertex index mapping
    vertex_index_map: HashMap<(i32, i32), u32>,
}

/// Generates a continuous water surface mesh for a chunk.
/// Vertices are shared between adjacent water blocks to prevent gaps during wave animation.
/// Uses pooled allocations to avoid per-call heap allocations.
fn generate_water_mesh_for_chunk(
    world_map: &ClientWorldMap,
    chunk_pos: &IVec3,
    pool: &mut WaterMeshGenPool,
) -> Option<Mesh> {
    let chunk = world_map.map.get(chunk_pos)?;

    // Clear and reuse pooled water_surfaces
    pool.water_surfaces.clear();

    for (local_block_pos, block) in chunk.map.iter() {
        if block.id != BlockId::Water {
            continue;
        }

        let global_block_pos = to_global_pos(chunk_pos, local_block_pos);

        // Check if there's air above (this is a surface water block)
        let above_pos = global_block_pos + IVec3::new(0, 1, 0);
        if world_map.get_block_by_coordinates(&above_pos).is_some() {
            continue;
        }

        pool.water_surfaces
            .entry(local_block_pos.y)
            .or_default()
            .insert((local_block_pos.x, local_block_pos.z));
    }

    if pool.water_surfaces.is_empty() {
        return None;
    }

    let total_blocks: usize = pool.water_surfaces.values().map(|s| s.len()).sum();

    // Pre-allocate vectors (these are consumed by the mesh, so can't be pooled)
    // Note: We don't use vertex colors - the water material/shader handles coloring
    let mut vertices: Vec<[f32; 3]> = Vec::with_capacity(total_blocks * 2);
    let mut indices: Vec<u32> = Vec::with_capacity(total_blocks * 6);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(total_blocks * 2);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(total_blocks * 2);

    let water_surface_offset = 0.875; // 14/16 of a block

    for (y_level, xz_positions) in pool.water_surfaces.iter() {
        let y = *y_level as f32 + water_surface_offset;

        // Clear and reuse pooled vertex_index_map
        pool.vertex_index_map.clear();

        for (block_x, block_z) in xz_positions.iter() {
            let corners = [
                (*block_x, *block_z),
                (*block_x + 1, *block_z),
                (*block_x, *block_z + 1),
                (*block_x + 1, *block_z + 1),
            ];

            for &(cx, cz) in corners.iter() {
                // Use Entry API to avoid double HashMap lookup
                if let Entry::Vacant(entry) = pool.vertex_index_map.entry((cx, cz)) {
                    let vertex_idx = vertices.len() as u32;
                    entry.insert(vertex_idx);

                    vertices.push([cx as f32, y, cz as f32]);
                    normals.push([0.0, 1.0, 0.0]);

                    let world_x = (chunk_pos.x * CHUNK_SIZE + cx) as f32;
                    let world_z = (chunk_pos.z * CHUNK_SIZE + cz) as f32;
                    uvs.push([world_x, world_z]);
                }
            }

            let bl = pool.vertex_index_map[&(*block_x, *block_z)];
            let br = pool.vertex_index_map[&(*block_x + 1, *block_z)];
            let tl = pool.vertex_index_map[&(*block_x, *block_z + 1)];
            let tr = pool.vertex_index_map[&(*block_x + 1, *block_z + 1)];

            indices.extend_from_slice(&[bl, tl, tr, bl, tr, br]);
        }
    }

    if vertices.is_empty() {
        return None;
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, Default::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));

    if let Err(e) = mesh.generate_tangents() {
        warn!("Error generating tangents for water mesh: {:?}", e);
    }

    Some(mesh)
}

/// System that listens for chunk updates and regenerates water meshes.
/// Water entities are independent from chunk entities.
///
/// Performance optimizations:
/// - Uses `Local<WaterMeshGenPool>` to avoid per-frame HashMap allocations
/// - Reuses mesh handles by updating existing Assets instead of creating new ones
/// - Early return when no events to process
pub fn water_render_system(
    mut commands: Commands,
    world_map: Res<ClientWorldMap>,
    mut water_entities: ResMut<WaterEntities>,
    mut water_material: ResMut<WaterMaterialHandle>,
    mut materials: ResMut<Assets<StandardWaterMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ev_chunk_update: EventReader<WorldRenderRequestUpdateEvent>,
    mut mesh_pool: Local<WaterMeshGenPool>,
    mut chunks_to_update: Local<Vec<IVec3>>,
) {
    // Initialize water material if needed
    if !water_material.is_initialized() {
        water_material.handle = Some(create_water_material(&mut materials));
    }

    // Early return if no events - avoid any allocations
    if ev_chunk_update.is_empty() {
        return;
    }

    // Collect and deduplicate chunks that need water updates
    chunks_to_update.clear();
    chunks_to_update.extend(ev_chunk_update.read().map(|ev| {
        let WorldRenderRequestUpdateEvent::ChunkToReload(pos) = ev;
        *pos
    }));
    // Deduplicate using sort + dedup (IVec3 doesn't impl Ord, so convert to tuples)
    chunks_to_update.sort_by_key(|v| (v.x, v.y, v.z));
    chunks_to_update.dedup();

    for chunk_pos in chunks_to_update.iter().copied() {
        // Check if we have an existing entity for this chunk
        if let Some(existing_data) = water_entities.entities.get(&chunk_pos) {
            // Try to update existing mesh in-place
            if let Some(water_mesh) =
                generate_water_mesh_for_chunk(&world_map, &chunk_pos, &mut mesh_pool)
            {
                // Update existing mesh asset in-place (avoids GPU resource churn)
                if let Some(mesh_asset) = meshes.get_mut(&existing_data.mesh_handle) {
                    *mesh_asset = water_mesh;
                    continue;
                }
            } else {
                // No water in this chunk anymore, despawn the entity
                commands.entity(existing_data.entity).despawn();
                water_entities.entities.remove(&chunk_pos);
                continue;
            }
        }

        // Remove stale entry if entity update failed
        if let Some(existing_data) = water_entities.entities.remove(&chunk_pos) {
            commands.entity(existing_data.entity).despawn();
        }

        // Generate new water mesh
        if let Some(water_mesh) =
            generate_water_mesh_for_chunk(&world_map, &chunk_pos, &mut mesh_pool)
        {
            let transform = Transform::from_xyz(
                (chunk_pos.x * CHUNK_SIZE) as f32,
                (chunk_pos.y * CHUNK_SIZE) as f32,
                (chunk_pos.z * CHUNK_SIZE) as f32,
            );

            // Create a new mesh handle that we can track for future updates
            let mesh_handle = meshes.add(water_mesh);

            let entity = commands
                .spawn((
                    StateScoped(GameState::Game),
                    transform,
                    Visibility::Visible,
                    Mesh3d(mesh_handle.clone()),
                    MeshMaterial3d(water_material.get()),
                    WaterMesh,
                    WaterSurface,
                    NotShadowCaster,
                    NotShadowReceiver,
                ))
                .id();

            water_entities.entities.insert(
                chunk_pos,
                WaterEntityData {
                    entity,
                    mesh_handle,
                },
            );
        }
    }
}

/// System to clean up water entities when their chunks are unloaded.
/// Only runs when ClientWorldMap has changed, avoiding unnecessary iteration.
pub fn water_cleanup_system(
    mut commands: Commands,
    world_map: Res<ClientWorldMap>,
    mut water_entities: ResMut<WaterEntities>,
    mut chunks_to_remove: Local<Vec<IVec3>>,
) {
    // Only check for cleanup when the world map has actually changed
    if !world_map.is_changed() {
        return;
    }

    // Clear and reuse pooled Vec
    chunks_to_remove.clear();

    // Find water entities whose chunks no longer exist
    chunks_to_remove.extend(
        water_entities
            .entities
            .keys()
            .filter(|pos| !world_map.map.contains_key(pos))
            .copied(),
    );

    for chunk_pos in chunks_to_remove.iter() {
        if let Some(data) = water_entities.entities.remove(chunk_pos) {
            commands.entity(data.entity).despawn();
        }
    }
}

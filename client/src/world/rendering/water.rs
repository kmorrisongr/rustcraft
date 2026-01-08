//! Dedicated water rendering system.
//!
//! This module handles water rendering independently from chunk meshing.
//! Water entities are top-level (not chunk children), making it easier to
//! transition to physics-based water in the future.
//!
//! ## Future Migration Path
//! When implementing physics-based water:
//! 1. Replace `WaterSurface` mesh generation with physics simulation output
//! 2. Add `WaterVolume` component for physics interactions (bucket, player effects)
//! 3. Water blocks (`BlockId::Water`) can remain for placement/removal via bucket

use bevy::{
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
};
use std::collections::{HashMap, HashSet};

use crate::shaders::water::{StandardWaterMaterial, WaterMaterial, WaterMesh};
use crate::world::{ClientWorldMap, WorldRenderRequestUpdateEvent};
use crate::GameState;
use bevy::pbr::{ExtendedMaterial, NotShadowCaster, NotShadowReceiver};
use shared::water_physics::GerstnerWaveSystem;
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
    /// Maps chunk position to the water entity for that chunk
    pub entities: HashMap<IVec3, Entity>,
}

/// Resource to store water material handle.
/// Using a single material with ocean amplitude for seamless cross-chunk rendering.
#[derive(Resource, Default)]
pub struct WaterMaterialHandle {
    pub handle: Option<Handle<StandardWaterMaterial>>,
}

/// Resource containing the Gerstner wave system for water physics and rendering.
#[derive(Resource)]
pub struct WaterWaveSystem {
    pub gerstner: GerstnerWaveSystem,
    /// Time offset for wave animation
    pub time: f32,
}

impl Default for WaterWaveSystem {
    fn default() -> Self {
        Self {
            gerstner: GerstnerWaveSystem::ocean_waves(0.0),
            time: 0.0,
        }
    }
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
            deep_color: Color::srgba(0.05, 0.15, 0.25, 0.9),
            shallow_color: Color::srgba(0.15, 0.35, 0.45, 0.75),
            edge_color: Color::srgba(0.8, 0.9, 1.0, 0.5),
            edge_scale: 0.1,
            coord_scale: Vec2::new(1.0, 1.0),
            coord_offset: Vec2::ZERO,
            ..default()
        },
    })
}

/// Generates a continuous water surface mesh for a chunk.
/// Vertices are shared between adjacent water blocks to prevent gaps during wave animation.
fn generate_water_mesh_for_chunk(world_map: &ClientWorldMap, chunk_pos: &IVec3) -> Option<Mesh> {
    let chunk = world_map.map.get(chunk_pos)?;

    // Collect water surface positions grouped by Y level
    let mut water_surfaces: HashMap<i32, HashSet<(i32, i32)>> = HashMap::new();

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

        water_surfaces
            .entry(local_block_pos.y)
            .or_default()
            .insert((local_block_pos.x, local_block_pos.z));
    }

    if water_surfaces.is_empty() {
        return None;
    }

    let total_blocks: usize = water_surfaces.values().map(|s| s.len()).sum();

    // Pre-allocate vectors
    let mut vertices: Vec<[f32; 3]> = Vec::with_capacity(total_blocks * 2);
    let mut indices: Vec<u32> = Vec::with_capacity(total_blocks * 6);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(total_blocks * 2);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(total_blocks * 2);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(total_blocks * 2);

    let water_surface_offset = 0.875; // 14/16 of a block

    for (y_level, xz_positions) in water_surfaces.iter() {
        let y = *y_level as f32 + water_surface_offset;
        let mut vertex_index_map: HashMap<(i32, i32), u32> = HashMap::new();

        for (block_x, block_z) in xz_positions.iter() {
            let corners = [
                (*block_x, *block_z),
                (*block_x + 1, *block_z),
                (*block_x, *block_z + 1),
                (*block_x + 1, *block_z + 1),
            ];

            for (cx, cz) in corners.iter() {
                if !vertex_index_map.contains_key(&(*cx, *cz)) {
                    let vertex_idx = vertices.len() as u32;
                    vertex_index_map.insert((*cx, *cz), vertex_idx);

                    vertices.push([*cx as f32, y, *cz as f32]);
                    normals.push([0.0, 1.0, 0.0]);

                    let world_x = (chunk_pos.x * CHUNK_SIZE + *cx) as f32;
                    let world_z = (chunk_pos.z * CHUNK_SIZE + *cz) as f32;
                    uvs.push([world_x, world_z]);

                    colors.push([1.0, 1.0, 1.0, 0.7]);
                }
            }

            let bl = vertex_index_map[&(*block_x, *block_z)];
            let br = vertex_index_map[&(*block_x + 1, *block_z)];
            let tl = vertex_index_map[&(*block_x, *block_z + 1)];
            let tr = vertex_index_map[&(*block_x + 1, *block_z + 1)];

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
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));

    if let Err(e) = mesh.generate_tangents() {
        warn!("Error generating tangents for water mesh: {:?}", e);
    }

    Some(mesh)
}

/// System that listens for chunk updates and regenerates water meshes.
/// Water entities are independent from chunk entities.
pub fn water_render_system(
    mut commands: Commands,
    world_map: Res<ClientWorldMap>,
    mut water_entities: ResMut<WaterEntities>,
    mut water_material: ResMut<WaterMaterialHandle>,
    mut materials: ResMut<Assets<StandardWaterMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ev_chunk_update: EventReader<WorldRenderRequestUpdateEvent>,
    _wave_system: Res<WaterWaveSystem>, // Reserved for future mesh animation
) {
    // Initialize water material if needed
    if !water_material.is_initialized() {
        water_material.handle = Some(create_water_material(&mut materials));
    }

    // Collect chunks that need water updates
    let chunks_to_update: HashSet<IVec3> = ev_chunk_update
        .read()
        .map(|ev| {
            let WorldRenderRequestUpdateEvent::ChunkToReload(pos) = ev;
            *pos
        })
        .collect();

    for chunk_pos in chunks_to_update {
        // Despawn existing water entity for this chunk if any
        if let Some(entity) = water_entities.entities.remove(&chunk_pos) {
            commands.entity(entity).despawn();
        }

        // Generate new water mesh with wave displacement
        if let Some(water_mesh) = generate_water_mesh_for_chunk(&world_map, &chunk_pos) {
            let transform = Transform::from_xyz(
                (chunk_pos.x * CHUNK_SIZE) as f32,
                (chunk_pos.y * CHUNK_SIZE) as f32,
                (chunk_pos.z * CHUNK_SIZE) as f32,
            );

            let entity = commands
                .spawn((
                    StateScoped(GameState::Game),
                    transform,
                    Visibility::Visible,
                    Mesh3d(meshes.add(water_mesh)),
                    MeshMaterial3d(water_material.get()),
                    WaterMesh,
                    WaterSurface,
                    NotShadowCaster,
                    NotShadowReceiver,
                ))
                .id();

            water_entities.entities.insert(chunk_pos, entity);
        }
    }
}

/// System to clean up water entities when their chunks are unloaded.
pub fn water_cleanup_system(
    mut commands: Commands,
    world_map: Res<ClientWorldMap>,
    mut water_entities: ResMut<WaterEntities>,
) {
    // Find water entities whose chunks no longer exist
    let chunks_to_remove: Vec<IVec3> = water_entities
        .entities
        .keys()
        .filter(|pos| !world_map.map.contains_key(pos))
        .copied()
        .collect();

    for chunk_pos in chunks_to_remove {
        if let Some(entity) = water_entities.entities.remove(&chunk_pos) {
            commands.entity(entity).despawn();
        }
    }
}

/// System to update water wave animation time.
/// Uses Bevy's time system for smooth animation.
pub fn water_wave_update_system(time: Res<Time>, mut wave_system: ResMut<WaterWaveSystem>) {
    // Update wave time based on elapsed time
    // Use total elapsed time for continuous wave animation
    wave_system.time = time.elapsed_secs();
}

//! Debug visualization for the volume-based water system.
//!
//! This module provides a simple mesh-based debug view of water volumes.
//! Toggle with F8 (ToggleWaterDebugMode) to see water cells rendered as
//! semi-transparent blue cubes scaled by their volume.

use bevy::{
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
};
use std::collections::HashMap;

use crate::world::{ClientWorldMap, WorldRenderRequestUpdateEvent};
use crate::GameState;
use shared::world::FULL_WATER_HEIGHT;
use shared::CHUNK_SIZE;

/// Marker component for water debug mesh entities
#[derive(Component)]
pub struct WaterDebugMesh;

/// Resource to track water debug entities per chunk
#[derive(Resource, Default)]
pub struct WaterDebugEntities {
    pub entities: HashMap<IVec3, Entity>,
}

/// Resource to control water debug visibility
#[derive(Resource)]
pub struct WaterDebugSettings {
    pub enabled: bool,
}

impl Default for WaterDebugSettings {
    fn default() -> Self {
        Self { enabled: false }
    }
}

impl WaterDebugSettings {
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        info!(
            "Water debug mode: {}",
            if self.enabled { "ON" } else { "OFF" }
        );
    }
}

/// Material handle for water debug rendering
#[derive(Resource, Default)]
pub struct WaterDebugMaterial {
    pub handle: Option<Handle<StandardMaterial>>,
}

/// Creates a simple cube mesh for a water cell
fn create_water_cell_mesh(
    volume: f32,
    local_pos: IVec3,
) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<u32>) {
    let height = volume * FULL_WATER_HEIGHT;

    // Base position (local within chunk)
    let x = local_pos.x as f32;
    let y = local_pos.y as f32;
    let z = local_pos.z as f32;

    // Slightly inset to avoid z-fighting with blocks
    let inset = 0.01;
    let x0 = x + inset;
    let x1 = x + 1.0 - inset;
    let y0 = y + inset;
    let y1 = y + height - inset;
    let z0 = z + inset;
    let z1 = z + 1.0 - inset;

    // If water is too small, don't render
    if height < 0.02 {
        return (vec![], vec![], vec![]);
    }

    // 8 vertices of a cube
    let vertices = vec![
        // Bottom face (y = y0)
        [x0, y0, z0], // 0
        [x1, y0, z0], // 1
        [x1, y0, z1], // 2
        [x0, y0, z1], // 3
        // Top face (y = y1)
        [x0, y1, z0], // 4
        [x1, y1, z0], // 5
        [x1, y1, z1], // 6
        [x0, y1, z1], // 7
    ];

    // Normals for each vertex (simplified - pointing outward)
    let normals = vec![
        [0.0, -1.0, 0.0], // 0
        [0.0, -1.0, 0.0], // 1
        [0.0, -1.0, 0.0], // 2
        [0.0, -1.0, 0.0], // 3
        [0.0, 1.0, 0.0],  // 4
        [0.0, 1.0, 0.0],  // 5
        [0.0, 1.0, 0.0],  // 6
        [0.0, 1.0, 0.0],  // 7
    ];

    // Indices for 6 faces (12 triangles)
    let indices = vec![
        // Bottom
        0, 2, 1, 0, 3, 2, // Top
        4, 5, 6, 4, 6, 7, // Front (z = z1)
        3, 6, 2, 3, 7, 6, // Back (z = z0)
        0, 1, 5, 0, 5, 4, // Right (x = x1)
        1, 2, 6, 1, 6, 5, // Left (x = x0)
        0, 4, 7, 0, 7, 3,
    ];

    (vertices, normals, indices)
}

/// Generates a combined mesh for all water cells in a chunk
fn generate_water_debug_mesh_for_chunk(
    world_map: &ClientWorldMap,
    chunk_pos: &IVec3,
) -> Option<Mesh> {
    let chunk = world_map.map.get(chunk_pos)?;

    if chunk.water.is_empty() {
        return None;
    }

    let mut all_vertices: Vec<[f32; 3]> = Vec::new();
    let mut all_normals: Vec<[f32; 3]> = Vec::new();
    let mut all_indices: Vec<u32> = Vec::new();

    for (local_pos, cell) in chunk.water.iter() {
        let (vertices, normals, indices) = create_water_cell_mesh(cell.volume(), *local_pos);

        if vertices.is_empty() {
            continue;
        }

        let base_index = all_vertices.len() as u32;
        all_vertices.extend(vertices);
        all_normals.extend(normals);
        all_indices.extend(indices.iter().map(|i| i + base_index));
    }

    if all_vertices.is_empty() {
        return None;
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, Default::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, all_vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, all_normals);
    mesh.insert_indices(Indices::U32(all_indices));

    Some(mesh)
}

/// System to toggle water debug mode
pub fn toggle_water_debug_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<WaterDebugSettings>,
) {
    // F8 to toggle water debug
    if keyboard_input.just_pressed(KeyCode::F8) {
        settings.toggle();
    }
}

/// System to render water debug meshes
pub fn water_debug_render_system(
    mut commands: Commands,
    world_map: Res<ClientWorldMap>,
    settings: Res<WaterDebugSettings>,
    mut debug_entities: ResMut<WaterDebugEntities>,
    mut debug_material: ResMut<WaterDebugMaterial>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ev_chunk_update: EventReader<WorldRenderRequestUpdateEvent>,
) {
    // Initialize material if needed
    if debug_material.handle.is_none() {
        debug_material.handle = Some(materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.5, 0.9, 0.5),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None, // Show both sides
            ..default()
        }));
    }

    // If debug is disabled, despawn all debug entities
    if !settings.enabled {
        for (_, entity) in debug_entities.entities.drain() {
            commands.entity(entity).despawn();
        }
        // Consume events to avoid buildup
        ev_chunk_update.clear();
        return;
    }

    let material = debug_material.handle.clone().unwrap();

    // Collect chunks that need updating
    let chunks_to_update: Vec<IVec3> = ev_chunk_update
        .read()
        .map(|ev| {
            let WorldRenderRequestUpdateEvent::ChunkToReload(pos) = ev;
            *pos
        })
        .collect();

    for chunk_pos in chunks_to_update {
        // Remove existing entity if any
        if let Some(entity) = debug_entities.entities.remove(&chunk_pos) {
            commands.entity(entity).despawn();
        }

        // Generate new debug mesh
        if let Some(mesh) = generate_water_debug_mesh_for_chunk(&world_map, &chunk_pos) {
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
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(material.clone()),
                    WaterDebugMesh,
                ))
                .id();

            debug_entities.entities.insert(chunk_pos, entity);
        }
    }
}

/// System to clean up debug entities when chunks are unloaded
pub fn water_debug_cleanup_system(
    mut commands: Commands,
    world_map: Res<ClientWorldMap>,
    mut debug_entities: ResMut<WaterDebugEntities>,
) {
    if !world_map.is_changed() {
        return;
    }

    let chunks_to_remove: Vec<IVec3> = debug_entities
        .entities
        .keys()
        .filter(|pos| !world_map.map.contains_key(pos))
        .copied()
        .collect();

    for chunk_pos in chunks_to_remove {
        if let Some(entity) = debug_entities.entities.remove(&chunk_pos) {
            commands.entity(entity).despawn();
        }
    }
}

/// System to rebuild all water debug meshes when toggled on
pub fn water_debug_rebuild_on_enable_system(
    mut commands: Commands,
    world_map: Res<ClientWorldMap>,
    settings: Res<WaterDebugSettings>,
    mut debug_entities: ResMut<WaterDebugEntities>,
    mut debug_material: ResMut<WaterDebugMaterial>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // Only rebuild when settings change and debug is enabled
    if !settings.is_changed() || !settings.enabled {
        return;
    }

    // Initialize material if needed
    if debug_material.handle.is_none() {
        debug_material.handle = Some(materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.5, 0.9, 0.5),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        }));
    }

    let material = debug_material.handle.clone().unwrap();

    // Build debug meshes for all loaded chunks
    for (chunk_pos, _) in world_map.map.iter() {
        // Skip if already has a debug entity
        if debug_entities.entities.contains_key(chunk_pos) {
            continue;
        }

        if let Some(mesh) = generate_water_debug_mesh_for_chunk(&world_map, chunk_pos) {
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
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(material.clone()),
                    WaterDebugMesh,
                ))
                .id();

            debug_entities.entities.insert(*chunk_pos, entity);
        }
    }
}

//! Water rendering system for Rustcraft.
//!
//! This module manages water mesh entities, handles chunk updates,
//! and applies LOD-based rendering for water surfaces.
//!
//! ## System Overview
//! - Water meshes are generated separately from solid block meshes
//! - Each chunk with water gets its own water mesh entity
//! - Water uses a custom Gerstner wave shader for realistic animation
//! - LOD reduces wave detail at distance for performance
//!
//! ## Integration
//! The water rendering system runs in PostUpdate after chunk updates
//! but before final rendering, ensuring water meshes stay synchronized
//! with the world state.

use bevy::{
    prelude::*,
    tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task},
};
use std::collections::HashMap;
use std::sync::Arc;

use crate::player::CurrentPlayerMarker;
use crate::world::{ClientWorldMap, WorldRenderRequestUpdateEvent};
use crate::GameState;
use shared::CHUNK_SIZE;

use super::render_distance::RenderDistance;
use super::water_material::{WaterMaterialResource, WaterRenderSettings};
use super::water_mesh::{
    generate_water_mesh, generate_water_mesh_lod, WaterMeshData, WaterMeshInput,
};

/// Marker component for water mesh entities.
#[derive(Component)]
pub struct WaterMesh {
    /// Chunk position this water mesh belongs to
    pub chunk_pos: IVec3,
    /// Whether this mesh uses LOD (reduced detail)
    pub is_lod: bool,
}

/// Resource tracking water mesh entities per chunk.
#[derive(Resource, Default)]
pub struct WaterMeshEntities {
    /// Maps chunk position to water mesh entity
    pub entities: HashMap<IVec3, Entity>,
}

/// Async task for generating water meshes.
#[derive(Debug)]
pub struct WaterMeshTask {
    pub chunk_pos: IVec3,
    pub task: Task<Option<WaterMeshData>>,
    pub is_lod: bool,
}

/// Resource for queued water mesh generation tasks.
#[derive(Resource, Default)]
pub struct WaterMeshTasks {
    pub tasks: Vec<WaterMeshTask>,
}

/// Event to request water mesh regeneration for a chunk.
#[derive(Event, Debug, Clone, Copy)]
pub struct WaterMeshUpdateEvent(pub IVec3);

/// System to queue water mesh generation when chunks update.
///
/// This listens for chunk update events and queues water mesh regeneration
/// for chunks that contain water.
pub fn queue_water_mesh_updates(
    world_map: Res<ClientWorldMap>,
    mut ev_chunk_update: EventReader<WorldRenderRequestUpdateEvent>,
    mut ev_water_update: EventWriter<WaterMeshUpdateEvent>,
) {
    for event in ev_chunk_update.read() {
        let WorldRenderRequestUpdateEvent::ChunkToReload(chunk_pos) = event;

        // Check if chunk has water
        if let Some(chunk) = world_map.map.get(chunk_pos) {
            if !chunk.water.is_empty() {
                ev_water_update.write(WaterMeshUpdateEvent(*chunk_pos));
            }
        }
    }
}

/// System to spawn water mesh generation tasks.
///
/// This processes water mesh update events and spawns async tasks
/// to generate water meshes without blocking the main thread.
pub fn spawn_water_mesh_tasks(
    world_map: Res<ClientWorldMap>,
    render_distance: Res<RenderDistance>,
    render_settings: Option<Res<WaterRenderSettings>>,
    player_query: Query<&Transform, With<CurrentPlayerMarker>>,
    mut ev_water_update: EventReader<WaterMeshUpdateEvent>,
    mut tasks: ResMut<WaterMeshTasks>,
) {
    use shared::world::WaveScaleConfig;

    let pool = AsyncComputeTaskPool::get();

    // Get player position for LOD determination
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let player_pos = player_transform.translation;

    let lod_distance_sq = render_distance.lod0_distance_sq() as f32;

    // Get wave scale config and tessellation settings (use defaults if settings not available)
    let wave_scale_config = render_settings
        .as_ref()
        .map(|s| s.wave_scale_config)
        .unwrap_or_default();
    
    let tessellation = render_settings
        .as_ref()
        .map(|s| s.tessellation)
        .unwrap_or(4);
    
    let tessellation_lod = render_settings
        .as_ref()
        .map(|s| s.tessellation_lod)
        .unwrap_or(2);

    for WaterMeshUpdateEvent(chunk_pos) in ev_water_update.read() {
        // Check if chunk exists and has water
        let chunk = match world_map.map.get(chunk_pos) {
            Some(c) if !c.water.is_empty() => Arc::clone(c),
            _ => continue,
        };

        // Calculate chunk center for LOD determination
        let chunk_center = Vec3::new(
            (chunk_pos.x * CHUNK_SIZE + CHUNK_SIZE / 2) as f32,
            (chunk_pos.y * CHUNK_SIZE + CHUNK_SIZE / 2) as f32,
            (chunk_pos.z * CHUNK_SIZE + CHUNK_SIZE / 2) as f32,
        );

        let distance_sq = player_pos.distance_squared(chunk_center);
        let is_lod = distance_sq > lod_distance_sq;

        let chunk_pos_copy = *chunk_pos;
        let world_map_clone = world_map.clone();
        let wave_scale_config_copy = wave_scale_config;
        let tessellation_copy = tessellation;
        let tessellation_lod_copy = tessellation_lod;

        let task = pool.spawn(async move {
            let input = WaterMeshInput {
                chunk_pos: chunk_pos_copy,
                water: &chunk.water,
                world_map: &world_map_clone,
                wave_scale_config: wave_scale_config_copy,
                tessellation: tessellation_copy,
                tessellation_lod: tessellation_lod_copy,
            };

            if is_lod {
                generate_water_mesh_lod(&input)
            } else {
                generate_water_mesh(&input)
            }
        });

        tasks.tasks.push(WaterMeshTask {
            chunk_pos: *chunk_pos,
            task,
            is_lod,
        });
    }
}

/// System to process completed water mesh tasks and spawn entities.
///
/// This polls async mesh generation tasks and creates/updates
/// water mesh entities when tasks complete.
pub fn process_water_mesh_tasks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut tasks: ResMut<WaterMeshTasks>,
    mut water_entities: ResMut<WaterMeshEntities>,
    material_resource: Option<Res<WaterMaterialResource>>,
) {
    // Get water material handle if available
    let Some(material_res) = material_resource else {
        // Water material not yet initialized, skip processing
        return;
    };
    let material_handle = material_res.handle.clone();

    // Process completed tasks
    tasks.tasks.retain_mut(|mesh_task| {
        if let Some(result) = block_on(future::poll_once(&mut mesh_task.task)) {
            // Remove existing entity for this chunk
            if let Some(entity) = water_entities.entities.remove(&mesh_task.chunk_pos) {
                commands.entity(entity).despawn();
            }

            // Spawn new entity if mesh was generated
            if let Some(mesh_data) = result {
                if let Some(mesh) = mesh_data.into_mesh() {
                    let transform = Transform::from_xyz(
                        (mesh_task.chunk_pos.x * CHUNK_SIZE) as f32,
                        (mesh_task.chunk_pos.y * CHUNK_SIZE) as f32,
                        (mesh_task.chunk_pos.z * CHUNK_SIZE) as f32,
                    );

                    let entity = commands
                        .spawn((
                            StateScoped(GameState::Game),
                            transform,
                            Visibility::Visible,
                            Mesh3d(meshes.add(mesh)),
                            MeshMaterial3d(material_handle.clone()),
                            WaterMesh {
                                chunk_pos: mesh_task.chunk_pos,
                                is_lod: mesh_task.is_lod,
                            },
                        ))
                        .id();

                    water_entities.entities.insert(mesh_task.chunk_pos, entity);
                }
            }

            false // Remove completed task
        } else {
            true // Keep pending task
        }
    });
}

/// System to clean up water meshes when chunks are unloaded.
pub fn cleanup_water_meshes(
    mut commands: Commands,
    world_map: Res<ClientWorldMap>,
    mut water_entities: ResMut<WaterMeshEntities>,
) {
    if !world_map.is_changed() {
        return;
    }

    // Find chunks that no longer exist
    let chunks_to_remove: Vec<IVec3> = water_entities
        .entities
        .keys()
        .filter(|pos| !world_map.map.contains_key(pos))
        .copied()
        .collect();

    // Despawn entities for removed chunks
    for chunk_pos in chunks_to_remove {
        if let Some(entity) = water_entities.entities.remove(&chunk_pos) {
            commands.entity(entity).despawn();
        }
    }
}

/// System to rebuild water meshes when entering the game state.
///
/// This ensures all existing water in the world gets rendered
/// when the player joins or respawns.
pub fn rebuild_all_water_meshes(
    world_map: Res<ClientWorldMap>,
    mut ev_water_update: EventWriter<WaterMeshUpdateEvent>,
) {
    for (chunk_pos, chunk) in world_map.map.iter() {
        if !chunk.water.is_empty() {
            ev_water_update.write(WaterMeshUpdateEvent(*chunk_pos));
        }
    }

    info!(
        "Queued water mesh rebuild for {} chunks",
        world_map.map.len()
    );
}

/// System to update water mesh LOD based on player distance.
///
/// This periodically checks if water meshes need to switch between
/// full detail and LOD versions based on player position.
pub fn update_water_lod(
    water_query: Query<(&WaterMesh, &Transform)>,
    player_query: Query<&Transform, With<CurrentPlayerMarker>>,
    render_distance: Res<RenderDistance>,
    mut ev_water_update: EventWriter<WaterMeshUpdateEvent>,
    time: Res<Time>,
    mut last_update: Local<f32>,
) {
    // Only check every 2 seconds to avoid constant rebuilding
    *last_update += time.delta_secs();
    if *last_update < 2.0 {
        return;
    }
    *last_update = 0.0;

    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let player_pos = player_transform.translation;

    let lod_distance_sq = render_distance.lod0_distance_sq() as f32;

    for (water_mesh, transform) in water_query.iter() {
        let distance_sq = player_pos.distance_squared(transform.translation);
        let should_be_lod = distance_sq > lod_distance_sq;

        // If LOD state doesn't match, queue rebuild
        if water_mesh.is_lod != should_be_lod {
            ev_water_update.write(WaterMeshUpdateEvent(water_mesh.chunk_pos));
        }
    }
}

/// System to toggle water rendering on/off (F9).
pub fn toggle_water_rendering(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<WaterRenderSettings>,
    mut water_query: Query<&mut Visibility, With<WaterMesh>>,
) {
    if keyboard_input.just_pressed(KeyCode::F9) {
        settings.enabled = !settings.enabled;
        info!(
            "Water rendering: {}",
            if settings.enabled { "ON" } else { "OFF" }
        );

        // Update visibility of all water meshes
        let visibility = if settings.enabled {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };

        for mut vis in water_query.iter_mut() {
            *vis = visibility;
        }
    }
}

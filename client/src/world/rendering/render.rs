use crate::shaders::water::{StandardWaterMaterial, WaterMaterial, WaterMesh};
use crate::{player::CurrentPlayerMarker, world::FirstChunkReceived};
use std::sync::Arc;
use std::{collections::HashSet, time::Instant};

use bevy::{
    asset::Assets,
    math::IVec3,
    pbr::ExtendedMaterial,
    prelude::*,
    tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task},
};
use shared::{
    world::{global_block_to_chunk_pos, LodLevel, SIX_OFFSETS},
    CHUNK_SIZE,
};

use crate::{
    world::{self, MaterialResource, QueuedEvents, WorldRenderRequestUpdateEvent},
    GameState,
};

use crate::world::{ClientChunk, ClientWorldMap};

use super::meshing::ChunkMeshResponse;
use super::render_distance::RenderDistance;

#[derive(Debug)]
pub struct MeshingTask {
    pub chunk_pos: IVec3,
    pub mesh_request_ts: Instant,
    pub thread: Task<ChunkMeshResponse>,
    pub lod_level: LodLevel, // Track the LOD level for this mesh task
}

#[derive(Debug, Default, Resource)]
pub struct QueuedMeshes {
    pub meshes: Vec<MeshingTask>,
}

#[derive(Default)]
pub(crate) struct WorldMapCache {
    cached: Option<Arc<ClientWorldMap>>,
}

fn update_chunk(
    chunk: &mut ClientChunk,
    chunk_pos: &IVec3,
    material_resource: &MaterialResource,
    water_material: &Handle<StandardWaterMaterial>,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    new_meshes: ChunkMeshResponse,
    lod_level: LodLevel, // Track the LOD level used for this mesh
) {
    let solid_texture = material_resource
        .global_materials
        .get(&world::GlobalMaterial::Blocks)
        .unwrap();

    if chunk.entity.is_some() {
        commands.entity(chunk.entity.unwrap()).despawn();
        chunk.entity = None;
    }

    if chunk.entity.is_none() {
        // Offset the chunk's position by half a block so that blocks are centered
        let chunk_t = Transform::from_xyz(
            (chunk_pos.x * CHUNK_SIZE) as f32,
            (chunk_pos.y * CHUNK_SIZE) as f32,
            (chunk_pos.z * CHUNK_SIZE) as f32,
        );

        let new_entity = commands
            .spawn((chunk_t, Visibility::Visible))
            .with_children(|root| {
                // Spawn solid mesh
                if let Some(new_solid_mesh) = new_meshes.solid_mesh {
                    root.spawn((
                        StateScoped(GameState::Game),
                        Mesh3d(meshes.add(new_solid_mesh)),
                        MeshMaterial3d(solid_texture.clone()),
                    ));
                }
                // Spawn water mesh with custom water material (only for LOD 0)
                if let Some(new_water_mesh) = new_meshes.water_mesh {
                    debug!("Spawning water mesh for chunk");
                    root.spawn((
                        StateScoped(GameState::Game),
                        Mesh3d(meshes.add(new_water_mesh)),
                        MeshMaterial3d(water_material.clone()),
                        WaterMesh,
                    ));
                }
            })
            .id();

        chunk.entity = Some(new_entity);
    }

    // Update the chunk's current LOD level
    chunk.current_lod = lod_level;
    // debug!("ClientChunk updated : len={}", chunk.map.len());
}

/// Resource to store the water material handle for chunk rendering
#[derive(Resource, Default)]
pub struct ChunkWaterMaterial {
    pub handle: Option<Handle<StandardWaterMaterial>>,
}

pub fn world_render_system(
    mut world_map: ResMut<ClientWorldMap>,
    material_resource: Res<MaterialResource>,
    render_distance: Res<RenderDistance>,
    mut water_material_res: ResMut<ChunkWaterMaterial>,
    mut water_materials: ResMut<Assets<StandardWaterMaterial>>,
    mut ev_render: EventReader<WorldRenderRequestUpdateEvent>,
    mut queued_events: Local<QueuedEvents>,
    mut queued_meshes: Local<QueuedMeshes>,
    mut world_map_cache: Local<WorldMapCache>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
    mut first_chunk_received: ResMut<FirstChunkReceived>,
    player_pos: Query<&Transform, With<CurrentPlayerMarker>>,
) {
    for event in ev_render.read() {
        queued_events.events.insert(*event);
    }

    if material_resource.blocks.is_none() {
        // Wait until the texture is ready
        return;
    }

    // Initialize water material if not already created
    let water_material_handle = if let Some(ref handle) = water_material_res.handle {
        handle.clone()
    } else {
        let handle = water_materials.add(ExtendedMaterial {
            base: StandardMaterial {
                base_color: Color::srgba(0.1, 0.3, 0.5, 0.8),
                alpha_mode: AlphaMode::Blend,
                ..default()
            },
            extension: WaterMaterial {
                amplitude: 0.5,
                clarity: 0.3,
                deep_color: Color::srgba(0.05, 0.15, 0.25, 0.9),
                shallow_color: Color::srgba(0.15, 0.35, 0.45, 0.75),
                edge_color: Color::srgba(0.8, 0.9, 1.0, 0.5),
                edge_scale: 0.1,
                ..default()
            },
        });
        water_material_res.handle = Some(handle.clone());
        handle
    };

    let pool = AsyncComputeTaskPool::get();

    let events = queued_events.events.clone();

    // Get LOD thresholds
    let lod0_distance_sq = render_distance.lod0_distance_sq();

    if !events.is_empty() {
        // Clone map only when it changed, then share it as read-only across all meshing threads
        let map_ptr =
            if world_map.dirty || world_map_cache.cached.is_none() {
                let start = std::time::Instant::now();
                let new_clone = Arc::new(world_map.clone());
                world_map.dirty = false;
                world_map_cache.cached = Some(Arc::clone(&new_clone));
                let delta = start.elapsed();
                info!("cloning map for render, took {:?}", delta);
                new_clone
            } else {
                Arc::clone(world_map_cache.cached.as_ref().expect(
                    "World map cache should be populated after first clone; caching logic bug",
                ))
            };

        let uvs = Arc::new(material_resource.blocks.as_ref().unwrap().uvs.clone());

        let player_pos = player_pos
            .single()
            .expect("Player should exist")
            .translation;
        let player_chunk_pos = global_block_to_chunk_pos(&IVec3::new(
            player_pos.x as i32,
            player_pos.y as i32,
            player_pos.z as i32,
        ));

        let mut chunks_to_reload: HashSet<IVec3> = HashSet::new();

        // Using a set so same chunks are not reloaded multiple times
        // Accumulate chunks to render
        for event in &events {
            let WorldRenderRequestUpdateEvent::ChunkToReload(target_chunk_pos) = event;

            chunks_to_reload.insert(*target_chunk_pos);

            // Only add neighbor chunks for LOD0 chunks.
            // LOD1 chunks use simplified meshes that don't need neighbor precision,
            // so we skip the neighbor cascade to reduce mesh generation overhead.
            let chunk_distance_sq = target_chunk_pos.distance_squared(player_chunk_pos);
            let chunk_lod = LodLevel::from_distance_squared(chunk_distance_sq, lod0_distance_sq);
            if chunk_lod == LodLevel::Lod0 {
                for offset in &SIX_OFFSETS {
                    chunks_to_reload.insert(*target_chunk_pos + *offset);
                }
            }
        }

        let mut chunks_to_reload = Vec::from_iter(chunks_to_reload);

        chunks_to_reload.sort_by_key(|pos| pos.distance_squared(player_chunk_pos));

        for pos in chunks_to_reload {
            if let Some(chunk) = world_map.map.get(&pos) {
                // If chunk is empty, ignore it
                if chunk.map.is_empty() {
                    continue;
                }

                // Calculate LOD level based on distance from player
                let chunk_distance_sq = pos.distance_squared(player_chunk_pos);
                let lod_level =
                    LodLevel::from_distance_squared(chunk_distance_sq, lod0_distance_sq);

                // Define variables to move to the thread
                let map_clone = Arc::clone(&map_ptr);
                let uvs_clone = Arc::clone(&uvs);
                let ch = chunk.clone();
                let t = pool.spawn(async move {
                    world::meshing::generate_chunk_mesh_lod(
                        &map_clone, &ch, &pos, &uvs_clone, lod_level,
                    )
                });

                queued_meshes.meshes.push(MeshingTask {
                    chunk_pos: pos,
                    mesh_request_ts: Instant::now(),
                    thread: t,
                    lod_level,
                });
            }
        }
        first_chunk_received.0 = true;
    }

    // Iterate through queued meshes to see if they are completed
    queued_meshes.meshes.retain_mut(|task| {
        let MeshingTask {
            chunk_pos,
            mesh_request_ts,
            thread,
            lod_level,
        } = task;

        if let Some(chunk_arc) = world_map.map.get_mut(chunk_pos) {
            // If a later mesh has been completed before, we can drop this task
            if *mesh_request_ts < chunk_arc.last_mesh_ts {
                false
            }
            // If completed, use the mesh to update the chunk and delete it from the meshing queue
            else if let Some(new_meshes) = block_on(future::poll_once(thread)) {
                // Update the corresponding chunk (use Arc::make_mut for copy-on-write)
                let chunk = std::sync::Arc::make_mut(chunk_arc);
                update_chunk(
                    chunk,
                    chunk_pos,
                    &material_resource,
                    &water_material_handle,
                    &mut commands,
                    &mut meshes,
                    new_meshes,
                    *lod_level,
                );
                false
            } else {
                // Else, keep the task until it is done
                true
            }
        } else {
            true
        }
    });

    queued_events.events.clear();
}

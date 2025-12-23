use crate::{player::CurrentPlayerMarker, world::FirstChunkReceived};
use std::sync::Arc;
use std::{collections::HashSet, time::Instant};

use bevy::{
    asset::Assets,
    math::IVec3,
    prelude::*,
    render::camera::CameraProjection,
    tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task},
};
use shared::{
    world::{global_block_to_chunk_pos, SIX_OFFSETS},
    CHUNK_SIZE,
};

use crate::{
    world::{self, MaterialResource, QueuedEvents, WorldRenderRequestUpdateEvent},
    GameState,
};

use crate::world::{ClientChunk, ClientWorldMap};

use super::meshing::ChunkMeshResponse;

/// Marker component for chunk entities to enable frustum culling visibility updates
#[derive(Component)]
pub struct ChunkEntity {
    pub chunk_pos: IVec3,
}

#[derive(Debug)]
pub struct MeshingTask {
    pub chunk_pos: IVec3,
    pub mesh_request_ts: Instant,
    pub thread: Task<ChunkMeshResponse>,
}

#[derive(Debug, Default, Resource)]
pub struct QueuedMeshes {
    pub meshes: Vec<MeshingTask>,
}

fn update_chunk(
    chunk: &mut ClientChunk,
    chunk_pos: &IVec3,
    material_resource: &MaterialResource,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    new_meshes: ChunkMeshResponse,
) {
    // If the

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
            .spawn((
                chunk_t,
                Visibility::Visible,
                ChunkEntity {
                    chunk_pos: *chunk_pos,
                },
            ))
            .with_children(|root| {
                if let Some(new_solid_mesh) = new_meshes.solid_mesh {
                    root.spawn((
                        StateScoped(GameState::Game),
                        Mesh3d(meshes.add(new_solid_mesh)),
                        MeshMaterial3d(solid_texture.clone()),
                    ));
                }
            })
            .id();

        chunk.entity = Some(new_entity);
    }
    // debug!("ClientChunk updated : len={}", chunk.map.len());
}

pub fn world_render_system(
    mut world_map: ResMut<ClientWorldMap>,
    material_resource: Res<MaterialResource>,
    mut ev_render: EventReader<WorldRenderRequestUpdateEvent>,
    mut queued_events: Local<QueuedEvents>,
    mut queued_meshes: Local<QueuedMeshes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
    mut first_chunk_received: ResMut<FirstChunkReceived>,
    player_query: Query<&Transform, With<CurrentPlayerMarker>>,
    camera_query: Query<(&Transform, &Projection), With<Camera3d>>,
) {
    for event in ev_render.read() {
        queued_events.events.insert(*event);
    }

    if material_resource.blocks.is_none() {
        // Wait until the texture is ready
        return;
    }

    let pool = AsyncComputeTaskPool::get();

    let events = queued_events.events.clone();

    if !events.is_empty() {
        let start = std::time::Instant::now();

        // Clone map only once, then share it as read-only across all meshing threads
        let map_ptr = Arc::new(world_map.clone());
        let delta = start.elapsed();
        info!("cloning map for render, took {:?}", delta);

        let uvs = Arc::new(material_resource.blocks.as_ref().unwrap().uvs.clone());

        let mut chunks_to_reload: HashSet<IVec3> = HashSet::new();

        // Using a set so same chunks are not reloaded multiple times
        // Accumulate chunks to render
        for event in &events {
            let WorldRenderRequestUpdateEvent::ChunkToReload(target_chunk_pos) = event;

            chunks_to_reload.insert(*target_chunk_pos);
            for offset in &SIX_OFFSETS {
                chunks_to_reload.insert(*target_chunk_pos + *offset);
            }
        }

        let player_transform = player_query.single().expect("Player should exist");
        let player_pos = player_transform.translation;
        let player_chunk_pos = global_block_to_chunk_pos(&IVec3::new(
            player_pos.x as i32,
            player_pos.y as i32,
            player_pos.z as i32,
        ));

        // Get camera info for priority calculation
        let (camera_pos, camera_forward, frustum) = if let Ok((camera_transform, projection)) =
            camera_query.single()
        {
            let view_matrix = camera_transform.compute_matrix().inverse();
            let projection_matrix = match projection {
                Projection::Perspective(persp) => persp.get_clip_from_view(),
                Projection::Orthographic(ortho) => ortho.get_clip_from_view(),
                Projection::Custom(custom) => custom.get_clip_from_view(),
            };
            let view_projection = projection_matrix * view_matrix;
            let frustum = super::frustum::Frustum::from_view_projection_matrix(&view_projection);

            // Camera forward is the negative Z axis in camera space
            let forward = camera_transform.forward().as_vec3();

            (camera_transform.translation, forward, frustum)
        } else {
            // Fallback if camera not available
            let default_frustum = super::frustum::Frustum {
                planes: [
                    super::frustum::Plane::new(0.0, 0.0, 0.0, 1000000.0),
                    super::frustum::Plane::new(0.0, 0.0, 0.0, 1000000.0),
                    super::frustum::Plane::new(0.0, 0.0, 0.0, 1000000.0),
                    super::frustum::Plane::new(0.0, 0.0, 0.0, 1000000.0),
                    super::frustum::Plane::new(0.0, 0.0, 0.0, 1000000.0),
                    super::frustum::Plane::new(0.0, 0.0, 0.0, 1000000.0),
                ],
            };
            (player_pos, Vec3::NEG_Z, default_frustum)
        };

        let mut chunks_to_reload = Vec::from_iter(chunks_to_reload);

        // Sort chunks by priority score (lower = higher priority)
        // This ensures: player chunk first, then visible chunks in front, then partial, then behind
        chunks_to_reload.sort_by(|a, b| {
            let priority_a = super::frustum::calculate_chunk_priority(
                *a,
                CHUNK_SIZE,
                camera_pos,
                camera_forward,
                &frustum,
                player_chunk_pos,
            );
            let priority_b = super::frustum::calculate_chunk_priority(
                *b,
                CHUNK_SIZE,
                camera_pos,
                camera_forward,
                &frustum,
                player_chunk_pos,
            );
            priority_a
                .partial_cmp(&priority_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for pos in chunks_to_reload {
            if let Some(chunk) = world_map.map.get(&pos) {
                // If chunk is empty, ignore it
                if chunk.map.is_empty() {
                    continue;
                }

                // Define variables to move to the thread
                let map_clone = Arc::clone(&map_ptr);
                let uvs_clone = Arc::clone(&uvs);
                let ch = chunk.clone();
                let t = pool.spawn(async move {
                    world::meshing::generate_chunk_mesh(&map_clone, &ch, &pos, &uvs_clone)
                });

                queued_meshes.meshes.push(MeshingTask {
                    chunk_pos: pos,
                    mesh_request_ts: Instant::now(),
                    thread: t,
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
        } = task;

        if let Some(chunk) = world_map.map.get_mut(chunk_pos) {
            // If a later mesh has been completed before, we can drop this task
            if *mesh_request_ts < chunk.last_mesh_ts {
                false
            }
            // If completed, use the mesh to update the chunk and delete it from the meshing queue
            else if let Some(new_meshes) = block_on(future::poll_once(thread)) {
                // Update the corresponding chunk
                update_chunk(
                    chunk,
                    chunk_pos,
                    &material_resource,
                    &mut commands,
                    &mut meshes,
                    new_meshes,
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

/// System to update chunk visibility based on view frustum culling.
/// This runs every frame to show/hide chunks based on the current camera view.
pub fn frustum_cull_chunks_system(
    camera_query: Query<(&Transform, &Projection), With<Camera3d>>,
    mut chunk_query: Query<(&ChunkEntity, &mut Visibility)>,
) {
    // Get the camera transform and projection
    let Ok((camera_transform, projection)) = camera_query.single() else {
        return;
    };

    // Build the view-projection matrix
    let view_matrix = camera_transform.compute_matrix().inverse();
    let projection_matrix = match projection {
        Projection::Perspective(persp) => persp.get_clip_from_view(),
        Projection::Orthographic(ortho) => ortho.get_clip_from_view(),
        Projection::Custom(custom) => custom.get_clip_from_view(),
    };
    let view_projection = projection_matrix * view_matrix;

    // Extract the frustum
    let frustum = super::frustum::Frustum::from_view_projection_matrix(&view_projection);

    // Update visibility for each chunk entity
    for (chunk_entity, mut visibility) in chunk_query.iter_mut() {
        let is_visible = frustum.intersects_chunk(chunk_entity.chunk_pos, CHUNK_SIZE);

        *visibility = if is_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

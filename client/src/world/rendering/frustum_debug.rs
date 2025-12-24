use bevy::prelude::*;
use bevy::render::camera::CameraProjection;
use super::frustum::Frustum;
use shared::CHUNK_SIZE;
use crate::world::ClientWorldMap;

/// Resource to toggle frustum culling debug visualization
#[derive(Resource, Default)]
pub struct FrustumDebugSettings {
    pub enabled: bool,
    pub show_culled_chunks: bool,
}

/// System to toggle frustum debug visualization with F8 key
pub fn toggle_frustum_debug(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<FrustumDebugSettings>,
) {
    if keyboard.just_pressed(KeyCode::F8) {
        settings.enabled = !settings.enabled;
        if settings.enabled {
            info!("Frustum culling debug visualization enabled");
        } else {
            info!("Frustum culling debug visualization disabled");
        }
    }
    
    if keyboard.just_pressed(KeyCode::F9) {
        settings.show_culled_chunks = !settings.show_culled_chunks;
        if settings.show_culled_chunks {
            info!("Showing culled chunks");
        } else {
            info!("Hiding culled chunks");
        }
    }
}

/// System to display frustum culling statistics
pub fn display_frustum_stats(
    settings: Res<FrustumDebugSettings>,
    world_map: Res<ClientWorldMap>,
    camera_query: Query<(&Transform, &Projection), With<Camera3d>>,
    mut gizmos: Gizmos,
) {
    if !settings.enabled {
        return;
    }

    // Get frustum from camera
    let Ok((camera_transform, projection)) = camera_query.single() else {
        return;
    };

    let view_matrix = camera_transform.compute_matrix().inverse();
    let projection_matrix = match projection {
        Projection::Perspective(persp) => persp.get_clip_from_view(),
        Projection::Orthographic(ortho) => ortho.get_clip_from_view(),
        Projection::Custom(custom) => custom.get_clip_from_view(),
    };
    let view_projection = projection_matrix * view_matrix;
    let frustum = Frustum::from_view_projection_matrix(&view_projection);

    let mut visible_chunks = 0;
    let mut culled_chunks = 0;

    // Count visible and culled chunks
    for (chunk_pos, _chunk) in world_map.map.iter() {
        let is_visible =
            frustum.intersects_chunk(*chunk_pos, CHUNK_SIZE);
        
        if is_visible {
            visible_chunks += 1;
            
            // Draw visible chunk bounds in green
            if settings.show_culled_chunks {
                let min = Vec3::new(
                    (chunk_pos.x * CHUNK_SIZE) as f32,
                    (chunk_pos.y * CHUNK_SIZE) as f32,
                    (chunk_pos.z * CHUNK_SIZE) as f32,
                );
                let max = min + Vec3::splat(CHUNK_SIZE as f32);
                gizmos.cuboid(
                    Transform::from_translation((min + max) / 2.0)
                        .with_scale(max - min),
                    Color::srgb(0.0, 1.0, 0.0).with_alpha(0.3),
                );
            }
        } else {
            culled_chunks += 1;
            
            // Draw culled chunk bounds in red
            if settings.show_culled_chunks {
                let min = Vec3::new(
                    (chunk_pos.x * CHUNK_SIZE) as f32,
                    (chunk_pos.y * CHUNK_SIZE) as f32,
                    (chunk_pos.z * CHUNK_SIZE) as f32,
                );
                let max = min + Vec3::splat(CHUNK_SIZE as f32);
                gizmos.cuboid(
                    Transform::from_translation((min + max) / 2.0)
                        .with_scale(max - min),
                    Color::srgb(1.0, 0.0, 0.0).with_alpha(0.2),
                );
            }
        }
    }

    let total_chunks = visible_chunks + culled_chunks;
    let cull_percentage = if total_chunks > 0 {
        (culled_chunks as f32 / total_chunks as f32) * 100.0
    } else {
        0.0
    };

    debug!(
        "Frustum Culling - Visible: {}, Culled: {}, Total: {}, Culled %: {:.1}%",
        visible_chunks, culled_chunks, total_chunks, cull_percentage
    );
}

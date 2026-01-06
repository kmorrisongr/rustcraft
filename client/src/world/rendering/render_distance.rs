use crate::{
    input::{data::GameAction, keyboard::is_action_just_pressed},
    player::CurrentPlayerMarker,
    world::{ClientWorldMap, WorldRenderRequestUpdateEvent},
    KeyMap,
};
use bevy::prelude::*;
use shared::{
    world::{global_block_to_chunk_pos, LodLevel},
    DEFAULT_RENDER_DISTANCE, LOD1_MULTIPLIER,
};

#[derive(Resource, Default, Reflect)]
pub struct RenderDistance {
    pub distance: i32,
}

impl RenderDistance {
    /// Get the LOD 0 (full detail) render distance.
    /// Returns at least MIN_LOD0_DISTANCE to ensure LOD 1 doesn't appear too close.
    pub fn lod0_distance(&self) -> i32 {
        self.distance.max(DEFAULT_RENDER_DISTANCE)
    }

    /// Get the squared LOD 0 distance (for efficient distance comparisons)
    pub fn lod0_distance_sq(&self) -> i32 {
        self.lod0_distance().pow(2)
    }

    /// Get the LOD 1 (reduced detail) render distance
    pub fn lod1_distance(&self) -> i32 {
        (self.distance as f32 * LOD1_MULTIPLIER) as i32
    }

    /// Get the squared LOD 1 distance (for efficient distance comparisons)
    pub fn lod1_distance_sq(&self) -> i32 {
        self.lod1_distance().pow(2)
    }
}

pub fn render_distance_update_system(
    mut render_distance: ResMut<RenderDistance>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
) {
    if render_distance.distance <= 0 {
        render_distance.distance = DEFAULT_RENDER_DISTANCE;
    }

    if is_action_just_pressed(GameAction::RenderDistanceMinus, &keyboard_input, &key_map) {
        render_distance.distance = (render_distance.distance - 1).max(1);
    }

    if is_action_just_pressed(GameAction::RenderDistancePlus, &keyboard_input, &key_map) {
        render_distance.distance = render_distance.distance.saturating_add(1);
    }
}

/// Timer for periodic LOD transition checks
#[derive(Resource)]
pub struct LodTransitionTimer(pub Timer);

impl Default for LodTransitionTimer {
    fn default() -> Self {
        // Check every 0.5 seconds to balance responsiveness vs performance
        Self(Timer::from_seconds(0.5, TimerMode::Repeating))
    }
}

/// System that checks all loaded chunks and triggers re-render when LOD level should change.
/// This handles the case where player moves closer to LOD1 chunks that should become LOD0.
pub fn lod_transition_system(
    time: Res<Time>,
    mut timer: ResMut<LodTransitionTimer>,
    render_distance: Res<RenderDistance>,
    world_map: Res<ClientWorldMap>,
    player_query: Query<&Transform, With<CurrentPlayerMarker>>,
    mut ev_render: EventWriter<WorldRenderRequestUpdateEvent>,
) {
    // Only check periodically to avoid performance impact
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let Ok(player_transform) = player_query.single() else {
        return;
    };

    let player_pos = player_transform.translation;
    let player_chunk_pos = global_block_to_chunk_pos(&bevy::math::IVec3::new(
        player_pos.x as i32,
        player_pos.y as i32,
        player_pos.z as i32,
    ));

    let lod0_distance_sq = render_distance.lod0_distance_sq();
    let lod0_distance = (lod0_distance_sq as f32).sqrt() as i32;

    // Only check chunks near the LOD boundary (within 2 chunks of transition distance).
    // This avoids iterating over all loaded chunks when only boundary chunks can transition.
    let boundary_margin = 2;
    let min_check_distance_sq = (lod0_distance - boundary_margin).max(0).pow(2);
    let max_check_distance_sq = (lod0_distance + boundary_margin).pow(2);

    // Check only chunks near the LOD boundary to see if their LOD level should change
    for (chunk_pos, chunk) in world_map.map.iter() {
        let chunk_distance_sq = chunk_pos.distance_squared(player_chunk_pos);

        // Skip chunks that are clearly not near the LOD boundary
        if chunk_distance_sq < min_check_distance_sq || chunk_distance_sq > max_check_distance_sq {
            continue;
        }

        let expected_lod = LodLevel::from_distance_squared(chunk_distance_sq, lod0_distance_sq);

        // If the chunk's current LOD doesn't match what it should be, trigger a re-render
        if expected_lod != chunk.current_lod {
            ev_render.write(WorldRenderRequestUpdateEvent::ChunkToReload(*chunk_pos));
        }
    }
}

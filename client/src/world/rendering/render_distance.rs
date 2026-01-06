use crate::{
    input::{data::GameAction, keyboard::is_action_just_pressed},
    KeyMap,
};
use bevy::prelude::*;
use shared::{DEFAULT_RENDER_DISTANCE, LOD1_MULTIPLIER};

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

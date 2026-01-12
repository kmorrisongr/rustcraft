pub mod biome;
pub mod blocks;
pub mod chunks;
pub mod coords;
pub mod fps;
pub mod inspector;
mod loaded_stats;
pub mod raycast;
pub mod setup;
pub mod targeted_block;

use bevy::prelude::*;
pub use biome::*;
pub use blocks::*;
pub use chunks::*;
pub use coords::*;
pub use fps::*;
pub use loaded_stats::*;
pub use raycast::*;
pub use setup::*;
use shared::sets::GameUpdateSet;

use crate::ui::hud::debug::targeted_block::block_text_update_system;

#[derive(Resource, Default)]
pub struct DebugOptions {
    is_chunk_debug_mode_enabled: bool,
    is_raycast_debug_mode_enabled: bool,
}

impl DebugOptions {
    pub fn toggle_chunk_debug_mode(&mut self) {
        self.is_chunk_debug_mode_enabled = !self.is_chunk_debug_mode_enabled;
    }

    pub fn toggle_raycast_debug_mode(&mut self) {
        self.is_raycast_debug_mode_enabled = !self.is_raycast_debug_mode_enabled;
        println!(
            "Raycast debug mode is now {}",
            self.is_raycast_debug_mode_enabled
        );
    }
}

pub struct DebugHudPlugin;
impl Plugin for DebugHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                fps_text_update_system,
                coords_text_update_system,
                biome_text_update_system,
                total_blocks_text_update_system,
                block_text_update_system,
                time_text_update_system,
                toggle_hud_system,
                chunk_ghost_update_system,
                raycast_debug_update_system,
                toggle_wireframe_system,
            )
                .in_set(GameUpdateSet::Ui),
        );
    }
}

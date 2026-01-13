pub mod materials;
pub mod meshing;
pub mod render;
pub mod render_distance;
pub mod voxel;
pub mod water;

pub use materials::*;
pub use render::*;
pub use render_distance::*;
// Note: water module types are imported directly where needed (game.rs)
// to avoid polluting the rendering namespace

use bevy::prelude::*;
use shared::{
    sets::GameSet,
    world::{BlockId, ItemId},
};

use crate::{
    world::water::{water_cleanup_system, water_render_system, WaterEntities, WaterMaterialHandle},
    GameState,
};

pub struct RenderingPlugin;
impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WaterEntities>()
            .init_resource::<WaterMaterialHandle>()
            .init_resource::<RenderDistance>()
            .init_resource::<LodTransitionTimer>()
            .init_resource::<MaterialResource>()
            .init_resource::<AtlasHandles<BlockId>>()
            .init_resource::<AtlasHandles<ItemId>>()
            .init_resource::<TextureLoadingState>()
            .add_systems(
                OnEnter(GameState::PreGameLoading),
                (setup_materials,).in_set(GameSet::Resources),
            )
            .add_systems(
                Update,
                (setup_materials, create_all_atlases).in_set(GameSet::Initialize),
            )
            .add_systems(
                Update,
                (render_distance_update_system, lod_transition_system).in_set(GameSet::Rendering),
            )
            .add_systems(
                PostUpdate,
                (
                    world_render_system,
                    // Water rendering runs after chunk meshing, listening to the same events
                    water_render_system,
                    water_cleanup_system,
                )
                    .chain()
                    .in_set(GameSet::Rendering),
            );
    }
}

pub fn reset_texture_loading_state(mut loading: ResMut<TextureLoadingState>) {
    *loading = TextureLoadingState::default();
}

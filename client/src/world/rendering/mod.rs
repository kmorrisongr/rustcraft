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
use bevy_asset_loader::prelude::*;
use shared::{
    game_state::GameState,
    sets::GameSets,
    world::{BlockId, ItemId},
};

use crate::world::water::{
    water_cleanup_system, water_render_system, WaterEntities, WaterMaterialHandle,
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
            // Configure dynamic assets before loading state starts
            .add_systems(
                OnEnter(GameState::PreGameLoading),
                (configure_dynamic_texture_assets, setup_basic_materials).chain(),
            )
            // Configure the loading state with bevy_asset_loader
            .add_loading_state(
                LoadingState::new(GameState::PreGameLoading)
                    .load_collection::<BlockTextureAssets>()
                    .load_collection::<ItemTextureAssets>(),
            )
            // Initialize atlas handles after assets are loaded, then create atlases
            .add_systems(
                OnEnter(GameState::Game),
                (
                    init_block_atlas_handles,
                    init_item_atlas_handles,
                    create_all_atlases,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (render_distance_update_system, lod_transition_system)
                    .in_set(GameSets::Update::Rendering),
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
                    .in_set(GameSets::PostUpdate::Rendering),
            );
    }
}

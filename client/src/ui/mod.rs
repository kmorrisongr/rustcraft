pub mod assets;
pub mod button;
pub mod hud;
pub mod list_item;
pub mod menus;
pub mod style;

use bevy::prelude::*;
use shared::sets::{GameOnEnterSet, GameUpdateSet};

use crate::{
    ui::{
        hud::{
            chat::{render_chat, setup_chat},
            debug::setup_hud,
            loading_overlay::{setup_loading_overlay, update_loading_overlay},
            render_inventory_hotbar,
            reticle::spawn_reticle,
            set_ui_mode,
        },
        menus::pause::{render_pause_menu, setup_pause_menu},
    },
    GameState,
};

pub struct PlayerUiPlugin;
impl Plugin for PlayerUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::Game),
            (
                spawn_reticle,
                setup_loading_overlay,
                setup_hud,
                setup_chat,
                setup_pause_menu,
            )
                .chain()
                .in_set(GameOnEnterSet::Ui),
        )
        .add_systems(
            Update,
            (
                render_pause_menu,
                render_chat,
                render_inventory_hotbar,
                set_ui_mode,
                update_loading_overlay,
            )
                .in_set(GameUpdateSet::Ui),
        );
    }
}

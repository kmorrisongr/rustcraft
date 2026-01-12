pub mod assets;
pub mod button;
pub mod hud;
pub mod list_item;
pub mod menus;
pub mod style;

use bevy::prelude::*;
use shared::sets::GameUpdateSet;

use crate::ui::menus::MenusPlugin;

pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MenusPlugin);
    }
}

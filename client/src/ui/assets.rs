use bevy::prelude::*;
use bevy_asset_loader::prelude::*;

use super::style::{
    CHAT_FONT_SIZE, MENU_FONT_SIZE, SECONDARY_FONT_SIZE, SECONDARY_TEXT_COLOR, TEXT_COLOR,
};

/// Button background variants (normal and dark, regular and large sizes)
#[derive(AssetCollection, Resource)]
pub struct ButtonAssets {
    #[asset(path = "graphics/button_background.png")]
    pub normal: Handle<Image>,

    #[asset(path = "graphics/button_background_large.png")]
    pub normal_large: Handle<Image>,

    #[asset(path = "graphics/dark_button_background.png")]
    pub dark: Handle<Image>,

    #[asset(path = "graphics/dark_button_background_large.png")]
    pub dark_large: Handle<Image>,
}

/// Asset collection for UI assets, loaded automatically via bevy_asset_loader.
/// This resource is available after `GameState::Splash` completes loading.
#[derive(AssetCollection, Resource)]
pub struct UiAssets {
    #[asset(path = "fonts/RustCraftRegular-Bmg3.otf")]
    pub font: Handle<Font>,

    #[asset(path = "graphics/play.png")]
    pub play_icon: Handle<Image>,

    #[asset(path = "graphics/trash.png")]
    pub trash_icon: Handle<Image>,

    #[asset(path = "graphics/background.png")]
    pub background: Handle<Image>,

    #[asset(path = "graphics/title.png")]
    pub title: Handle<Image>,
}

impl UiAssets {
    /// Creates a TextFont with the game's custom font at the specified size
    pub fn text_font(&self, font_size: f32) -> TextFont {
        TextFont {
            font: self.font.clone(),
            font_size,
            ..Default::default()
        }
    }

    /// Creates a TextFont for menu text (20px)
    pub fn menu_text_font(&self) -> TextFont {
        self.text_font(MENU_FONT_SIZE)
    }

    /// Creates a TextFont for chat text (17px)
    pub fn chat_text_font(&self) -> TextFont {
        self.text_font(CHAT_FONT_SIZE)
    }

    /// Creates a TextFont for secondary text (15px)
    pub fn secondary_text_font(&self) -> TextFont {
        self.text_font(SECONDARY_FONT_SIZE)
    }
}

/// Creates a white TextColor (most common text color)
pub fn white_text_color() -> TextColor {
    TextColor(TEXT_COLOR)
}

/// Creates a secondary TextColor (used for less prominent text)
pub fn secondary_text_color() -> TextColor {
    TextColor(SECONDARY_TEXT_COLOR)
}

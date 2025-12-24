use bevy::prelude::*;

use super::style::{
    CHAT_FONT_SIZE, MENU_FONT_SIZE, SECONDARY_FONT_SIZE, SECONDARY_TEXT_COLOR, TEXT_COLOR,
};

// Path to fonts
pub const FONT_PATH: &str = "./fonts/RustCraftRegular-Bmg3.otf";

// Path to icons
pub const PLAY_ICON_PATH: &str = "./graphics/play.png";
pub const TRASH_ICON_PATH: &str = "./graphics/trash.png";
pub const BACKGROUND_IMAGE_PATH: &str = "./graphics/background.png";
pub const BUTTON_BACKGROUND_IMAGE_PATH: &str = "./graphics/button_background.png";
pub const BUTTON_BACKGROUND_LARGE_IMAGE_PATH: &str = "./graphics/button_background_large.png";
pub const DARK_BUTTON_BACKGROUND_IMAGE_PATH: &str = "./graphics/dark_button_background.png";
pub const DARK_BUTTON_BACKGROUND_LARGE_IMAGE_PATH: &str =
    "./graphics/dark_button_background_large.png";
pub const TITLE_IMAGE_PATH: &str = "./graphics/title.png";

// Function to load the font asset
pub fn load_font(asset_server: &Res<AssetServer>) -> Handle<Font> {
    asset_server.load(FONT_PATH)
}

// Function to load common icons
pub fn load_play_icon(asset_server: &Res<AssetServer>) -> Handle<Image> {
    asset_server.load(PLAY_ICON_PATH)
}

pub fn load_trash_icon(asset_server: &Res<AssetServer>) -> Handle<Image> {
    asset_server.load(TRASH_ICON_PATH)
}

pub fn load_background_image(asset_server: &Res<AssetServer>) -> Handle<Image> {
    asset_server.load(BACKGROUND_IMAGE_PATH)
}

pub fn load_button_background_image(asset_server: &Res<AssetServer>) -> Handle<Image> {
    asset_server.load(BUTTON_BACKGROUND_IMAGE_PATH)
}

pub fn load_button_background_large_image(asset_server: &Res<AssetServer>) -> Handle<Image> {
    asset_server.load(BUTTON_BACKGROUND_LARGE_IMAGE_PATH)
}

pub fn load_dark_button_background_image(asset_server: &Res<AssetServer>) -> Handle<Image> {
    asset_server.load(DARK_BUTTON_BACKGROUND_IMAGE_PATH)
}

pub fn load_dark_button_background_large_image(asset_server: &Res<AssetServer>) -> Handle<Image> {
    asset_server.load(DARK_BUTTON_BACKGROUND_LARGE_IMAGE_PATH)
}

pub fn load_title_image(asset_server: &Res<AssetServer>) -> Handle<Image> {
    asset_server.load(TITLE_IMAGE_PATH)
}

/// Creates a TextFont with the game's custom font at the specified size
pub fn game_text_font(asset_server: &Res<AssetServer>, font_size: f32) -> TextFont {
    TextFont {
        font: load_font(asset_server),
        font_size,
        ..Default::default()
    }
}

/// Creates a TextFont for menu text (20px)
pub fn menu_text_font(asset_server: &Res<AssetServer>) -> TextFont {
    game_text_font(asset_server, MENU_FONT_SIZE)
}

/// Creates a TextFont for chat text (17px)
pub fn chat_text_font(asset_server: &Res<AssetServer>) -> TextFont {
    game_text_font(asset_server, CHAT_FONT_SIZE)
}

/// Creates a TextFont for secondary text (15px)
pub fn secondary_text_font(asset_server: &Res<AssetServer>) -> TextFont {
    game_text_font(asset_server, SECONDARY_FONT_SIZE)
}

/// Creates a white TextColor (most common text color)
pub fn white_text_color() -> TextColor {
    TextColor(TEXT_COLOR)
}

/// Creates a secondary TextColor (used for less prominent text)
pub fn secondary_text_color() -> TextColor {
    TextColor(SECONDARY_TEXT_COLOR)
}

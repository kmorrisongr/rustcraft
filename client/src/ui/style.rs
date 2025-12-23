use bevy::prelude::*;
use bevy::ui::{AlignItems, Display, FlexDirection, JustifyContent, Node, UiRect, Val};

// Common styles for buttons
pub const NORMAL_BUTTON: Color = Color::srgb(0.3, 0.3, 0.3);
pub const HOVERED_BUTTON: Color = Color::srgb(0.4, 0.4, 0.4);
// pub const HOVERED_PRESSED_BUTTON: Color = Color::srgb(0.25, 0.65, 0.25);
pub const PRESSED_BUTTON: Color = Color::srgb(0.2, 0.2, 0.2);

// Common background colors
pub const BACKGROUND_COLOR: Color = Color::srgb(0.5, 0.5, 0.5);
// pub const BUTTON_BORDER_COLOR: Color = Color::BLACK;

// Common text colors
pub const TEXT_COLOR: Color = Color::WHITE;
pub const SECONDARY_TEXT_COLOR: Color = Color::srgb(0.4, 0.4, 0.4);

/// Default font size for menu text
pub const MENU_FONT_SIZE: f32 = 20.0;
/// Default font size for chat text
pub const CHAT_FONT_SIZE: f32 = 17.0;

// Button styles
pub fn big_button_style() -> Node {
    Node {
        width: Val::Px(400.0),
        height: Val::Px(60.0),
        margin: UiRect::all(Val::Px(20.0)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..Default::default()
    }
}

// Text styles
pub fn text_font(font: Handle<Font>, font_size: f32) -> TextFont {
    TextFont {
        font,
        font_size,
        ..Default::default()
    }
}

pub fn background_image_style() -> Node {
    Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        flex_direction: FlexDirection::Column,
        ..Default::default()
    }
}

/// Common button style used in menu lists (solo, multi, etc.)
/// Creates a centered flex column button with border
pub fn menu_list_button_style() -> Node {
    Node {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(2.)),
        height: Val::Px(40.0),
        ..Default::default()
    }
}

/// Button style for icon buttons in list items (play, delete, etc.)
pub fn icon_button_style() -> Node {
    Node {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(2.)),
        height: Val::Percent(80.),
        ..Default::default()
    }
}

/// Style for icon images within buttons
pub fn icon_image_style() -> Node {
    Node {
        height: Val::Percent(100.),
        ..Default::default()
    }
}

/// Common style for list item rows (server entries, world entries)
pub fn list_item_row_style() -> Node {
    Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(5.),
        width: Val::Percent(100.),
        height: Val::Vh(10.),
        padding: UiRect::horizontal(Val::Percent(2.)),
        border: UiRect::all(Val::Px(2.)),
        ..Default::default()
    }
}

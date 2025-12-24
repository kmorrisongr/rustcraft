//! Common UI components for list items (servers, worlds, etc.)
//!
//! This module provides reusable builders for creating consistent list item UI elements
//! used in menus like the server list and world list.

use bevy::prelude::*;

use super::{
    assets::{load_play_icon, load_trash_icon, menu_text_font, secondary_text_color, secondary_text_font, white_text_color},
    style::{icon_button_style, icon_image_style, list_item_row_style, BACKGROUND_COLOR},
};

/// Configuration for building a list item row
pub struct ListItemConfig<'a> {
    pub asset_server: &'a Res<'a, AssetServer>,
    pub primary_text: &'a str,
    pub secondary_text: Option<&'a str>,
}

/// Result of spawning a list item, containing entity IDs for further customization
pub struct ListItemEntities {
    pub row: Entity,
    pub play_button: Entity,
    pub delete_button: Entity,
    #[allow(dead_code)]
    pub text: Entity,
}

/// Spawns a list item row with play and delete buttons
///
/// This creates a consistent UI pattern used for both server list and world list items.
/// The caller is responsible for:
/// - Adding the appropriate action components to play_button and delete_button
/// - Adding the row to the parent list container
/// - Storing the row entity in the list's HashMap
pub fn spawn_list_item_row(commands: &mut Commands, config: ListItemConfig) -> ListItemEntities {
    let row = commands
        .spawn((BorderColor(BACKGROUND_COLOR), list_item_row_style()))
        .id();

    let play_btn = commands
        .spawn((Button, icon_button_style()))
        .with_children(|btn| {
            let icon = load_play_icon(config.asset_server);
            btn.spawn((ImageNode::new(icon), icon_image_style()));
        })
        .id();

    let delete_btn = commands
        .spawn((Button, icon_button_style()))
        .with_children(|btn| {
            let icon = load_trash_icon(config.asset_server);
            btn.spawn((ImageNode::new(icon), icon_image_style()));
        })
        .id();

    let txt = commands
        .spawn((
            Text::new(format!("{}\n", config.primary_text)),
            menu_text_font(config.asset_server),
            white_text_color(),
            Node {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
        ))
        .id();

    // Spawn secondary text if provided
    let secondary_txt = if let Some(secondary) = config.secondary_text {
        Some(
            commands
                .spawn((
                    Text::new(secondary),
                    secondary_text_font(config.asset_server),
                    secondary_text_color(),
                ))
                .id(),
        )
    } else {
        None
    };

    let mut children = vec![play_btn, delete_btn, txt];
    if let Some(sec) = secondary_txt {
        children.push(sec);
    }

    commands.entity(row).add_children(&children);

    ListItemEntities {
        row,
        play_button: play_btn,
        delete_button: delete_btn,
        text: txt,
    }
}

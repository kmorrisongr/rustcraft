use bevy::prelude::*;

use crate::ui::assets::UiAssets;
use crate::ui::style::{background_image_style, big_button_style, text_font, NORMAL_BUTTON};
use crate::TEXT_COLOR;

use super::{MenuButtonAction, MenuState};

pub fn home_setup(mut commands: Commands, ui_assets: Res<UiAssets>) {
    let button_text_color = TextColor(TEXT_COLOR);
    let button_text_font = text_font(ui_assets.font.clone(), 33.0);

    // Main container for the menu
    commands
        .spawn((
            (
                background_image_style(),
                BackgroundColor(Color::NONE),
                StateScoped(MenuState::Main),
            ),
            ImageNode::new(ui_assets.background.clone()), // Set the background image
        ))
        .with_children(|parent| {
            // Display the game title as an image
            let image_width = 800.0;
            let aspect_ratio = 601.0 / 94.0;
            let image_height = image_width / aspect_ratio;

            parent.spawn((
                Node {
                    margin: UiRect::bottom(Val::Px(120.0)), // Add space below the title image
                    width: Val::Px(image_width),
                    height: Val::Px(image_height),
                    ..default()
                },
                ImageNode::new(ui_assets.title.clone()),
            ));

            // Add buttons for each action available in the menu
            for (action, label) in [
                (MenuButtonAction::Solo, "Singleplayer"),
                (MenuButtonAction::Multi, "Multiplayer"),
                (MenuButtonAction::Settings, "Settings"),
                (MenuButtonAction::Quit, "Quit"),
            ] {
                parent
                    .spawn((
                        (
                            Button,
                            big_button_style(), // Use large button style
                            BackgroundColor(NORMAL_BUTTON),
                            ImageNode::new(ui_assets.button_background.clone()),
                        ),
                        action,
                    ))
                    .with_children(|parent| {
                        parent.spawn((
                            Text::new(label),
                            button_text_font.clone(),
                            button_text_color,
                        ));
                    });
            }
        });
}

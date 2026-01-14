use bevy::prelude::*;
use bevy_asset_loader::prelude::*;

use crate::ui::assets::UiAssets;
use crate::GameState;

// This plugin will display a splash screen with Bevy logo while loading UI assets
pub fn splash_plugin(app: &mut App) {
    app
        // Configure bevy_asset_loader to load UI assets during Splash state
        .add_loading_state(
            LoadingState::new(GameState::Splash)
                .continue_to_state(GameState::Menu)
                .load_collection::<UiAssets>(),
        )
        // When entering the state, spawn everything needed for this screen
        .add_systems(OnEnter(GameState::Splash), splash_setup);
}

fn splash_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Display the logo
    let icon = asset_server.load("./graphics/bevy_icon.png");

    commands
        .spawn((
            (Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            }),
            StateScoped(GameState::Splash),
        ))
        .with_children(|parent| {
            parent.spawn((
                Node {
                    // This will set the logo to be 200px wide, and auto adjust its height
                    width: Val::Px(200.0),
                    ..default()
                },
                ImageNode::new(icon),
            ));
        });
}

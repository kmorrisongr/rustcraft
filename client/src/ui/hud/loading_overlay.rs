use bevy::prelude::*;
use shared::players::Player;

use crate::{player::CurrentPlayerMarker, GameState};

/// Marker component for the loading overlay UI
#[derive(Component)]
pub struct LoadingOverlay;

/// Spawns the loading overlay UI (hidden by default)
pub fn setup_loading_overlay(mut commands: Commands) {
    commands
        .spawn((
            StateScoped(GameState::Game),
            LoadingOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            // Loading text
            parent.spawn((
                Text::new("Loading terrain..."),
                TextFont {
                    font_size: 32.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            // Subtitle
            parent.spawn((
                Text::new("Please wait while chunks generate"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgba(0.8, 0.8, 0.8, 1.0)),
                Node {
                    margin: UiRect::top(Val::Px(10.0)),
                    ..default()
                },
            ));
        });
}

/// Shows/hides the loading overlay based on player's gravity_enabled state
pub fn update_loading_overlay(
    player_query: Query<&Player, With<CurrentPlayerMarker>>,
    mut overlay_query: Query<&mut Visibility, With<LoadingOverlay>>,
) {
    let Ok(player) = player_query.single() else {
        return;
    };

    let Ok(mut visibility) = overlay_query.single_mut() else {
        return;
    };

    // Show overlay when gravity is disabled (chunks not loaded yet)
    *visibility = if player.gravity_enabled {
        Visibility::Hidden
    } else {
        Visibility::Visible
    };
}

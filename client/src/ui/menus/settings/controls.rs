use bevy::{
    color::{palettes::css, Color},
    input::ButtonInput,
    prelude::*,
    ui::{
        AlignItems, BackgroundColor, BorderColor, BorderRadius, Display, FlexDirection,
        FocusPolicy, Interaction, JustifyContent, Node, Overflow, PositionType, RepeatedGridTrack,
        UiRect, Val,
    },
    utils::default,
};
use leafwing_input_manager::prelude::*;
use shared::GameFolderPaths;

use crate::input::action_state::GlobalInputManager;
use crate::input::data::GameAction;
use crate::menus::{MenuButtonAction, MenuState, ScrollingList};

use crate::ui::assets::UiAssets;
use crate::ui::style::NORMAL_BUTTON;

#[derive(Debug, Component, PartialEq, Eq)]
pub struct ClearButton(GameAction, Entity);

#[derive(Component, Debug, PartialEq, Eq)]
pub struct EditControlButton(GameAction);

#[derive(Component)]
pub struct ActionRecorder {
    pub action: GameAction,
    pub entity: Entity,
}

pub fn controls_menu_setup(
    mut commands: Commands,
    ui_assets: Res<UiAssets>,
    input_map_query: Query<&InputMap<GameAction>, With<GlobalInputManager>>,
    paths: Res<GameFolderPaths>,
) {
    let input_map = input_map_query
        .single()
        .expect("GlobalInputManager should exist");

    let trash_icon = ui_assets.trash_icon.clone();

    commands
        .spawn((
            DespawnOnExit(MenuState::SettingsControls),
            (
                Node {
                    padding: UiRect::horizontal(Val::Vw(15.)),
                    top: Val::Px(0.),
                    display: Display::Flex,
                    width: Val::Vw(100.),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: Val::Px(10.),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ),
            ImageNode::new(ui_assets.background.clone()),
        ))
        .with_children(|root| {
            let placeholder = root
                .spawn((
                    (
                        Button,
                        GlobalZIndex(3),
                        Node {
                            position_type: PositionType::Absolute,
                            top: Val::Px(10.),
                            left: Val::Px(10.),
                            padding: UiRect::all(Val::Px(5.)),
                            ..default()
                        },
                    ),
                    MenuButtonAction::BackToSettings,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Back"),
                        TextFont {
                            font: ui_assets.font.clone(),
                            font_size: 21.,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                })
                .id();

            root.spawn((Node {
                overflow: Overflow::clip_y(),
                height: Val::Vh(100.),
                width: Val::Vw(60.),
                flex_direction: FlexDirection::Column,
                ..default()
            },))
                .with_children(|wrapper| {
                    wrapper
                        .spawn((
                            (Node {
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                width: Val::Percent(100.),
                                ..default()
                            },),
                            ScrollingList { position: 0. },
                        ))
                        .with_children(|list| {
                            list.spawn((
                                Text::new("Keyboard Controls"),
                                TextFont {
                                    font: ui_assets.font.clone(),
                                    font_size: 36.,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                Node {
                                    margin: UiRect::vertical(Val::Px(20.)),
                                    ..default()
                                },
                            ));
                            // Iterate over all buttonlike actions in the InputMap
                            for (action, bindings) in input_map.iter_buttonlike() {
                                // Convert Box<dyn Buttonlike> to KeyCode for display
                                let keys: Vec<KeyCode> = bindings
                                    .iter()
                                    .filter_map(|b| {
                                        // Try to downcast to KeyCode
                                        b.as_any().downcast_ref::<KeyCode>().copied()
                                    })
                                    .collect();

                                list.spawn((
                                    (
                                        Button,
                                        BorderColor::all(Color::srgb(0.3, 0.3, 0.3)),
                                        Node {
                                            display: Display::Grid,
                                            width: Val::Percent(100.),
                                            height: Val::Auto,
                                            align_items: AlignItems::Center,
                                            justify_content: JustifyContent::Center,
                                            grid_template_columns: vec![
                                                RepeatedGridTrack::flex(2, 1.),
                                                RepeatedGridTrack::px(1, 40.),
                                            ],
                                            border: UiRect::bottom(Val::Px(2.5)),
                                            ..default()
                                        },
                                    ),
                                    EditControlButton(*action),
                                ))
                                .with_children(|line| {
                                    line.spawn((
                                        Text::new(format!("{action:?}")),
                                        TextFont {
                                            font: ui_assets.font.clone(),
                                            font_size: 24.,
                                            ..default()
                                        },
                                        TextColor(Color::WHITE),
                                        Node {
                                            margin: UiRect::all(Val::Px(10.)),
                                            ..default()
                                        },
                                    ));

                                    let mut component = line.spawn(Node {
                                        flex_direction: FlexDirection::RowReverse,
                                        column_gap: Val::Px(15.),
                                        margin: UiRect::horizontal(Val::Px(10.)),
                                        ..default()
                                    });

                                    let id = component.id();

                                    update_input_component(
                                        &mut component.commands(),
                                        id,
                                        &keys,
                                        &ui_assets,
                                        &paths,
                                    );

                                    line.spawn((
                                        (
                                            Button,
                                            BorderRadius::all(Val::Percent(25.)),
                                            FocusPolicy::Pass,
                                            Node {
                                                align_items: AlignItems::Center,
                                                justify_content: JustifyContent::Center,
                                                width: Val::Percent(80.),
                                                padding: UiRect::all(Val::Px(5.)),
                                                ..default()
                                            },
                                        ),
                                        ClearButton(*action, id),
                                    ))
                                    .with_children(|btn| {
                                        btn.spawn((
                                            ImageNode::new(trash_icon.clone()),
                                            Node {
                                                width: Val::Percent(100.),
                                                ..default()
                                            },
                                        ));
                                    });
                                });
                            }
                        });
                });

            root.spawn((
                (
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Vw(100.),
                        height: Val::Vh(100.),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    Visibility::Hidden,
                    FocusPolicy::Block,
                    GlobalZIndex(2),
                ),
                ActionRecorder {
                    action: GameAction::Escape,
                    entity: placeholder,
                },
            ))
            .with_children(|wrapper| {
                wrapper
                    .spawn((
                        BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
                        BorderColor::all(Color::Srgba(css::BLUE_VIOLET)),
                        Node {
                            border: UiRect::all(Val::Px(2.5)),
                            min_width: Val::Vw(50.),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            padding: UiRect::all(Val::Px(10.)),
                            ..default()
                        },
                    ))
                    .with_children(|dialog| {
                        dialog.spawn((
                            Text::new("Press any key..."),
                            TextFont {
                                font: ui_assets.font.clone(),
                                font_size: 21.,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                            Node {
                                margin: UiRect::all(Val::Px(25.)),
                                width: Val::Auto,
                                ..default()
                            },
                        ));
                    });
            });
        });
}

pub fn update_input_component(
    commands: &mut Commands,
    entity: Entity,
    binds: &Vec<KeyCode>,
    ui_assets: &Res<UiAssets>,
    _paths: &Res<GameFolderPaths>,
) {
    commands.entity(entity).despawn_related::<Children>();

    // List all possible binds, and add them as text elements
    for key in binds {
        let child = commands
            .spawn(((
                BackgroundColor(Color::Srgba(css::BLUE_VIOLET)),
                BorderRadius::all(Val::Px(10.)),
                Node {
                    padding: UiRect::horizontal(Val::Px(10.)),
                    ..default()
                },
            ),))
            .with_children(|k| {
                k.spawn((
                    Text::new({
                        // Formats keybindings
                        let mut output = format!("{key:?}").replace("Key", "");
                        if output.starts_with("Arrow") {
                            if output.ends_with("Left") {
                                output = "←".into()
                            }
                            if output.ends_with("Right") {
                                output = "→".into()
                            }
                            if output.ends_with("Up") {
                                output = "↑".into()
                            }
                            if output.ends_with("Down") {
                                output = "↓".into()
                            }
                        }
                        output
                    }),
                    TextFont {
                        font: ui_assets.font.clone(),
                        font_size: 21.,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            })
            .id();

        commands.entity(entity).add_child(child);
    }
}

pub fn controls_update_system(
    queries: (
        Query<(&Interaction, &EditControlButton, &Children, &mut Node), Changed<Interaction>>,
        Query<(&Interaction, &ClearButton, &mut BackgroundColor), Changed<Interaction>>,
        Query<(&mut ActionRecorder, &mut Visibility)>,
    ),
    mut commands: Commands,
    resources: (Res<UiAssets>, Res<ButtonInput<KeyCode>>),
    mut input_map_query: Query<&mut InputMap<GameAction>, With<GlobalInputManager>>,
    paths: Res<GameFolderPaths>,
) {
    let (mut edit_query, mut clear_query, mut visibility_query) = queries;
    let (ui_assets, input) = resources;

    if visibility_query.is_empty() {
        return;
    }

    let (mut recorder, mut vis) = visibility_query.single_mut().unwrap();

    if *vis == Visibility::Visible {
        if let Some(btn) = input.get_just_pressed().next() {
            *vis = Visibility::Hidden;

            // Add the new key binding to the InputMap
            if let Ok(mut input_map) = input_map_query.single_mut() {
                input_map.insert(recorder.action, *btn);

                // Get updated bindings for display
                let keys: Vec<KeyCode> = input_map
                    .get_buttonlike(&recorder.action)
                    .map(|bindings| {
                        bindings
                            .iter()
                            .filter_map(|b| b.as_any().downcast_ref::<KeyCode>().copied())
                            .collect()
                    })
                    .unwrap_or_default();

                update_input_component(&mut commands, recorder.entity, &keys, &ui_assets, &paths);
            }
            return;
        }
    }

    for (interaction, btn_action, children, mut style) in edit_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                let kbd_action = btn_action.0;
                // Check if "Clear" button received an event. If so, ignores input
                if clear_query.get(children[2]).is_ok() {
                    continue;
                }
                // Open the "add input" dialog
                *vis = Visibility::Visible;
                recorder.action = kbd_action;
                recorder.entity = children[1];
            }
            Interaction::Hovered => {
                // Show "Clear" button
                style.grid_template_columns.pop();
                style
                    .grid_template_columns
                    .push(RepeatedGridTrack::px(1, 40.));
            }
            Interaction::None => {
                // Hide "clear button"
                style.grid_template_columns.pop();
                style
                    .grid_template_columns
                    .push(RepeatedGridTrack::px(1, 10.));
            }
        }
    }

    for (interaction, clear, mut bg) in clear_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                // Clear all binds for this action
                if let Ok(mut input_map) = input_map_query.single_mut() {
                    input_map.clear_action(&clear.0);

                    // Update visual element with empty bindings
                    update_input_component(&mut commands, clear.1, &Vec::new(), &ui_assets, &paths);
                }
            }
            Interaction::Hovered => {
                bg.0 = Color::Srgba(css::RED);
            }
            Interaction::None => {
                bg.0 = NORMAL_BUTTON;
            }
        }
    }
}

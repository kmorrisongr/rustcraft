use crate::ui::hud::UIMode;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

pub fn handle_mouse_system(
    mut primary_cursor_options: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    ui_mode: Res<UIMode>,
) {
    let mut window = windows.single_mut().unwrap();

    let is_playing = *ui_mode == UIMode::Closed;

    primary_cursor_options.grab_mode = if is_playing {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };

    primary_cursor_options.visible = !is_playing;
}

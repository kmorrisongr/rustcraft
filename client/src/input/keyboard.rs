use crate::{constants::BINDS_PATH, input::data::GameAction, KeyMap};
use bevy::prelude::*;
use bevy::{
    input::ButtonInput,
    prelude::{KeyCode, Res},
};
use ron::{from_str, ser::PrettyConfig};
use shared::GameFolderPaths;
use std::path::Path;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

fn write_keybindings_to_path(key_map: &KeyMap, binds_path: &Path) -> Result<(), std::io::Error> {
    let pretty_config = PrettyConfig::new()
        .with_depth_limit(3)
        .with_separate_tuple_members(true)
        .with_enumerate_arrays(true);

    let serialized = ron::ser::to_string_pretty(key_map, pretty_config)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "serialization failed"))?;
    if let Some(parent) = binds_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(binds_path)?;
    file.write_all(serialized.as_bytes())
}

pub fn is_action_pressed(
    action: GameAction,
    keyboard_input: &ButtonInput<KeyCode>,
    key_map: &KeyMap,
) -> bool {
    if let Some(key_codes) = key_map.map.get(&action) {
        for key_code in key_codes {
            if keyboard_input.pressed(*key_code) {
                return true;
            }
        }
    }
    false
}

pub fn is_action_just_pressed(
    action: GameAction,
    keyboard_input: &ButtonInput<KeyCode>,
    key_map: &KeyMap,
) -> bool {
    if let Some(key_codes) = key_map.map.get(&action) {
        for key_code in key_codes {
            if keyboard_input.just_pressed(*key_code) {
                return true;
            }
        }
    }
    false
}

pub fn is_action_just_released(
    action: GameAction,
    keyboard_input: &ButtonInput<KeyCode>,
    key_map: &KeyMap,
) -> bool {
    if let Some(key_codes) = key_map.map.get(&action) {
        for key_code in key_codes {
            if keyboard_input.just_released(*key_code) {
                return true;
            }
        }
    }
    false
}

pub fn get_action_keys(action: GameAction, key_map: &KeyMap) -> Vec<KeyCode> {
    key_map.map.get(&action).unwrap().to_vec()
}

pub(crate) fn default_key_map() -> BTreeMap<GameAction, Vec<KeyCode>> {
    let mut map = BTreeMap::new();
    map.insert(GameAction::MoveForward, vec![KeyCode::KeyW, KeyCode::ArrowUp]);
    map.insert(
        GameAction::MoveBackward,
        vec![KeyCode::KeyS, KeyCode::ArrowDown],
    );
    map.insert(GameAction::MoveLeft, vec![KeyCode::KeyA, KeyCode::ArrowLeft]);
    map.insert(
        GameAction::MoveRight,
        vec![KeyCode::KeyD, KeyCode::ArrowRight],
    );
    map.insert(GameAction::Jump, vec![KeyCode::Space]);
    map.insert(GameAction::Escape, vec![KeyCode::Escape]);
    map.insert(GameAction::ToggleFps, vec![KeyCode::F3]);
    map.insert(GameAction::ToggleChunkDebugMode, vec![KeyCode::F4]);
    map.insert(GameAction::ToggleViewMode, vec![KeyCode::F5]);
    map.insert(
        GameAction::ToggleBlockWireframeDebugMode,
        vec![KeyCode::F6],
    );
    map.insert(GameAction::ToggleRaycastDebugMode, vec![KeyCode::F7]);
    map.insert(GameAction::ToggleFlyMode, vec![KeyCode::KeyF]);
    map.insert(GameAction::FlyUp, vec![KeyCode::Space]);
    map.insert(GameAction::FlyDown, vec![KeyCode::ShiftLeft]);
    map.insert(GameAction::ToggleInventory, vec![KeyCode::KeyE]);
    map.insert(GameAction::OpenChat, vec![KeyCode::KeyT]);
    map.insert(GameAction::RenderDistanceMinus, vec![KeyCode::KeyO]);
    map.insert(GameAction::RenderDistancePlus, vec![KeyCode::KeyP]);
    map.insert(GameAction::ReloadChunks, vec![KeyCode::KeyR]);
    map
}

pub fn get_bindings(game_folder_paths: &GameFolderPaths) -> KeyMap {
    let binds_path: PathBuf = Path::new(&game_folder_paths.assets_folder_path).join(BINDS_PATH);

    if let Ok(content) = fs::read_to_string(binds_path.as_path())
        && let Ok(key_map) = from_str::<KeyMap>(&content)
    {
        return key_map;
    }

    let key_map = KeyMap::default();
    if let Err(e) = write_keybindings_to_path(&key_map, binds_path.as_path()) {
        error!(
            "Failed to create default keybindings file at {:?}: {}",
            binds_path, e
        );
    }
    key_map
}

pub fn save_keybindings(key_map: Res<KeyMap>, game_folder_path: Res<GameFolderPaths>) {
    let binds_path = game_folder_path.assets_folder_path.join(BINDS_PATH);
    match write_keybindings_to_path(key_map.into_inner(), &binds_path) {
        Ok(_) => info!("Keybindings successfully saved to {:?}", binds_path),
        Err(e) => error!("Failed to save keybindings to {:?}: {}", binds_path, e),
    }
}

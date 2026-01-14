use crate::{constants::BINDS_PATH, input::data::GameAction};
use bevy::prelude::*;
use leafwing_input_manager::prelude::*;
use ron::{from_str, ser::PrettyConfig};
use shared::GameFolderPaths;
use std::path::Path;
use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

fn write_keybindings_to_path(
    input_map: &InputMap<GameAction>,
    binds_path: &Path,
) -> Result<(), std::io::Error> {
    let pretty_config = PrettyConfig::new()
        .with_depth_limit(4)
        .with_separate_tuple_members(true)
        .with_enumerate_arrays(true);

    let serialized = ron::ser::to_string_pretty(input_map, pretty_config).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("serialization failed: {e}"),
        )
    })?;
    if let Some(parent) = binds_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(binds_path)?;
    file.write_all(serialized.as_bytes())
}

/// Creates the default InputMap with all game keybindings.
pub fn default_input_map() -> InputMap<GameAction> {
    InputMap::default()
        .with_one_to_many(GameAction::MoveForward, [KeyCode::KeyW, KeyCode::ArrowUp])
        .with_one_to_many(
            GameAction::MoveBackward,
            [KeyCode::KeyS, KeyCode::ArrowDown],
        )
        .with_one_to_many(GameAction::MoveLeft, [KeyCode::KeyA, KeyCode::ArrowLeft])
        .with_one_to_many(GameAction::MoveRight, [KeyCode::KeyD, KeyCode::ArrowRight])
        .with(GameAction::Jump, KeyCode::Space)
        .with(GameAction::Escape, KeyCode::Escape)
        .with(GameAction::ToggleFps, KeyCode::F3)
        .with(GameAction::ToggleChunkDebugMode, KeyCode::F4)
        .with(GameAction::ToggleViewMode, KeyCode::F5)
        .with(GameAction::ToggleBlockWireframeDebugMode, KeyCode::F6)
        .with(GameAction::ToggleRaycastDebugMode, KeyCode::F7)
        .with(GameAction::ToggleFlyMode, KeyCode::KeyF)
        .with(GameAction::FlyUp, KeyCode::Space)
        .with(GameAction::FlyDown, KeyCode::ShiftLeft)
        .with(GameAction::ToggleInventory, KeyCode::KeyE)
        .with(GameAction::OpenChat, KeyCode::KeyT)
        .with(GameAction::RenderDistanceMinus, KeyCode::KeyO)
        .with(GameAction::RenderDistancePlus, KeyCode::KeyP)
        .with(GameAction::ReloadChunks, KeyCode::KeyR)
}

/// Loads keybindings from the config file, or creates defaults if missing.
pub fn get_bindings(game_folder_paths: &GameFolderPaths) -> InputMap<GameAction> {
    let binds_path: PathBuf = Path::new(&game_folder_paths.assets_folder_path).join(BINDS_PATH);

    if let Ok(content) = fs::read_to_string(binds_path.as_path()) {
        match from_str::<InputMap<GameAction>>(&content) {
            Ok(input_map) => return input_map,
            Err(e) => warn!(
                "Failed to deserialize keybindings at {:?}, writing defaults: {}",
                binds_path, e
            ),
        }
    }

    let input_map = default_input_map();
    if let Err(e) = write_keybindings_to_path(&input_map, binds_path.as_path()) {
        error!(
            "Failed to create default keybindings file at {:?}: {}",
            binds_path, e
        );
    }
    input_map
}

/// Saves the current keybindings to the config file.
pub fn save_keybindings(
    query: Query<&InputMap<GameAction>, With<crate::input::action_state::GlobalInputManager>>,
    game_folder_path: Res<GameFolderPaths>,
) {
    let binds_path = game_folder_path.assets_folder_path.join(BINDS_PATH);
    if let Ok(input_map) = query.single() {
        match write_keybindings_to_path(input_map, &binds_path) {
            Ok(_) => info!("Keybindings successfully saved to {:?}", binds_path),
            Err(e) => error!("Failed to save keybindings to {:?}: {}", binds_path, e),
        }
    }
}

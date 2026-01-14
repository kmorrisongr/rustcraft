use crate::input::data::GameAction;
use bevy::prelude::*;
use leafwing_input_manager::prelude::*;
use std::collections::BTreeMap;

/// Marker component for the global input manager entity.
/// This entity holds the InputMap and ActionState for global game inputs.
#[derive(Component)]
pub struct GlobalInputManager;

/// Converts a BTreeMap keybinding format to an InputMap.
/// Used when loading keybindings from settings.
pub fn input_map_from_btree(map: &BTreeMap<GameAction, Vec<KeyCode>>) -> InputMap<GameAction> {
    let mut input_map = InputMap::default();

    for (action, keys) in map {
        for key in keys {
            input_map.insert(*action, *key);
        }
    }

    input_map
}

/// Spawns the global input manager entity that will hold the ActionState.
pub fn spawn_global_input_manager(mut commands: Commands, key_map: Res<crate::KeyMap>) {
    let input_map = input_map_from_btree(&key_map.map);

    commands.spawn((
        GlobalInputManager,
        Name::new("GlobalInputManager"),
        input_map,
        ActionState::<GameAction>::default(),
    ));
}

/// Syncs the InputMap component with changes from the KeyMap resource.
/// Call this after modifying keybindings in the settings UI.
pub fn sync_input_map_from_keymap(
    key_map: Res<crate::KeyMap>,
    mut query: Query<&mut InputMap<GameAction>, With<GlobalInputManager>>,
) {
    if !key_map.is_changed() {
        return;
    }

    if let Ok(mut input_map) = query.single_mut() {
        *input_map = input_map_from_btree(&key_map.map);
        info!("InputMap synced with KeyMap changes");
    }
}

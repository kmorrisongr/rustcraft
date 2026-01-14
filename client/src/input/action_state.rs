use crate::input::data::GameAction;
use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

/// Marker component for the global input manager entity.
/// This entity holds the InputMap and ActionState for global game inputs.
#[derive(Component)]
pub struct GlobalInputManager;

/// Spawns the global input manager entity that will hold the ActionState.
pub fn spawn_global_input_manager(
    mut commands: Commands,
    loaded_input_map: Res<crate::LoadedInputMap>,
) {
    commands.spawn((
        GlobalInputManager,
        Name::new("GlobalInputManager"),
        loaded_input_map.0.clone(),
        ActionState::<GameAction>::default(),
    ));
}

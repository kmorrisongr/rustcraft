use crate::input::data::GameAction;
use crate::input::GlobalInputManager;
use crate::world::materials::MaterialResource;
use crate::world::GlobalMaterial;
use bevy::pbr::wireframe::WireframeConfig;
use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

#[derive(Resource, Default)]
pub struct BlockDebugWireframeSettings {
    pub is_enabled: bool,
}

pub fn toggle_wireframe_system(
    action_query: Query<&ActionState<GameAction>, With<GlobalInputManager>>,
    mut settings: ResMut<BlockDebugWireframeSettings>,
    mut config: ResMut<WireframeConfig>,
    material_resource: ResMut<MaterialResource>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(action_state) = action_query.single() else {
        return;
    };

    if action_state.just_pressed(&GameAction::ToggleBlockWireframeDebugMode) && !settings.is_enabled
    {
        settings.is_enabled = true;
        config.global = true;
        let handle = material_resource
            .global_materials
            .get(&GlobalMaterial::Blocks)
            .unwrap();
        let material = materials.get_mut(handle).unwrap();
        material.alpha_mode = AlphaMode::Blend;
        material.base_color.set_alpha(0.3);
        return;
    }

    if action_state.just_pressed(&GameAction::ToggleBlockWireframeDebugMode) {
        settings.is_enabled = false;
        config.global = false;
        let handle = material_resource
            .global_materials
            .get(&GlobalMaterial::Blocks)
            .unwrap();
        let material = materials.get_mut(handle).unwrap();
        material.alpha_mode = AlphaMode::AlphaToCoverage;
        material.base_color.set_alpha(1.0);
    }
}

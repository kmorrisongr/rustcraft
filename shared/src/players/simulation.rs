use crate::{
    messages::PlayerFrameInput,
    physics::simulate_player_movement_rapier,
    players::{
        blocks::{simulate_player_block_interactions, CallerType},
        Player,
    },
    world::WorldMap,
};

pub fn simulate_player_actions(
    player: &mut Player,
    world_map: &mut impl WorldMap,
    action: &PlayerFrameInput,
    caller_type: CallerType,
) {
    // if !action.inputs.is_empty() {
    // debug!(
    //     "Simulating player actions for player {} -> {:?}",
    //     player.id, action
    // );
    // }

    // debug!("Camera = {:?}", action.camera);
    // debug!("Hotbar slot = {:?}", action.hotbar_slot);
    // debug!("Player position before = {:?}", player.position);
    // debug!("Player view mode = {:?}", action.view_mode);

    simulate_player_block_interactions(player, world_map, action, caller_type);
    simulate_player_movement_rapier(player, world_map, action);
}

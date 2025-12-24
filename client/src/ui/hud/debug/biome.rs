use crate::player::CurrentPlayerMarker;
use bevy::prelude::*;
use shared::world::{calculate_biome_at_position, WorldSeed};

#[derive(Component)]
pub struct BiomeText;

/// System to update the biome text based on the player's current position
pub fn biome_text_update_system(
    player: Query<&Transform, With<CurrentPlayerMarker>>,
    query: Query<Entity, With<BiomeText>>,
    mut writer: TextUiWriter,
    world_seed: Res<WorldSeed>,
) {
    let Ok(player_transform) = player.single() else {
        return;
    };

    let biome_type = calculate_biome_at_position(
        player_transform.translation.x as i32,
        player_transform.translation.z as i32,
        world_seed.0,
    );

    for entity in query.iter() {
        *writer.text(entity, 0) = format!("Biome: {}", biome_type.name());
    }
}

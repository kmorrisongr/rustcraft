use crate::player::CurrentPlayerMarker;
use bevy::prelude::*;
use shared::world::{block_to_chunk_coord, calculate_biome_at_position, WorldSeed};

#[derive(Component)]
pub struct BiomeText;

/// Component to track a player's last known chunk position for biome updates
/// Attached to each player entity to ensure per-player tracking
#[derive(Component, Default)]
pub struct LastBiomeChunk {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub biome_name: String,
}

/// System to update the biome text based on the player's current position
/// Only recalculates when the player enters a new chunk to optimize performance
pub fn biome_text_update_system(
    mut player: Query<(&Transform, &mut LastBiomeChunk), With<CurrentPlayerMarker>>,
    query: Query<Entity, With<BiomeText>>,
    mut writer: TextUiWriter,
    world_seed: Res<WorldSeed>,
) {
    let Ok((player_transform, mut last_chunk)) = player.single_mut() else {
        return;
    };

    // Calculate player's current chunk position
    let current_chunk_x = block_to_chunk_coord(player_transform.translation.x as i32);
    let current_chunk_z = block_to_chunk_coord(player_transform.translation.z as i32);

    // Only recalculate biome if player has moved to a new chunk
    if current_chunk_x != last_chunk.chunk_x || current_chunk_z != last_chunk.chunk_z {
        let biome_type = calculate_biome_at_position(
            player_transform.translation.x as i32,
            player_transform.translation.z as i32,
            world_seed.0,
        );

        last_chunk.chunk_x = current_chunk_x;
        last_chunk.chunk_z = current_chunk_z;
        last_chunk.biome_name = biome_type.name().to_string();
    }

    // Update the UI text (this is cheap, just a string copy)
    for entity in query.iter() {
        *writer.text(entity, 0) = format!("Biome: {}", last_chunk.biome_name);
    }
}

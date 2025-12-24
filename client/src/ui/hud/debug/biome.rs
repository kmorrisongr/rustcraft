use crate::player::CurrentPlayerMarker;
use bevy::prelude::*;
use noise::{NoiseFn, Perlin};
use shared::world::{determine_biome, BiomeType, WorldSeed};

#[derive(Component)]
pub struct BiomeText;

/// System to update the biome text based on the player's current position
pub fn biome_text_update_system(
    player: Query<&Transform, With<CurrentPlayerMarker>>,
    query: Query<Entity, With<BiomeText>>,
    mut writer: TextUiWriter,
    world_seed: Res<WorldSeed>,
) {
    let Ok(player_transform) = player.get_single() else {
        return;
    };

    let biome_type = calculate_biome_at_position(
        player_transform.translation.x as i32,
        player_transform.translation.z as i32,
        world_seed.0,
    );

    let biome_name = format_biome_name(biome_type);

    for entity in query.iter() {
        *writer.text(entity, 0) = format!("Biome: {}", biome_name);
    }
}

/// Calculate the biome at a given world position using the same logic as world generation
fn calculate_biome_at_position(x: i32, z: i32, seed: u32) -> BiomeType {
    // Use the same noise generators as the server world generation
    let temp_perlin = Perlin::new(seed + 1);
    let humidity_perlin = Perlin::new(seed + 2);
    let biome_scale = 0.01;

    // Calculate temperature and humidity at this position
    let temperature = (temp_perlin.get([x as f64 * biome_scale, z as f64 * biome_scale]) + 1.0) / 2.0;
    let humidity = (humidity_perlin.get([x as f64 * biome_scale, z as f64 * biome_scale]) + 1.0) / 2.0;

    // Determine biome using shared logic
    determine_biome(temperature, humidity)
}

/// Format biome type as a human-readable string
fn format_biome_name(biome_type: BiomeType) -> &'static str {
    match biome_type {
        BiomeType::Plains => "Plains",
        BiomeType::Forest => "Forest",
        BiomeType::MediumMountain => "Medium Mountain",
        BiomeType::HighMountainGrass => "High Mountain Grass",
        BiomeType::Desert => "Desert",
        BiomeType::IcePlain => "Ice Plain",
        BiomeType::FlowerPlains => "Flower Plains",
        BiomeType::ShallowOcean => "Shallow Ocean",
        BiomeType::Ocean => "Ocean",
        BiomeType::DeepOcean => "Deep Ocean",
    }
}

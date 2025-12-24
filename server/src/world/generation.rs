use bevy::prelude::*;
use noise::{NoiseFn, Perlin};
use shared::{world::*, CHUNK_SIZE, SEA_LEVEL};
use std::collections::HashMap;

// Re-export shared biome functions
use shared::world::{calculate_temperature_humidity, determine_biome};

fn generate_tree(chunk: &mut ServerChunk, x: i32, y: i32, z: i32, trunk: BlockId, leaves: BlockId) {
    // create trunk
    let trunk_height = 3 + rand::random::<u8>() % 3; // random height between 3 and 5
    for dy in 0..trunk_height {
        let trunk_y = y + dy as i32;
        // Only place trunk blocks within chunk boundaries
        if x >= 0
            && x < CHUNK_SIZE
            && z >= 0
            && z < CHUNK_SIZE
            && trunk_y >= 0
            && trunk_y < CHUNK_SIZE
        {
            chunk.map.insert(
                IVec3::new(x, trunk_y, z),
                BlockData::new(trunk, BlockDirection::Front),
            );
        }
    }

    // place the leaves
    let leaf_start_y = y + trunk_height as i32 - 1;
    for layer in 0..3 {
        let current_y = leaf_start_y + layer;
        for offset_x in -2i32..=2i32 {
            for offset_z in -2i32..=2i32 {
                let cond1 = (offset_x.abs() + offset_z.abs()) < 3 - layer;
                let cond2 = (offset_x.abs() + offset_z.abs()) == 3 - layer
                    && rand::random::<f32>() < 0.2
                    && layer < 2;
                if cond1 || cond2 {
                    let leaf_x = x + offset_x;
                    let leaf_z = z + offset_z;
                    // Only place leaf blocks within chunk boundaries
                    if leaf_x >= 0
                        && leaf_x < CHUNK_SIZE
                        && leaf_z >= 0
                        && leaf_z < CHUNK_SIZE
                        && current_y >= 0
                        && current_y < CHUNK_SIZE
                    {
                        chunk.map.insert(
                            IVec3::new(leaf_x, current_y, leaf_z),
                            BlockData::new(leaves, BlockDirection::Front),
                        );
                    }
                }
            }
        }
    }
    let top_trunk_y = y + trunk_height as i32 - 1;
    if x >= 0
        && x < CHUNK_SIZE
        && z >= 0
        && z < CHUNK_SIZE
        && top_trunk_y >= 0
        && top_trunk_y < CHUNK_SIZE
    {
        chunk.map.insert(
            IVec3::new(x, top_trunk_y, z),
            BlockData::new(trunk, BlockDirection::Front),
        );
    }

    // add one leaf block at the top of the trunk
}

fn generate_big_tree(
    chunk: &mut ServerChunk,
    x: i32,
    y: i32,
    z: i32,
    trunk: BlockId,
    leaves: BlockId,
) {
    let trunk_height = 4 + rand::random::<u8>() % 3; // random height between 4 and 7
    let leaf_start_y = y + trunk_height as i32 - 2;
    // add branches
    for _ in 1..3 {
        let branch_x = x + rand::random::<i32>() % 2;
        let branch_z = z + rand::random::<i32>() % 2;
        let branch_y = std::cmp::max(leaf_start_y - 1 - rand::random::<i32>() % 2, 2);
        let prof = rand::random::<u8>() % 2 + 1;
        for dx in 0..prof {
            let bx = branch_x + dx as i32;
            // Branch leaves and trunk - check boundaries
            if bx >= 0
                && bx < CHUNK_SIZE
                && branch_z + 1 >= 0
                && branch_z + 1 < CHUNK_SIZE
                && branch_y >= 0
                && branch_y < CHUNK_SIZE
            {
                chunk.map.insert(
                    IVec3::new(bx, branch_y, branch_z + 1),
                    BlockData::new(leaves, BlockDirection::Front),
                );
            }
            if bx >= 0
                && bx < CHUNK_SIZE
                && branch_z - 1 >= 0
                && branch_z - 1 < CHUNK_SIZE
                && branch_y >= 0
                && branch_y < CHUNK_SIZE
            {
                chunk.map.insert(
                    IVec3::new(bx, branch_y, branch_z - 1),
                    BlockData::new(leaves, BlockDirection::Front),
                );
            }
            if bx >= 0
                && bx < CHUNK_SIZE
                && branch_z >= 0
                && branch_z < CHUNK_SIZE
                && branch_y + 1 >= 0
                && branch_y + 1 < CHUNK_SIZE
            {
                chunk.map.insert(
                    IVec3::new(bx, branch_y + 1, branch_z),
                    BlockData::new(leaves, BlockDirection::Front),
                );
            }
            if bx >= 0
                && bx < CHUNK_SIZE
                && branch_z >= 0
                && branch_z < CHUNK_SIZE
                && branch_y >= 0
                && branch_y < CHUNK_SIZE
            {
                chunk.map.insert(
                    IVec3::new(bx, branch_y, branch_z),
                    BlockData::new(trunk, BlockDirection::Front),
                );
            }
        }
        let final_bx = branch_x + prof as i32;
        if final_bx >= 0
            && final_bx < CHUNK_SIZE
            && branch_z >= 0
            && branch_z < CHUNK_SIZE
            && branch_y >= 0
            && branch_y < CHUNK_SIZE
        {
            chunk.map.insert(
                IVec3::new(final_bx, branch_y, branch_z),
                BlockData::new(leaves, BlockDirection::Front),
            );
        }
    }
    // create trunk

    for dy in 0..trunk_height {
        let trunk_y = y + dy as i32;
        if x >= 0
            && x < CHUNK_SIZE
            && z >= 0
            && z < CHUNK_SIZE
            && trunk_y >= 0
            && trunk_y < CHUNK_SIZE
        {
            chunk.map.insert(
                IVec3::new(x, trunk_y, z),
                BlockData::new(trunk, BlockDirection::Front),
            );
        }
    }

    // place the leaves

    for layer in 0..2 {
        let current_y = leaf_start_y + layer;
        for offset_x in -2i32..=2i32 {
            for offset_z in -2i32..=2i32 {
                if !(offset_x == 0 && offset_z == 0 || offset_x.abs() == 2 && offset_z.abs() == 2) {
                    let leaf_x = x + offset_x;
                    let leaf_z = z + offset_z;
                    if leaf_x >= 0
                        && leaf_x < CHUNK_SIZE
                        && leaf_z >= 0
                        && leaf_z < CHUNK_SIZE
                        && current_y >= 0
                        && current_y < CHUNK_SIZE
                    {
                        chunk.map.insert(
                            IVec3::new(leaf_x, current_y, leaf_z),
                            BlockData::new(leaves, BlockDirection::Front),
                        );
                    }
                }
            }
        }
    }

    // add one leaf block at the top of the trunk
    let top_y = leaf_start_y + 2;
    if x >= 0 && x < CHUNK_SIZE && z >= 0 && z < CHUNK_SIZE && top_y >= 0 && top_y < CHUNK_SIZE {
        chunk.map.insert(
            IVec3::new(x, top_y, z),
            BlockData::new(leaves, BlockDirection::Front),
        );
    }

    // Add random leaves above the top leaf
    for layer in 0..3 {
        let current_y = leaf_start_y + layer + 2;
        for offset_x in -2i32..=2i32 {
            for offset_z in -2i32..=2i32 {
                let cond1 = (offset_x.abs() + offset_z.abs()) < 3 - layer;
                let cond2 = (offset_x.abs() + offset_z.abs()) == 3 - layer
                    && rand::random::<f32>() < 0.2
                    && layer < 2;
                if cond1 || cond2 {
                    let leaf_x = x + offset_x;
                    let leaf_z = z + offset_z;
                    if leaf_x >= 0
                        && leaf_x < CHUNK_SIZE
                        && leaf_z >= 0
                        && leaf_z < CHUNK_SIZE
                        && current_y >= 0
                        && current_y < CHUNK_SIZE
                    {
                        chunk.map.insert(
                            IVec3::new(leaf_x, current_y, leaf_z),
                            BlockData::new(leaves, BlockDirection::Front),
                        );
                    }
                }
            }
        }
    }
}

fn generate_cactus(chunk: &mut ServerChunk, x: i32, y: i32, z: i32, cactus: BlockId) {
    let cactus_height = 2 + rand::random::<u8>() % 2;
    for dy in 0..cactus_height {
        let cactus_y = y + dy as i32;
        // Only place cactus blocks within chunk boundaries
        if x >= 0
            && x < CHUNK_SIZE
            && z >= 0
            && z < CHUNK_SIZE
            && cactus_y >= 0
            && cactus_y < CHUNK_SIZE
        {
            chunk.map.insert(
                IVec3::new(x, cactus_y, z),
                BlockData::new(cactus, BlockDirection::Front),
            );
        }
    }
}

fn interpolated_height(
    x: i32,
    z: i32,
    biome_scale: f64,
    perlin: &Perlin,
    temp_perlin: &Perlin,
    humidity_perlin: &Perlin,
    scale: f64,
) -> i32 {
    // get the properties of the main biome at (x, z)
    let temperature =
        (temp_perlin.get([x as f64 * biome_scale, z as f64 * biome_scale]) + 1.0) / 2.0;
    let humidity =
        (humidity_perlin.get([x as f64 * biome_scale, z as f64 * biome_scale]) + 1.0) / 2.0;
    let biome_type = determine_biome(temperature, humidity);
    let biome = get_biome_data(biome_type);

    // initialize weighted values
    let mut weighted_base_height = biome.base_height as f64;
    let mut weighted_variation = biome.height_variation as f64;
    let mut total_weight = 1.0;

    // loop through neighboring blocks to get influences
    for &offset_x in &[-4, 0, 4] {
        for &offset_z in &[-4, 0, 4] {
            if offset_x == 0 && offset_z == 0 {
                continue; // ignore the central position
            }

            let neighbor_x = x + offset_x;
            let neighbor_z = z + offset_z;

            // calculate the temperature and humidity of the neighboring block
            let neighbor_temp = (temp_perlin.get([
                neighbor_x as f64 * biome_scale,
                neighbor_z as f64 * biome_scale,
            ]) + 1.0)
                / 2.0;
            let neighbor_humidity = (humidity_perlin.get([
                neighbor_x as f64 * biome_scale,
                neighbor_z as f64 * biome_scale,
            ]) + 1.0)
                / 2.0;

            // determine the biome of the neighboring block
            let neighbor_biome_type = determine_biome(neighbor_temp, neighbor_humidity);
            let neighbor_biome = get_biome_data(neighbor_biome_type);

            // weight by distance (the farther a neighbor is, the less influence it has)
            let distance = ((offset_x.pow(2) + offset_z.pow(2)) as f64).sqrt();
            let weight = 1.0 / (distance + 1.0); // distance +1 to avoid division by zero

            // update weighted values
            weighted_base_height += neighbor_biome.base_height as f64 * weight;
            weighted_variation += neighbor_biome.height_variation as f64 * weight;
            total_weight += weight;
        }
    }

    // normalize weighted values
    weighted_base_height /= total_weight;
    weighted_variation /= total_weight;

    // final calculation of height with perlin noise
    let terrain_noise = perlin.get([x as f64 * scale, z as f64 * scale]);
    let interpolated_height = weighted_base_height + (weighted_variation * terrain_noise);

    interpolated_height.round() as i32
}

/// Helper function to attempt flora placement based on biome-specific thresholds
/// Returns true if flora was placed, false otherwise
fn try_place_flora<F>(
    threshold: f32,
    current_block: BlockId,
    valid_surface_blocks: &[BlockId],
    placement_fn: F,
) -> bool
where
    F: FnOnce(),
{
    if threshold <= 0.0 {
        return false;
    }

    if !valid_surface_blocks.contains(&current_block) {
        return false;
    }

    let chance = rand::random::<f32>();
    if chance < threshold {
        placement_fn();
        true
    } else {
        false
    }
}

/// Helper function to check if flora should be placed based on threshold and surface block.
/// Returns true if the roll succeeds, false otherwise.
fn should_place_flora(
    threshold: f32,
    current_block: BlockId,
    valid_surface_blocks: &[BlockId],
) -> bool {
    if threshold <= 0.0 {
        return false;
    }

    if !valid_surface_blocks.contains(&current_block) {
        return false;
    }

    rand::random::<f32>() < threshold
}

/// Fulfills a flora generation request by placing the appropriate flora type at the given position.
fn fulfill_flora_request(chunk: &mut ServerChunk, request: &FloraRequest) {
    let local_pos = IVec3::new(request.local_x, 0, request.local_z);

    match request.flora_type {
        FloraType::Flower => {
            let flower_type = if rand::random::<f32>() < 0.5 {
                BlockId::Dandelion
            } else {
                BlockId::Poppy
            };
            chunk.map.insert(
                local_pos,
                BlockData::new(flower_type, BlockDirection::Front),
            );
        }
        FloraType::TallGrass => {
            chunk.map.insert(
                local_pos,
                BlockData::new(BlockId::TallGrass, BlockDirection::Front),
            );
        }
        FloraType::Tree => {
            generate_tree(
                chunk,
                request.local_x,
                0,
                request.local_z,
                BlockId::OakLog,
                BlockId::OakLeaves,
            );
        }
        FloraType::BigTree => {
            generate_big_tree(
                chunk,
                request.local_x,
                0,
                request.local_z,
                BlockId::OakLog,
                BlockId::OakLeaves,
            );
        }
        FloraType::Cactus => {
            generate_cactus(chunk, request.local_x, 0, request.local_z, BlockId::Cactus);
        }
    }
}

/// Result of chunk generation containing the generated chunk and any pending
/// generation requests for the chunk above.
pub struct ChunkGenerationResult {
    /// The generated chunk
    pub chunk: ServerChunk,
    /// Generation requests to be fulfilled by the chunk above (y + 1)
    pub requests_for_chunk_above: Vec<FloraRequest>,
}

/// Generates a chunk at the given position.
///
/// # Arguments
/// * `chunk_pos` - The chunk position in chunk coordinates
/// * `seed` - The world seed for procedural generation
/// * `pending_requests` - Optional list of pending flora generation requests from the chunk below.
///   These are processed first before generating new flora.
///
/// # Returns
/// A `ChunkGenerationResult` containing the generated chunk and any requests for the chunk above.
pub fn generate_chunk(
    chunk_pos: IVec3,
    seed: u32,
    pending_requests: Option<Vec<FloraRequest>>,
) -> ChunkGenerationResult {
    let perlin = Perlin::new(seed);
    let temp_perlin = Perlin::new(seed + 1);
    let humidity_perlin = Perlin::new(seed + 2);

    let scale = 0.1;
    let biome_scale = 0.01;
    let cx = chunk_pos.x;
    let cy = chunk_pos.y;
    let cz = chunk_pos.z;

    let mut chunk = ServerChunk {
        map: HashMap::new(),
        ts: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
        sent_to_clients: vec![],
    };

    // Collection of generation requests for the chunk above
    let mut requests_for_chunk_above: Vec<FloraRequest> = Vec::new();

    // First, process any pending generation requests from the chunk below
    if let Some(requests) = pending_requests {
        for request in requests {
            fulfill_flora_request(&mut chunk, &request);
        }
    }

    for dx in 0..CHUNK_SIZE {
        for dz in 0..CHUNK_SIZE {
            let x = CHUNK_SIZE * cx + dx;
            let z = CHUNK_SIZE * cz + dz;

            // calculate temperature and humidity using shared function
            let climate = calculate_temperature_humidity(x, z, seed);

            // get biome regarding the two values
            let biome_type = determine_biome(climate.temperature, climate.humidity);
            let biome = get_biome_data(biome_type);

            // get terrain height
            let terrain_height = interpolated_height(
                x,
                z,
                biome_scale,
                &perlin,
                &temp_perlin,
                &humidity_perlin,
                scale,
            );

            // generate blocs
            for dy in 0..CHUNK_SIZE {
                let y = CHUNK_SIZE * cy + dy;

                if y > terrain_height && y > SEA_LEVEL {
                    break;
                }

                let block = if y == 0 {
                    BlockId::Bedrock
                } else if y < terrain_height - 4 {
                    BlockId::Stone
                } else if y < terrain_height {
                    biome.sub_surface_block
                } else if y == terrain_height {
                    biome.surface_block
                } else if y <= SEA_LEVEL {
                    BlockId::Water
                } else {
                    panic!();
                };

                let block_pos = IVec3::new(dx, dy, dz);

                chunk
                    .map
                    .insert(block_pos, BlockData::new(block, BlockDirection::Front));

                // Determine flora placement thresholds based on biome
                let flower_threshold = match biome_type {
                    BiomeType::FlowerPlains => 0.1,
                    BiomeType::Plains | BiomeType::Forest | BiomeType::MediumMountain => 0.02,
                    _ => 0.0,
                };

                let tall_grass_threshold = match biome_type {
                    BiomeType::HighMountainGrass | BiomeType::Desert | BiomeType::IcePlain => 0.0,
                    _ => 0.1,
                };

                let tree_threshold = match biome_type {
                    BiomeType::Forest => 0.06,
                    BiomeType::FlowerPlains | BiomeType::MediumMountain => 0.02,
                    _ => 0.0,
                };

                let cactus_threshold = match biome_type {
                    BiomeType::Desert => 0.01,
                    _ => 0.0,
                };

                let valid_tree_position =
                    dx >= 1 && dx < CHUNK_SIZE - 1 && dz >= 1 && dz < CHUNK_SIZE - 1;

                // If we're at the top of the chunk (dy == CHUNK_SIZE - 1), create a generation
                // request for the chunk above instead of placing flora directly
                if block_pos.y + 1 >= CHUNK_SIZE {
                    // Try to create generation requests for the chunk above
                    if should_place_flora(flower_threshold, block, &[BlockId::Grass]) {
                        requests_for_chunk_above.push(FloraRequest {
                            local_x: dx,
                            local_z: dz,
                            flora_type: FloraType::Flower,
                            biome_type,
                        });
                    } else if should_place_flora(tall_grass_threshold, block, &[BlockId::Grass]) {
                        requests_for_chunk_above.push(FloraRequest {
                            local_x: dx,
                            local_z: dz,
                            flora_type: FloraType::TallGrass,
                            biome_type,
                        });
                    } else if valid_tree_position
                        && should_place_flora(tree_threshold, block, &[BlockId::Grass])
                    {
                        // Determine if this should be a big tree based on biome and threshold
                        // Note: tree_threshold > 0.0 is guaranteed by should_place_flora returning true
                        let flora_type = if biome_type == BiomeType::Forest
                            && tree_threshold > 0.0
                            && rand::random::<f32>() < 0.01 / tree_threshold
                        {
                            FloraType::BigTree
                        } else {
                            FloraType::Tree
                        };
                        requests_for_chunk_above.push(FloraRequest {
                            local_x: dx,
                            local_z: dz,
                            flora_type,
                            biome_type,
                        });
                    } else if should_place_flora(cactus_threshold, block, &[BlockId::Sand]) {
                        requests_for_chunk_above.push(FloraRequest {
                            local_x: dx,
                            local_z: dz,
                            flora_type: FloraType::Cactus,
                            biome_type,
                        });
                    }
                    continue;
                }

                // Normal flora placement for blocks not at chunk top
                // Try placing flora in priority order
                if try_place_flora(flower_threshold, block, &[BlockId::Grass], || {
                    let flower_type = if rand::random::<f32>() < 0.5 {
                        BlockId::Dandelion
                    } else {
                        BlockId::Poppy
                    };
                    chunk.map.insert(
                        block_pos.with_y(block_pos.y + 1),
                        BlockData::new(flower_type, BlockDirection::Front),
                    );
                }) {
                    continue;
                }

                if try_place_flora(tall_grass_threshold, block, &[BlockId::Grass], || {
                    chunk.map.insert(
                        block_pos.with_y(block_pos.y + 1),
                        BlockData::new(BlockId::TallGrass, BlockDirection::Front),
                    );
                }) {
                    continue;
                }

                if valid_tree_position
                    && try_place_flora(tree_threshold, block, &[BlockId::Grass], || {
                        // Determine if this should be a big tree based on biome and threshold
                        // Note: tree_threshold > 0.0 is guaranteed by try_place_flora calling this closure
                        if biome_type == BiomeType::Forest
                            && tree_threshold > 0.0
                            && rand::random::<f32>() < 0.01 / tree_threshold
                        {
                            generate_big_tree(
                                &mut chunk,
                                dx,
                                dy + 1,
                                dz,
                                BlockId::OakLog,
                                BlockId::OakLeaves,
                            );
                        } else {
                            generate_tree(
                                &mut chunk,
                                dx,
                                dy + 1,
                                dz,
                                BlockId::OakLog,
                                BlockId::OakLeaves,
                            );
                        }
                    })
                {
                    continue;
                }

                try_place_flora(cactus_threshold, block, &[BlockId::Sand], || {
                    generate_cactus(&mut chunk, dx, dy + 1, dz, BlockId::Cactus);
                });
            }
        }
    }
    ChunkGenerationResult {
        chunk,
        requests_for_chunk_above,
    }
}

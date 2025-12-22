use bevy::prelude::*;
use noise::{NoiseFn, Perlin};
use shared::{world::*, CHUNK_SIZE};
use std::collections::HashMap;

/// Apply pending blocks from neighboring chunks to a newly generated chunk
pub fn apply_pending_blocks(
    chunk: &mut ServerChunk,
    chunk_pos: IVec3,
    chunks_map: &HashMap<IVec3, ServerChunk>,
) {
    // Process pending blocks from neighboring chunks
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                if dx == 0 && dy == 0 && dz == 0 {
                    continue;
                }
                
                let neighbor_pos = chunk_pos + IVec3::new(dx, dy, dz);
                let inverse_offset = IVec3::new(-dx, -dy, -dz);
                
                if let Some(neighbor_chunk) = chunks_map.get(&neighbor_pos) {
                    if let Some(pending_blocks) = neighbor_chunk.pending_blocks.get(&inverse_offset) {
                        for (local_pos, block_data) in pending_blocks.iter() {
                            // Only insert if the position is empty to avoid overwriting terrain
                            chunk.map.entry(*local_pos).or_insert(*block_data);
                        }
                    }
                }
            }
        }
    }
}

/// Helper function to place a block in the chunk or queue it for a neighboring chunk
fn place_or_queue_block(
    chunk: &mut ServerChunk,
    x: i32,
    y: i32,
    z: i32,
    block: BlockData,
) {
    // Check if block is within chunk boundaries
    if x >= 0 && x < CHUNK_SIZE && y >= 0 && y < CHUNK_SIZE && z >= 0 && z < CHUNK_SIZE {
        // Place block directly in this chunk
        chunk.map.insert(IVec3::new(x, y, z), block);
    } else {
        // Calculate which neighboring chunk this block belongs to
        let chunk_offset_x = if x < 0 {
            -1
        } else if x >= CHUNK_SIZE {
            1
        } else {
            0
        };
        let chunk_offset_y = if y < 0 {
            -1
        } else if y >= CHUNK_SIZE {
            1
        } else {
            0
        };
        let chunk_offset_z = if z < 0 {
            -1
        } else if z >= CHUNK_SIZE {
            1
        } else {
            0
        };
        
        let chunk_offset = IVec3::new(chunk_offset_x, chunk_offset_y, chunk_offset_z);
        
        // Calculate local position in the neighboring chunk
        let local_x = ((x % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;
        let local_y = ((y % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;
        let local_z = ((z % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;
        let local_pos = IVec3::new(local_x, local_y, local_z);
        
        // Queue the block for the neighboring chunk
        chunk
            .pending_blocks
            .entry(chunk_offset)
            .or_insert_with(HashMap::new)
            .insert(local_pos, block);
    }
}

fn generate_tree(chunk: &mut ServerChunk, x: i32, y: i32, z: i32, trunk: BlockId, leaves: BlockId) {
    // create trunk
    let trunk_height = 3 + rand::random::<u8>() % 3; // random height between 3 and 5
    for dy in 0..trunk_height {
        let trunk_y = y + dy as i32;
        place_or_queue_block(
            chunk,
            x,
            trunk_y,
            z,
            BlockData::new(trunk, BlockDirection::Front),
        );
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
                    place_or_queue_block(
                        chunk,
                        leaf_x,
                        current_y,
                        leaf_z,
                        BlockData::new(leaves, BlockDirection::Front),
                    );
                }
            }
        }
    }
    let top_trunk_y = y + trunk_height as i32 - 1;
    place_or_queue_block(
        chunk,
        x,
        top_trunk_y,
        z,
        BlockData::new(trunk, BlockDirection::Front),
    );

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
            // Branch leaves and trunk
            place_or_queue_block(
                chunk,
                bx,
                branch_y,
                branch_z + 1,
                BlockData::new(leaves, BlockDirection::Front),
            );
            place_or_queue_block(
                chunk,
                bx,
                branch_y,
                branch_z - 1,
                BlockData::new(leaves, BlockDirection::Front),
            );
            place_or_queue_block(
                chunk,
                bx,
                branch_y + 1,
                branch_z,
                BlockData::new(leaves, BlockDirection::Front),
            );
            place_or_queue_block(
                chunk,
                bx,
                branch_y,
                branch_z,
                BlockData::new(trunk, BlockDirection::Front),
            );
        }
        let final_bx = branch_x + prof as i32;
        place_or_queue_block(
            chunk,
            final_bx,
            branch_y,
            branch_z,
            BlockData::new(leaves, BlockDirection::Front),
        );
    }
    // create trunk

    for dy in 0..trunk_height {
        let trunk_y = y + dy as i32;
        place_or_queue_block(
            chunk,
            x,
            trunk_y,
            z,
            BlockData::new(trunk, BlockDirection::Front),
        );
    }

    // place the leaves

    for layer in 0..2 {
        let current_y = leaf_start_y + layer;
        for offset_x in -2i32..=2i32 {
            for offset_z in -2i32..=2i32 {
                if !(offset_x == 0 && offset_z == 0 || offset_x.abs() == 2 && offset_z.abs() == 2) {
                    let leaf_x = x + offset_x;
                    let leaf_z = z + offset_z;
                    place_or_queue_block(
                        chunk,
                        leaf_x,
                        current_y,
                        leaf_z,
                        BlockData::new(leaves, BlockDirection::Front),
                    );
                }
            }
        }
    }

    // add one leaf block at the top of the trunk
    let top_y = leaf_start_y + 2;
    place_or_queue_block(
        chunk,
        x,
        top_y,
        z,
        BlockData::new(leaves, BlockDirection::Front),
    );

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
                    place_or_queue_block(
                        chunk,
                        leaf_x,
                        current_y,
                        leaf_z,
                        BlockData::new(leaves, BlockDirection::Front),
                    );
                }
            }
        }
    }
}

fn generate_cactus(chunk: &mut ServerChunk, x: i32, y: i32, z: i32, cactus: BlockId) {
    let cactus_height = 2 + rand::random::<u8>() % 2;
    for dy in 0..cactus_height {
        let cactus_y = y + dy as i32;
        place_or_queue_block(
            chunk,
            x,
            cactus_y,
            z,
            BlockData::new(cactus, BlockDirection::Front),
        );
    }
}

pub fn determine_biome(temperature: f64, humidity: f64) -> BiomeType {
    let ocean_percentage: f64 = 0.33;
    if humidity > (1.0 - (ocean_percentage / 3.0)) {
        return BiomeType::DeepOcean;
    }
    if humidity > (1.0 - 2.0 * (ocean_percentage / 3.0)) {
        return BiomeType::Ocean;
    }
    if humidity > (1.0 - ocean_percentage) {
        return BiomeType::ShallowOcean;
    }
    if temperature > 0.6 {
        if humidity > (1.0 - ocean_percentage) / 2.0 {
            BiomeType::Forest
        } else {
            BiomeType::Desert
        }
    } else if temperature > 0.3 {
        if humidity > 2.0 * (1.0 - ocean_percentage) / 3.0 {
            BiomeType::FlowerPlains
        } else if humidity > (1.0 - ocean_percentage) / 3.0 {
            BiomeType::Plains
        } else {
            BiomeType::MediumMountain
        }
    } else if temperature >= 0.0 {
        if humidity > (1.0 - ocean_percentage) / 2.0 {
            BiomeType::IcePlain
        } else {
            BiomeType::HighMountainGrass
        }
    } else {
        panic!();
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

pub fn generate_chunk(chunk_pos: IVec3, seed: u32) -> ServerChunk {
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
        pending_blocks: HashMap::new(),
    };

    for dx in 0..CHUNK_SIZE {
        for dz in 0..CHUNK_SIZE {
            let x = CHUNK_SIZE * cx + dx;
            let z = CHUNK_SIZE * cz + dz;

            // calculate temperature and humidity
            let temperature =
                (temp_perlin.get([x as f64 * biome_scale, z as f64 * biome_scale]) + 1.0) / 2.0;
            let humidity =
                (humidity_perlin.get([x as f64 * biome_scale, z as f64 * biome_scale]) + 1.0) / 2.0;

            // get biome regarding the two values
            let biome_type = determine_biome(temperature, humidity);
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

                if y > terrain_height && y > 62 {
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
                } else if y <= 62 {
                    BlockId::Water
                } else {
                    panic!();
                };

                let block_pos = IVec3::new(dx, dy, dz);

                chunk
                    .map
                    .insert(block_pos, BlockData::new(block, BlockDirection::Front));

                // Add flora in biomes
                if y == terrain_height && terrain_height > 62 {
                    let above_surface_pos = IVec3::new(dx, terrain_height + 1, dz);

                    // Add flowers
                    let flower_chance = rand::random::<f32>();
                    match biome_type {
                        BiomeType::FlowerPlains => {
                            // High probability for flowers in Flower Plains
                            if flower_chance < 0.1 {
                                let flower_type = if rand::random::<f32>() < 0.5 {
                                    BlockId::Dandelion
                                } else {
                                    BlockId::Poppy
                                };

                                chunk.map.insert(
                                    block_pos.with_y(block_pos.y + 1),
                                    BlockData::new(flower_type, BlockDirection::Front),
                                );
                            }
                        }
                        BiomeType::Plains | BiomeType::Forest | BiomeType::MediumMountain => {
                            // Low probability for flowers in Plains, Forest, Medium Mountain
                            if flower_chance < 0.02 {
                                let flower_type = if rand::random::<f32>() < 0.5 {
                                    BlockId::Dandelion
                                } else {
                                    BlockId::Poppy
                                };

                                chunk.map.insert(
                                    block_pos.with_y(block_pos.y + 1),
                                    BlockData::new(flower_type, BlockDirection::Front),
                                );
                            }
                        }
                        _ => {}
                    }

                    // Add tall grass
                    if biome_type != BiomeType::HighMountainGrass
                        && biome_type != BiomeType::Desert
                        && biome_type != BiomeType::IcePlain
                    {
                        let tall_grass_chance = rand::random::<f32>();
                        if tall_grass_chance < 0.10 {
                            chunk.map.insert(
                                block_pos.with_y(block_pos.y + 1),
                                BlockData::new(BlockId::TallGrass, BlockDirection::Front),
                            );
                        }
                    }

                    // Add trees
                    let tree_chance = rand::random::<f32>();
                    match biome_type {
                        BiomeType::Forest => {
                            // High probability for trees in Forest
                            if tree_chance < 0.06 && !chunk.map.contains_key(&above_surface_pos) {
                                if tree_chance < 0.01 {
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
                            }
                        }
                        BiomeType::FlowerPlains | BiomeType::MediumMountain => {
                            // Medium probability for trees in Flower Plains and Medium Mountain
                            if tree_chance < 0.02 && !chunk.map.contains_key(&above_surface_pos) {
                                generate_tree(
                                    &mut chunk,
                                    dx,
                                    dy + 1,
                                    dz,
                                    BlockId::OakLog,
                                    BlockId::OakLeaves,
                                );
                            }
                        }
                        _ => {}
                    }

                    // Add cactus in Desert
                    if biome_type == BiomeType::Desert {
                        let cactus_chance = rand::random::<f32>();
                        if cactus_chance < 0.01 && !chunk.map.contains_key(&above_surface_pos) {
                            generate_cactus(&mut chunk, dx, dy + 1, dz, BlockId::Cactus);
                        }
                    }
                }
            }
        }
    }
    chunk
}

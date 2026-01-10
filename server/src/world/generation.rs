use bevy::prelude::*;
use noiz::prelude::*;
use shared::{world::*, CHUNK_SIZE, SEA_LEVEL};
use std::collections::{HashMap, HashSet};

fn try_place_block(
    chunk: &mut ServerChunk,
    x: i32,
    y: i32,
    z: i32,
    block: BlockId,
    direction: BlockDirection,
) {
    if x >= 0 && x < CHUNK_SIZE && y >= 0 && y < CHUNK_SIZE && z >= 0 && z < CHUNK_SIZE {
        chunk
            .map
            .insert(IVec3::new(x, y, z), BlockData::new(block, direction));
    }
}

// Import shared biome functions
use shared::world::{calculate_temperature_humidity_with_noises, ClimateNoises};

fn generate_tree(chunk: &mut ServerChunk, x: i32, y: i32, z: i32, trunk: BlockId, leaves: BlockId) {
    // create trunk
    let trunk_height = 3 + rand::random::<u8>() % 3; // random height between 3 and 5
    for dy in 0..trunk_height {
        let trunk_y = y + dy as i32;
        try_place_block(chunk, x, trunk_y, z, trunk, BlockDirection::Front);
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
                    try_place_block(
                        chunk,
                        leaf_x,
                        current_y,
                        leaf_z,
                        leaves,
                        BlockDirection::Front,
                    );
                }
            }
        }
    }
    let top_trunk_y = y + trunk_height as i32 - 1;
    try_place_block(chunk, x, top_trunk_y, z, trunk, BlockDirection::Front);

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
            try_place_block(
                chunk,
                bx,
                branch_y,
                branch_z + 1,
                leaves,
                BlockDirection::Front,
            );
            try_place_block(
                chunk,
                bx,
                branch_y,
                branch_z - 1,
                leaves,
                BlockDirection::Front,
            );
            try_place_block(
                chunk,
                bx,
                branch_y + 1,
                branch_z,
                leaves,
                BlockDirection::Front,
            );
            try_place_block(chunk, bx, branch_y, branch_z, trunk, BlockDirection::Front);
        }
        let final_bx = branch_x + prof as i32;
        try_place_block(
            chunk,
            final_bx,
            branch_y,
            branch_z,
            leaves,
            BlockDirection::Front,
        );
    }
    // create trunk
    for dy in 0..trunk_height {
        let trunk_y = y + dy as i32;
        try_place_block(chunk, x, trunk_y, z, trunk, BlockDirection::Front);
    }

    // place the leaves

    for layer in 0..2 {
        let current_y = leaf_start_y + layer;
        for offset_x in -2i32..=2i32 {
            for offset_z in -2i32..=2i32 {
                if !(offset_x == 0 && offset_z == 0 || offset_x.abs() == 2 && offset_z.abs() == 2) {
                    let leaf_x = x + offset_x;
                    let leaf_z = z + offset_z;
                    try_place_block(
                        chunk,
                        leaf_x,
                        current_y,
                        leaf_z,
                        leaves,
                        BlockDirection::Front,
                    );
                }
            }
        }
    }

    // add one leaf block at the top of the trunk
    let top_y = leaf_start_y + 2;
    try_place_block(chunk, x, top_y, z, leaves, BlockDirection::Front);

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
                    try_place_block(
                        chunk,
                        leaf_x,
                        current_y,
                        leaf_z,
                        leaves,
                        BlockDirection::Front,
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
    perlin: &Noise<common_noise::Perlin>,
    scale: f32,
    seed: u32,
) -> i32 {
    // get the properties of the main biome at (x, z)
    let climate = calculate_temperature_humidity(x, z, seed);
    let biome_type = BiomeType::from_climate(climate);
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
            let neighbor_climate = calculate_temperature_humidity(neighbor_x, neighbor_z, seed);

            // determine the biome of the neighboring block
            let neighbor_biome_type = BiomeType::from_climate(neighbor_climate);
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
    let sample_pos = Vec2::new(x as f32 * scale, z as f32 * scale);
    let terrain_noise = perlin.sample_for::<f32>(sample_pos) as f64;
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
    let mut perlin = Noise::<common_noise::Perlin>::default();
    perlin.set_seed(seed);
    let mut climate_noises = ClimateNoises::new(seed);

    let scale: f32 = 0.1;
    let cx = chunk_pos.x;
    let cy = chunk_pos.y;
    let cz = chunk_pos.z;

    let mut chunk = ServerChunk {
        map: HashMap::new(),
        water: shared::world::ChunkWaterStorage::new(),
        water_surfaces: shared::world::ChunkWaterSurfaces::new(),
        ts: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
        sent_to_clients: HashSet::new(),
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
            let climate = calculate_temperature_humidity_with_noises(x, z, &mut climate_noises);

            // get biome regarding the two values
            let biome_type = BiomeType::from_climate(climate);
            let biome = get_biome_data(biome_type);

            // get terrain height
            let terrain_height = interpolated_height(x, z, &perlin, scale, seed);

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
                    // Add water to volume storage as well as block
                    chunk.water.set_full(IVec3::new(dx, dy, dz));
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

// ============================================================================
// DEBUG WORLD GENERATION
// ============================================================================

/// Debug world configuration for testing wave scale thresholds.
///
/// Creates a flat world with water bodies of increasing size to visualize
/// how wave scaling responds to different local volumes.
pub struct DebugWaterWorld {
    /// Base Y level for the ground
    pub ground_level: i32,
    /// Water depth (blocks below surface)
    pub water_depth: i32,
}

impl Default for DebugWaterWorld {
    fn default() -> Self {
        Self {
            ground_level: 60,
            water_depth: 3,
        }
    }
}

/// Generates a debug chunk for wave scale testing.
///
/// The world layout (all positions relative to spawn at 0,0):
/// - Spawn on a stone platform at (0, ground_level, 0)
/// - Puddle (1x1) at X=5, Z=5  -> ~1.0 volume (below puddle threshold)
/// - Pond (3x3) at X=15, Z=5   -> ~9.0 volume (pond threshold)  
/// - Lake (5x5) at X=30, Z=5   -> ~25.0 volume (lake threshold)
/// - Ocean (10x10+) at X=50, Z=5 -> 50+ volume (ocean threshold)
///
/// Each water body is surrounded by stone walls and has configurable depth.
pub fn generate_debug_water_chunk(chunk_pos: IVec3, config: &DebugWaterWorld) -> ServerChunk {
    let mut chunk = ServerChunk {
        map: HashMap::new(),
        water: shared::world::ChunkWaterStorage::new(),
        water_surfaces: shared::world::ChunkWaterSurfaces::new(),
        ts: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
        sent_to_clients: HashSet::new(),
    };

    let cx = chunk_pos.x;
    let cy = chunk_pos.y;
    let cz = chunk_pos.z;

    // Generate flat ground and water bodies
    for dx in 0..CHUNK_SIZE {
        for dz in 0..CHUNK_SIZE {
            let world_x = cx * CHUNK_SIZE + dx;
            let world_z = cz * CHUNK_SIZE + dz;

            for dy in 0..CHUNK_SIZE {
                let world_y = cy * CHUNK_SIZE + dy;
                let local_pos = IVec3::new(dx, dy, dz);

                // Check if this position is in a water body
                let water_info = get_debug_water_body(world_x, world_z, config);

                let block = if world_y == 0 {
                    // Bedrock at bottom
                    Some(BlockId::Bedrock)
                } else if let Some((pool_depth, is_edge)) = water_info {
                    // We're inside or at the edge of a water body
                    let water_surface = config.ground_level;
                    let water_bottom = water_surface - pool_depth;

                    if is_edge {
                        // Edge wall
                        if world_y <= config.ground_level {
                            Some(BlockId::Stone)
                        } else {
                            None
                        }
                    } else if world_y < water_bottom {
                        // Below the pool
                        Some(BlockId::Stone)
                    } else if world_y == water_bottom {
                        // Pool floor
                        Some(BlockId::Sand)
                    } else if world_y < water_surface {
                        // Water
                        chunk.water.set_full(local_pos);
                        Some(BlockId::Water)
                    } else if world_y == water_surface {
                        // Water surface
                        chunk.water.set_full(local_pos);
                        Some(BlockId::Water)
                    } else {
                        None
                    }
                } else if world_y < config.ground_level {
                    // Underground
                    Some(BlockId::Stone)
                } else if world_y == config.ground_level {
                    // Ground surface
                    Some(BlockId::Grass)
                } else {
                    // Air above ground
                    None
                };

                if let Some(block_id) = block {
                    chunk
                        .map
                        .insert(local_pos, BlockData::new(block_id, BlockDirection::Front));
                }
            }
        }
    }

    chunk
}

/// Returns water body info for a world position: Some((depth, is_edge)) or None.
/// The depth is how deep the pool is, is_edge indicates if this is a wall block.
fn get_debug_water_body(
    world_x: i32,
    world_z: i32,
    config: &DebugWaterWorld,
) -> Option<(i32, bool)> {
    // Define water bodies: (center_x, center_z, half_size, depth, name)
    // Using wave scale thresholds as reference:
    // - Puddle threshold: 2.0 volume -> 1x1x1 = 1.0, need 1x1x2 or 2x1x1
    // - Pond threshold: 8.0 volume -> 3x3x1 = 9.0
    // - Lake threshold: 25.0 volume -> 5x5x1 = 25.0
    // - Ocean threshold: 50.0 volume -> 7x7x1 = 49.0, or smaller with depth

    let pools = [
        // (center_x, center_z, half_size, depth, name)
        (8, 8, 0, 2, "Tiny (1x1x2)"), // 1x1, 2 deep = ~2 volume (puddle)
        (20, 8, 1, 2, "Small (3x3x2)"), // 3x3, 2 deep = ~18 volume (pond+)
        (35, 8, 2, 3, "Medium (5x5x3)"), // 5x5, 3 deep = ~75 volume (lake+)
        (55, 8, 5, 4, "Large (11x11x4)"), // 11x11, 4 deep = ~484 volume (ocean)
    ];

    // Also add a row at Z=20 with shallower versions
    let shallow_pools = [
        (8, 25, 0, 1, "Puddle (1x1x1)"),   // 1x1, 1 deep = ~1 volume
        (20, 25, 1, 1, "Pond (3x3x1)"),    // 3x3, 1 deep = ~9 volume
        (35, 25, 2, 1, "Lake (5x5x1)"),    // 5x5, 1 deep = ~25 volume
        (55, 25, 5, 1, "Ocean (11x11x1)"), // 11x11, 1 deep = ~121 volume
    ];

    // Check deep pools
    for (cx, cz, half_size, depth, _name) in pools.iter() {
        let result = check_pool_bounds(world_x, world_z, *cx, *cz, *half_size, *depth, config);
        if result.is_some() {
            return result;
        }
    }

    // Check shallow pools
    for (cx, cz, half_size, depth, _name) in shallow_pools.iter() {
        let result = check_pool_bounds(world_x, world_z, *cx, *cz, *half_size, *depth, config);
        if result.is_some() {
            return result;
        }
    }

    None
}

/// Check if a position is within a pool's bounds.
/// Returns Some((depth, is_edge)) if in pool area, None otherwise.
fn check_pool_bounds(
    world_x: i32,
    world_z: i32,
    center_x: i32,
    center_z: i32,
    half_size: i32,
    depth: i32,
    _config: &DebugWaterWorld,
) -> Option<(i32, bool)> {
    let dx = (world_x - center_x).abs();
    let dz = (world_z - center_z).abs();

    // Edge is one block wider than the pool itself
    let edge_half = half_size + 1;

    if dx <= edge_half && dz <= edge_half {
        // We're in the pool area (including edge)
        let is_edge = dx == edge_half || dz == edge_half;
        Some((depth, is_edge))
    } else {
        None
    }
}

/// Generates the complete debug world map for all chunks that contain water bodies.
///
/// Returns a HashMap of chunk positions to chunks that can be merged with an existing world.
pub fn generate_debug_water_world() -> HashMap<IVec3, ServerChunk> {
    let config = DebugWaterWorld::default();
    let mut chunks = HashMap::new();

    // We need to generate chunks that cover:
    // 1. All water bodies (X=0 to X=70, Z=0 to Z=35)
    // 2. Enough area around spawn for the client to load (render distance = 8 chunks)
    // Spawn is at (0, 61, 0), so we need chunks around chunk (0, 3, 0)

    let min_chunk_x = -10;
    let max_chunk_x = 10; // Covers spawn area + water bodies
    let min_chunk_z = -10;
    let max_chunk_z = 10; // Covers spawn area + water bodies
    let min_chunk_y = 0;
    let max_chunk_y = 5; // Covers ground level and above

    for cx in min_chunk_x..=max_chunk_x {
        for cz in min_chunk_z..=max_chunk_z {
            for cy in min_chunk_y..=max_chunk_y {
                let chunk_pos = IVec3::new(cx, cy, cz);
                let chunk = generate_debug_water_chunk(chunk_pos, &config);

                // Only add chunk if it has content
                if !chunk.map.is_empty() {
                    chunks.insert(chunk_pos, chunk);
                }
            }
        }
    }

    info!("Generated debug water world with {} chunks", chunks.len());
    info!("Water bodies at ground level {}:", config.ground_level);
    info!("  Row 1 (Z=8): Deep pools - Tiny(1x1x2), Small(3x3x2), Medium(5x5x3), Large(11x11x4)");
    info!(
        "  Row 2 (Z=25): Shallow pools - Puddle(1x1x1), Pond(3x3x1), Lake(5x5x1), Ocean(11x11x1)"
    );
    info!("Spawn at (0, {}, 0)", config.ground_level + 1);

    chunks
}

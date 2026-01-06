# World System Documentation

## Overview

The world system is responsible for generating, managing, and rendering the voxel-based game world. It's split between server-side authority (generation, simulation) and client-side rendering.

## Architecture

```
World System
├── Server (Authority)
│   ├── Generation (Procedural terrain)
│   ├── Simulation (Physics, updates)
│   ├── Storage (Save/load)
│   └── Broadcasting (Send updates)
│
├── Client (Rendering)
│   ├── Meshing (Voxel to polygons)
│   ├── Rendering (Display)
│   └── Caching (Chunk management)
│
└── Shared (Data structures)
    ├── Blocks (Types and properties)
    ├── Chunks (16x16x256 sections)
    └── Items (Inventory objects)
```

## Data Structures

### Chunk

A chunk is a 16x16x256 section of the world:

```rust
pub struct ServerChunk {
    pub map: HashMap<IVec3, BlockData>,
    pub position: IVec2,  // x, z coordinates
}

pub struct ClientChunk {
    pub blocks: HashMap<IVec3, BlockData>,
    pub mesh_handle: Option<Handle<Mesh>>,
    pub needs_remesh: bool,
}
```

**Key Points**:
- Only stores non-air blocks (sparse storage)
- Position is 2D (x, z) as chunks are full height
- Y coordinate ranges from 0-255

### Block Data

```rust
pub struct BlockData {
    pub block_id: BlockId,
    pub direction: BlockDirection,
}

pub enum BlockId {
    Dirt,
    Grass,
    Stone,
    // ... 20 total block types
}
```

**Block Properties**:
- `is_solid()`: Can players collide with it?
- `is_transparent()`: Does light pass through?
- `is_climbable()`: Can players climb it?
- `has_collision()`: Physics interaction

### World Map

```rust
// Server
pub struct ServerWorldMap {
    pub chunks: HashMap<IVec2, ServerChunk>,
    pub seed: i32,
}

// Client
pub struct ClientWorldMap {
    pub chunks: HashMap<IVec2, ClientChunk>,
    pub render_distance: u32,
}
```

## Server-Side World Generation

### Generation Pipeline

**Location**: `server/src/world/generation.rs`

1. **Heightmap Generation**
   ```rust
   fn generate_heightmap(noise: &Perlin, chunk_pos: IVec2) -> [[i32; 16]; 16]
   ```
   - Uses Perlin noise for smooth terrain
   - Multiple octaves for detail at different scales
   - Heightmap determines ground level for each column

2. **Biome Selection**
   ```rust
   fn determine_biome(temperature: f64, moisture: f64) -> Biome
   ```
   
   Biomes:
   - **Plains**: Flat grassland, occasional trees
   - **Forest**: Dense oak trees, tall grass
   - **Mountains**: High peaks, stone exposed
   - **Desert**: Sand, cacti, hot and dry
   - **Ice Plain**: Snow, ice, frozen water
   - **Flower Plains**: Colorful flowers, grass

3. **Base Terrain**
   - Fill below heightmap with stone/dirt/sand
   - Add grass layer on top (biome-dependent)
   - Place bedrock at Y=0

4. **Feature Placement**
   ```rust
   fn place_trees(chunk: &mut ServerChunk, biome: Biome)
   fn place_vegetation(chunk: &mut ServerChunk, biome: Biome)
   fn place_flowers(chunk: &mut ServerChunk)
   ```
   
   Features:
   - **Trees**: Oak (plains/forest), Spruce (mountains)
   - **Vegetation**: Tall grass, cacti
   - **Flowers**: Dandelions, poppies
   - **Water**: Lakes and rivers (if below sea level)

### Noise-Based Generation

**Perlin Noise Parameters**:
```rust
const SCALE: f64 = 0.01;  // Controls terrain frequency
const OCTAVES: usize = 4;  // Detail levels
const PERSISTENCE: f64 = 0.5;  // Amplitude decay
const LACUNARITY: f64 = 2.0;  // Frequency increase
```

**Height Calculation**:
```rust
let mut height = 64;  // Sea level
for octave in 0..OCTAVES {
    let frequency = LACUNARITY.powi(octave);
    let amplitude = PERSISTENCE.powi(octave);
    height += noise.get([x * frequency, z * frequency]) * amplitude;
}
```

### Background Generation

**Location**: `server/src/world/background_generation.rs`

Chunks are generated asynchronously:
```rust
pub fn background_generation_system(
    mut commands: Commands,
    world_map: Res<ServerWorldMap>,
    player_query: Query<&Transform, With<Player>>,
) {
    // Calculate needed chunks based on player positions
    // Generate chunks in background thread
    // Insert when ready
}
```

**Benefits**:
- Doesn't block main game loop
- Generates ahead of player movement
- Prioritizes chunks closest to players

## Client-Side Rendering

### Mesh Generation (Greedy Meshing)

**Location**: `client/src/world/rendering/meshing.rs`

Greedy meshing combines adjacent faces:

```rust
pub fn generate_chunk_mesh(chunk: &ClientChunk) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    
    // For each block
    for (pos, block_data) in &chunk.blocks {
        // Check each face (top, bottom, north, south, east, west)
        for direction in DIRECTIONS {
            if should_render_face(chunk, pos, direction) {
                add_face(&mut vertices, &mut indices, pos, direction, block_data);
            }
        }
    }
    
    create_mesh(vertices, indices)
}
```

**Face Culling**:
```rust
fn should_render_face(chunk: &ClientChunk, pos: IVec3, dir: Direction) -> bool {
    let neighbor_pos = pos + dir.offset();
    
    // Don't render if neighbor is solid and opaque
    if let Some(neighbor) = chunk.blocks.get(&neighbor_pos) {
        if neighbor.is_solid() && !neighbor.is_transparent() {
            return false;
        }
    }
    
    true
}
```

### Texture Mapping

**Location**: `client/src/world/rendering/materials.rs`

Uses a texture atlas:
```rust
fn get_texture_coords(block: BlockId, face: BlockFace) -> [Vec2; 4] {
    match block {
        BlockId::Grass => match face {
            BlockFace::Top => GRASS_TOP_UV,
            BlockFace::Bottom => DIRT_UV,
            BlockFace::Side => GRASS_SIDE_UV,
        },
        BlockId::Stone => STONE_UV,
        // ... other blocks
    }
}
```

### Render Distance

**Location**: `client/src/world/rendering/render_distance.rs`

Dynamic loading/unloading:
```rust
pub fn update_render_distance_system(
    player_pos: Query<&Transform, With<Player>>,
    render_distance: Res<RenderDistance>,
    mut world_map: ResMut<ClientWorldMap>,
) {
    let player_chunk = world_to_chunk_pos(player_pos.translation);
    
    // Load chunks within render distance
    for x in -render_distance..render_distance {
        for z in -render_distance..render_distance {
            let chunk_pos = player_chunk + IVec2::new(x, z);
            if !world_map.chunks.contains_key(&chunk_pos) {
                request_chunk(chunk_pos);
            }
        }
    }
    
    // Unload distant chunks
    world_map.chunks.retain(|pos, _| {
        pos.distance(player_chunk) <= render_distance
    });
}
```

**Controls**:
- `O` key: Decrease render distance
- `P` key: Increase render distance
- Range: 4-32 chunks

### Mesh Updates

Chunks are remeshed when blocks change:
```rust
pub fn remesh_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut query: Query<(&mut ClientChunk, &ChunkEntity)>,
) {
    for (mut chunk, entity) in query.iter_mut() {
        if chunk.needs_remesh {
            let new_mesh = generate_chunk_mesh(&chunk);
            let mesh_handle = meshes.add(new_mesh);
            commands.entity(entity.0).insert(mesh_handle);
            chunk.needs_remesh = false;
        }
    }
}
```

## World Simulation

### Block Physics

**Location**: `server/src/world/simulation.rs`

Simulated block types:
- **Falling blocks**: Sand, gravel (affected by gravity)
- **Water**: Flows downward and spreads
- **Fire**: Spreads to flammable blocks

```rust
pub fn simulate_falling_blocks_system(
    mut world_map: ResMut<ServerWorldMap>,
) {
    for chunk in world_map.chunks.values_mut() {
        let mut blocks_to_move = Vec::new();
        
        for (pos, block) in &chunk.map {
            if block.is_falling() {
                let below = pos + IVec3::new(0, -1, 0);
                if chunk.map.get(&below).is_none() {
                    blocks_to_move.push((*pos, *block, below));
                }
            }
        }
        
        for (old_pos, block, new_pos) in blocks_to_move {
            chunk.map.remove(&old_pos);
            chunk.map.insert(new_pos, block);
        }
    }
}
```

### Time and Day/Night Cycle

**Location**: `client/src/world/time.rs`, `client/src/world/celestial.rs`

```rust
pub struct ClientTime {
    pub ticks: u64,  // Game ticks
    pub day_length: u64,  // Ticks per day (default: 24000)
}

impl ClientTime {
    pub fn time_of_day(&self) -> f32 {
        (self.ticks % self.day_length) as f32 / self.day_length as f32
    }
    
    pub fn is_day(&self) -> bool {
        let tod = self.time_of_day();
        tod > 0.25 && tod < 0.75  // Daytime: 6am-6pm
    }
}
```

**Celestial rendering**:
```rust
pub fn update_sun_position_system(
    time: Res<ClientTime>,
    mut sun_query: Query<&mut Transform, With<Sun>>,
) {
    let angle = time.time_of_day() * 2.0 * PI;
    for mut transform in sun_query.iter_mut() {
        transform.rotation = Quat::from_rotation_x(angle);
    }
}
```

## Persistence (Save/Load)

### Save Format

**Location**: `server/src/world/save.rs`

Uses RON (Rusty Object Notation):
```rust
#[derive(Serialize, Deserialize)]
pub struct WorldSave {
    pub seed: i32,
    pub chunks: Vec<ChunkSave>,
    pub spawn_point: Vec3,
}

#[derive(Serialize, Deserialize)]
pub struct ChunkSave {
    pub position: IVec2,
    pub blocks: HashMap<IVec3, BlockData>,
}
```

### Save System

```rust
pub fn save_world_system(
    world_map: Res<ServerWorldMap>,
    config: Res<GameServerConfig>,
) {
    let save_data = WorldSave {
        seed: world_map.seed,
        chunks: world_map.chunks.iter()
            .map(|(pos, chunk)| ChunkSave {
                position: *pos,
                blocks: chunk.map.clone(),
            })
            .collect(),
        spawn_point: Vec3::new(0.0, 64.0, 0.0),
    };
    
    let path = format!("saves/{}.ron", config.world_name);
    let serialized = ron::to_string(&save_data).unwrap();
    std::fs::write(path, serialized).unwrap();
}
```

### Load System

**Location**: `server/src/world/load_from_file.rs`

```rust
pub fn load_world(world_name: &str) -> Option<ServerWorldMap> {
    let path = format!("saves/{}.ron", world_name);
    let contents = std::fs::read_to_string(path).ok()?;
    let save_data: WorldSave = ron::from_str(&contents).ok()?;
    
    let mut world_map = ServerWorldMap {
        seed: save_data.seed,
        chunks: HashMap::new(),
    };
    
    for chunk_save in save_data.chunks {
        let chunk = ServerChunk {
            position: chunk_save.position,
            map: chunk_save.blocks,
        };
        world_map.chunks.insert(chunk_save.position, chunk);
    }
    
    Some(world_map)
}
```

## Network Synchronization

### World Updates

**Location**: `server/src/world/broadcast_world.rs`

Server sends chunk data to clients:
```rust
pub fn broadcast_chunks_system(
    world_map: Res<ServerWorldMap>,
    mut server: ResMut<RenetServer>,
    player_query: Query<(&Transform, &ClientId), With<Player>>,
) {
    for (player_transform, client_id) in player_query.iter() {
        let player_chunk = world_to_chunk_pos(player_transform.translation);
        
        // Send chunks within player's view distance
        for (chunk_pos, chunk) in &world_map.chunks {
            if chunk_pos.distance(player_chunk) <= VIEW_DISTANCE {
                let message = ServerToClientMessage::WorldUpdate(
                    WorldUpdateMessage {
                        chunk_pos: *chunk_pos,
                        blocks: chunk.map.clone(),
                    }
                );
                send_message(&mut server, *client_id, message);
            }
        }
    }
}
```

### Block Updates

Single block changes are sent as deltas:
```rust
pub fn broadcast_block_change(
    server: &mut RenetServer,
    pos: IVec3,
    block_data: Option<BlockData>,
) {
    let message = ServerToClientMessage::BlockUpdate(
        BlockUpdateMessage { pos, block_data }
    );
    server.broadcast_message(
        STC_STANDARD_CHANNEL,
        game_message_to_payload(&message)
    );
}
```

### Client Reception

**Location**: `client/src/network/world.rs`

```rust
pub fn receive_world_updates_system(
    mut events: EventReader<WorldUpdateEvent>,
    mut world_map: ResMut<ClientWorldMap>,
) {
    for event in events.read() {
        let chunk = ClientChunk {
            blocks: event.blocks.clone(),
            mesh_handle: None,
            needs_remesh: true,
        };
        world_map.chunks.insert(event.chunk_pos, chunk);
    }
}
```

## Performance Optimization

### Chunk Caching

Only active chunks are kept in memory:
- Server: Keeps chunks with nearby players
- Client: Keeps chunks within render distance
- Inactive chunks unloaded after timeout

### Lazy Meshing

Meshes only regenerated when needed:
- Block placed/broken in chunk
- Neighboring chunk loaded
- Manual refresh

### Spatial Optimization

**Octree-like organization** (future improvement):
- Hierarchical spatial structure
- Fast neighbor queries
- Efficient collision detection

### Memory Management

Sparse storage for blocks:
```rust
// Only non-air blocks stored
HashMap<IVec3, BlockData>  // ~ 12-20 bytes per block

// vs. dense storage (wasteful)
[[[BlockData; 256]; 16]; 16]  // ~1 MB per chunk
```

## Usage Examples

### Generating a New World

```rust
let seed = 12345;
let world_map = ServerWorldMap::new(seed);

// Generate spawn chunks
for x in -2..2 {
    for z in -2..2 {
        let chunk_pos = IVec2::new(x, z);
        let chunk = generate_chunk(seed, chunk_pos);
        world_map.chunks.insert(chunk_pos, chunk);
    }
}
```

### Placing a Block

```rust
// Server-side
pub fn place_block(
    world_map: &mut ServerWorldMap,
    pos: IVec3,
    block_id: BlockId,
) -> bool {
    let chunk_pos = IVec2::new(pos.x >> 4, pos.z >> 4);
    if let Some(chunk) = world_map.chunks.get_mut(&chunk_pos) {
        chunk.map.insert(pos, BlockData::new(block_id, BlockDirection::Front));
        true
    } else {
        false
    }
}
```

### Breaking a Block

```rust
pub fn break_block(
    world_map: &mut ServerWorldMap,
    pos: IVec3,
) -> Option<BlockData> {
    let chunk_pos = IVec2::new(pos.x >> 4, pos.z >> 4);
    world_map.chunks.get_mut(&chunk_pos)?.map.remove(&pos)
}
```

### Raycast Block Selection

**Location**: `shared/src/world/raycast.rs`

```rust
pub fn raycast(
    world_map: &ClientWorldMap,
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
) -> Option<RaycastResult> {
    let mut t = 0.0;
    while t < max_distance {
        let pos = origin + direction * t;
        let block_pos = pos.floor().as_ivec3();
        
        if let Some(block) = get_block(world_map, block_pos) {
            if block.is_solid() {
                return Some(RaycastResult {
                    position: block_pos,
                    distance: t,
                    normal: calculate_normal(pos, block_pos),
                });
            }
        }
        
        t += 0.1;  // Step size
    }
    None
}
```

## Future Enhancements

### Planned Features
- [ ] Cave generation (3D noise)
- [ ] More biomes (jungle, swamp, tundra)
- [ ] Water physics (flowing, source blocks)
- [ ] Redstone-like logic system
- [ ] Village generation
- [ ] Underground ores and minerals
- [ ] Vertical chunk loading (for tall worlds)

### Performance Improvements
- [ ] Chunk compression in memory
- [ ] GPU-based terrain generation
- [ ] Chunk streaming from disk

## Troubleshooting

### Chunks Not Loading
**Symptom**: Missing terrain, holes in world  
**Solutions**:
- Check render distance setting
- Verify network connection (multiplayer)
- Check console for generation errors
- Ensure chunks are within loaded area

### Rendering Artifacts
**Symptom**: Z-fighting, missing faces, flickering  
**Solutions**:
- Verify face culling logic
- Check mesh generation for duplicates
- Update neighboring chunks after changes
- Validate texture coordinates

### Generation Performance
**Symptom**: Lag when exploring new areas  
**Solutions**:
- Reduce render distance
- Optimize noise generation
- Profile generation code
- Use background generation
- Cache noise values

### Save/Load Issues
**Symptom**: World not persisting, corrupted saves  
**Solutions**:
- Verify save directory exists
- Check file permissions
- Validate RON format
- Handle serialization errors
- Backup saves before changes

---

The world system is the foundation of Rustcraft. Understanding its generation, rendering, and synchronization is key to extending the game with new features.

# Rustcraft Architecture Documentation

## Table of Contents
- [Overview](#overview)
- [Project Structure](#project-structure)
- [Architecture Pattern](#architecture-pattern)
- [Core Modules](#core-modules)
- [Data Flow](#data-flow)
- [Key Systems](#key-systems)
- [Dependencies](#dependencies)

## Overview

Rustcraft is a Minecraft-inspired voxel game built with Rust and the Bevy game engine. The project follows a client-server architecture with shared code for common functionality. The codebase is organized into three main workspace members:

- **Client**: The game client with rendering, UI, and player input handling
- **Server**: The game server managing world state, multiplayer, and game logic
- **Shared**: Common code used by both client and server (messages, data structures, utilities)

## Project Structure

```
rustcraft/
├── client/             # Client application
│   └── src/
│       ├── camera/     # Camera systems (controller, spawn)
│       ├── entities/   # Entity management (stacks, etc.)
│       ├── input/      # Input handling (keyboard, mouse)
│       ├── mob/        # Client-side mob rendering (fox, spawn)
│       ├── network/    # Network client code
│       ├── player/     # Player controller and interactions
│       ├── ui/         # User interface (HUD, menus, inventory)
│       ├── world/      # World rendering and time systems
│       ├── game.rs     # Game state and setup
│       └── main.rs     # Entry point
│
├── server/             # Server application
│   └── src/
│       ├── mob/        # Server-side mob behavior
│       ├── network/    # Network server code
│       ├── world/      # World generation and management
│       ├── init.rs     # Server initialization
│       ├── lib.rs      # Server library exports
│       └── main.rs     # Entry point
│
├── shared/             # Shared library
│   └── src/
│       ├── messages/   # Network message definitions
│       ├── players/    # Player data and physics
│       ├── world/      # World data structures (blocks, chunks, items)
│       ├── constants.rs
│       ├── utils.rs
│       └── lib.rs
│
├── data/               # Game assets (textures, etc.)
├── docs/               # Documentation and images
└── scripts/            # Build and utility scripts
```

## Architecture Pattern

### Client-Server Model

Rustcraft uses a authoritative server architecture:

1. **Server Authority**: The server is the source of truth for game state
2. **Client Prediction**: Clients predict movement locally for responsiveness
3. **State Synchronization**: Server broadcasts authoritative state to clients
4. **Network Protocol**: Uses `bevy_renet` for reliable ordered networking

### Entity Component System (ECS)

Built on Bevy's ECS architecture:
- **Entities**: Game objects (players, mobs, blocks)
- **Components**: Data attached to entities (position, velocity, inventory)
- **Systems**: Functions that operate on entities with specific components
- **Resources**: Global singleton data (world map, configuration)

### Workspace Organization

The project uses Cargo workspace to share code:
- Shared library provides common types and utilities
- Client and server can be built and run independently
- Clear separation of concerns between client rendering and server logic

## Core Modules

### Client Modules

#### Camera (`client/src/camera/`)
- **controller.rs**: Camera movement and rotation logic
- **spawn.rs**: Camera entity initialization
- **Purpose**: First-person and third-person camera controls

#### Input (`client/src/input/`)
- **keyboard.rs**: Keyboard input mapping and keybindings
- **mouse.rs**: Mouse input for looking and clicking
- **data.rs**: Input action definitions
- **Purpose**: Translates user input into game actions

#### Player (`client/src/player/`)
- **controller.rs**: Player movement and physics
- **interactions.rs**: Block breaking/placing, item interactions
- **labels.rs**: Player name tags and UI labels
- **update.rs**: Player state synchronization
- **Purpose**: Local player control and prediction

#### World Rendering (`client/src/world/rendering/`)
- **meshing.rs**: Converts voxel data to 3D meshes
- **materials.rs**: Block textures and materials
- **render.rs**: Mesh rendering and updates
- **render_distance.rs**: Dynamic chunk loading/unloading
- **voxel.rs**: Voxel-specific rendering logic
- **Purpose**: Efficient rendering of voxel world

#### UI (`client/src/ui/`)
- **hud/**: In-game HUD (hotbar, inventory, chat, debug info)
- **menus/**: Game menus (home, pause, settings, multiplayer)
- **Purpose**: All user interface elements

#### Network (`client/src/network/`)
- **buffered_client.rs**: Client-side input buffering
- **setup.rs**: Network connection initialization
- **world.rs**: Receiving world updates from server
- **chat.rs**: Chat message handling
- **Purpose**: Client-server communication

### Server Modules

#### World (`server/src/world/`)
- **generation.rs**: Procedural world generation with Perlin noise
- **data.rs**: Server-side world state management
- **simulation.rs**: Block physics (gravity, water flow)
- **save.rs**: World persistence to disk
- **load_from_file.rs**: Loading saved worlds
- **background_generation.rs**: Async chunk generation
- **broadcast_world.rs**: Sending world updates to clients
- **stacks.rs**: Item stack management
- **Purpose**: Complete world state authority

#### Network (`server/src/network/`)
- **dispatcher.rs**: Message routing and handling
- **broadcast_chat.rs**: Chat message distribution
- **cleanup.rs**: Connection cleanup
- **extensions.rs**: Network utility extensions
- **Purpose**: Server-side networking

#### Mob (`server/src/mob/`)
- **behavior.rs**: AI and mob movement logic
- **Purpose**: Server-authoritative mob simulation

### Shared Modules

#### Messages (`shared/src/messages/`)
- **auth.rs**: Authentication messages
- **chat.rs**: Chat messages
- **player.rs**: Player state updates
- **world.rs**: World/chunk updates
- **mob.rs**: Mob updates
- **Purpose**: Network protocol definitions

#### World (`shared/src/world/`)
- **blocks.rs**: Block type definitions and properties
- **data.rs**: Chunk and world data structures
- **items.rs**: Item definitions
- **mobs.rs**: Mob type definitions
- **raycast.rs**: Ray casting for block selection
- **utils.rs**: World utility functions
- **Purpose**: Core game data structures

#### Players (`shared/src/players/`)
- **data.rs**: Player state and inventory
- **movement.rs**: Movement physics
- **collision.rs**: Collision detection
- **simulation.rs**: Player physics simulation
- **blocks.rs**: Player-block interactions
- **Purpose**: Player mechanics shared between client and server

## Data Flow

### Game Loop

```
Client:
1. Read player input
2. Predict movement locally
3. Send input to server
4. Receive server updates
5. Reconcile local state
6. Render world

Server:
1. Receive client inputs
2. Simulate world physics
3. Update mob AI
4. Generate new chunks
5. Broadcast updates to clients
6. Save world state
```

### Network Communication

```
Client → Server:
- Player inputs (movement, block interactions)
- Chat messages
- Authentication requests

Server → Client:
- World updates (chunk data, block changes)
- Player positions (all players)
- Mob updates
- Chat messages
- Authentication responses
```

### World Data Pipeline

```
Generation:
Seed → Perlin Noise → Heightmap → Biome → Blocks → Features (trees, etc.)

Storage:
Chunks → Compression → Disk Files

Rendering:
Chunks → Mesh Generation → GPU Buffers → Rendered Frames
```

## Key Systems

### 1. World Generation System

**Location**: `server/src/world/generation.rs`

- Uses Perlin noise for terrain generation
- Generates multiple biomes (Plains, Forest, Mountains, Desert, Ice Plain, Flower Plains)
- Places natural features (trees, cacti, tall grass, flowers)
- Runs asynchronously in background threads

**Key Components**:
- Heightmap generation based on noise
- Biome selection based on temperature/moisture
- Feature placement (trees, vegetation)
- Chunk-based generation (16x16x256 blocks)

### 2. Networking System

**Location**: `client/src/network/`, `server/src/network/`, `shared/src/messages/`

- Uses `bevy_renet` for UDP-based networking
- Multiple channels for different message types:
  - Standard channel: General game messages
  - Chunk data channel: Large world updates
  - Auth channel: Authentication
- LZ4 compression for large messages
- Reliable ordered delivery

**Key Features**:
- Client prediction with server reconciliation
- Buffered input system
- Bandwidth management
- Connection handling and cleanup

### 3. Rendering System

**Location**: `client/src/world/rendering/`

- Greedy meshing algorithm for efficient voxel rendering
- Dynamic render distance adjustment
- Chunk-based rendering with frustum culling
- Texture atlas for block faces
- Wireframe debug mode

**Optimization Techniques**:
- Only render visible chunk faces
- Batch rendering by chunk
- Lazy mesh updates on block changes
- Distance-based LOD (render distance)

### 4. Physics System

**Location**: `shared/src/players/`

- AABB (Axis-Aligned Bounding Box) collision detection
- Gravity and jumping mechanics
- Flying mode support
- Block collision resolution

**Key Components**:
- Player bounding box
- Collision detection with blocks
- Movement integration
- Velocity damping

### 5. Inventory System

**Location**: `shared/src/players/data.rs`, `client/src/ui/hud/inventory/`

- 36-slot inventory (4 rows of 9 slots)
- 9-slot hotbar
- Item stacking
- Drag-and-drop UI
- Server-synchronized state

### 6. Save/Load System

**Location**: `server/src/world/save.rs`, `server/src/world/load_from_file.rs`

- RON (Rusty Object Notation) format for world data
- Saves chunk data, player positions, inventories
- Per-world save files
- Automatic saving on interval

### 7. Day/Night Cycle

**Location**: `client/src/world/celestial.rs`, `client/src/world/time.rs`

- Time synchronized between server and client
- Dynamic lighting based on time of day
- Atmospheric rendering with `bevy_atmosphere`
- Celestial bodies (sun/moon)

## Dependencies

### Core Framework
- **Bevy 0.16**: Game engine providing ECS, rendering, and systems
- **bevy_renet 2.0**: Networking library built on renet

### World Generation
- **noise 0.9**: Perlin noise for terrain generation
- **rand 0.8**: Random number generation

### Serialization
- **serde 1.0**: Serialization framework
- **bincode 1.3**: Binary serialization
- **ron 0.6**: Rusty Object Notation (human-readable config)
- **lz4 1.28**: Compression for network messages

### Client-Specific
- **bevy_atmosphere 0.13**: Atmospheric rendering (sky, fog)
- **bevy-inspector-egui 0.31**: Debug inspector UI
- **bevy_simple_text_input 0.11**: Text input widgets

### Utilities
- **clap 4.5**: Command-line argument parsing
- **log**: Logging framework
- **ulid 1.1**: Unique identifiers (server)

## Build and Configuration

### Build Profiles

**Development** (`profile.dev`):
- Optimizations: Level 1 for main code
- Optimizations: Level 3 for dependencies
- Fast compile times with reasonable runtime performance

**Release** (`profile.release`):
- Full optimizations
- Used for distribution builds

### Cargo Features

The project uses Bevy's dynamic linking feature in development:
```bash
cargo build --features=bevy/dynamic_linking
```

This significantly speeds up incremental compilation.

### Platform-Specific Paths

**Linux**:
- Game data: `$HOME/.local/share/rustcraft`
- Assets: `$HOME/.config/rustcraft`

**Windows**:
- Game data: `%AppData%/rustcraft`
- Assets: `%AppData%/rustcraft/data`

## Performance Considerations

### Optimization Strategies

1. **Chunk-Based Architecture**: World divided into 16x16x256 chunks for efficient loading/unloading
2. **Greedy Meshing**: Combines adjacent faces to reduce polygon count
3. **Culling**: Only render visible chunks and faces
4. **Compression**: LZ4 compression for network messages
5. **Async Generation**: World generation in background threads
6. **ECS Architecture**: Bevy's optimized ECS for efficient entity processing

### Scalability

- Dynamic render distance based on performance
- Background chunk generation doesn't block main thread
- Server can handle multiple concurrent clients
- Efficient delta updates over network

## Security Considerations

- Server-authoritative design prevents most cheating
- Input validation on server side
- Authentication system for multiplayer
- No client-side trust for game state

## Extending the Codebase

### Adding a New Block Type

1. Add variant to `BlockId` enum in `shared/src/world/blocks.rs`
2. Add texture mapping in client rendering code
3. Add crafting recipes if needed
4. Update block properties (solid, transparent, etc.)

### Adding a New Mob Type

1. Add variant to `MobType` enum in `shared/src/world/mobs.rs`
2. Implement behavior in `server/src/mob/behavior.rs`
3. Add rendering in `client/src/mob/`
4. Add spawn logic

### Adding New Network Messages

1. Define message type in `shared/src/messages/`
2. Implement serialization
3. Add handler on server in `server/src/network/dispatcher.rs`
4. Add handler on client in `client/src/network/`

### Adding New UI Elements

1. Create component in `client/src/ui/`
2. Add setup system for spawning UI entities
3. Add update system for UI logic
4. Register systems in game state

## Further Reading

- [Bevy Engine Documentation](https://docs.rs/bevy/)
- [bevy_renet Documentation](https://docs.rs/bevy_renet/)
- [ECS Pattern Overview](https://en.wikipedia.org/wiki/Entity_component_system)
- [Voxel Rendering Techniques](https://0fps.net/2012/06/30/meshing-in-a-minecraft-game/)

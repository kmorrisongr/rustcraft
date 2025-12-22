# Rustcraft Developer Guide

## Table of Contents
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Code Organization](#code-organization)
- [Common Development Tasks](#common-development-tasks)
- [Testing and Debugging](#testing-and-debugging)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)

## Getting Started

### Prerequisites

Before contributing to Rustcraft, ensure you have:

1. **Rust Toolchain** (Nightly)
   ```bash
   rustup install nightly
   rustup default nightly
   rustup component add rustc-codegen-cranelift-preview --toolchain nightly
   ```

2. **Just** (build tool)
   ```bash
   cargo install just
   ```

3. **System Dependencies**
   
   See the main README.md for platform-specific dependencies.

### First Build

```bash
# Clone the repository
git clone https://github.com/c2i-junia/rustcraft
cd rustcraft

# Build and run in debug mode
./run1.sh
```

The first build will take several minutes as it compiles all dependencies. Subsequent builds will be much faster thanks to incremental compilation.

### Understanding the Codebase

Before making changes, familiarize yourself with:

1. **ARCHITECTURE.md**: Understand the overall structure
2. **Client code** (`client/src/`): Rendering, UI, input handling
3. **Server code** (`server/src/`): Game logic, world generation
4. **Shared code** (`shared/src/`): Common types and utilities

## Development Workflow

### Typical Development Cycle

1. **Create a feature branch**
   ```bash
   git checkout -b feat/your-feature-name
   ```

2. **Make your changes**
   - Edit code in your preferred editor
   - Follow existing code style and patterns

3. **Test locally**
   ```bash
   # Run client
   ./run1.sh
   
   # Run server (optional, for multiplayer testing)
   ./run-server.sh
   ```

4. **Format code**
   ```bash
   cargo fmt
   ```

5. **Check for warnings**
   ```bash
   cargo check
   ```

6. **Commit changes**
   ```bash
   git add .
   git commit -m "feat: add your feature"
   ```
   
   Follow [Conventional Commits](https://www.conventionalcommits.org/) specification.

7. **Push and create PR**
   ```bash
   git push origin feat/your-feature-name
   ```

### Hot Reloading (Development Speed)

For faster iteration during development:

```bash
# Use dynamic linking (much faster recompilation)
cargo build --features=bevy/dynamic_linking
```

This feature significantly reduces compile times during development by dynamically linking the Bevy engine.

### Testing Changes

#### Single Player Testing
```bash
./run1.sh  # Runs client in debug mode
```

#### Multiplayer Testing
```bash
# Terminal 1: Run server
./run-server.sh

# Terminal 2: Run client 1
./run1.sh

# Terminal 3: Run client 2 (optional)
./run2.sh
```

#### Release Build Testing
```bash
just generate-release-folder
./release/bin/rustcraft
```

## Code Organization

### Module Structure

The codebase follows a modular architecture with client, server, and shared workspaces. For a complete directory structure, see [ARCHITECTURE.md](./ARCHITECTURE.md) → Project Structure.

Key organizational principles:
- **Client** (`client/src/`): Rendering, UI, input handling, client-side prediction
- **Server** (`server/src/`): Game logic, world generation, authoritative state
- **Shared** (`shared/src/`): Common types, messages, utilities

### File Naming Conventions

- **mod.rs**: Module root, exports public API
- **data.rs**: Data structures and types
- **setup.rs**: Initialization and spawning
- **<feature>.rs**: Feature-specific implementation

### Bevy System Organization

Systems are organized by:
1. **State**: Which `GameState` they run in
2. **Schedule**: When they run (Update, FixedUpdate, etc.)
3. **Order**: Dependencies between systems

Example:
```rust
app.add_systems(
    Update,
    (
        system_a,
        system_b.after(system_a),
        system_c,
    )
    .run_if(in_state(GameState::Playing))
);
```

## Common Development Tasks

### Adding a New Block Type

1. **Define the block** in `shared/src/world/blocks.rs`:
   ```rust
   pub enum BlockId {
       // ... existing blocks
       YourNewBlock,
   }
   ```

2. **Add block properties**:
   ```rust
   impl BlockId {
       pub fn is_solid(&self) -> bool {
           match self {
               // ... existing blocks
               BlockId::YourNewBlock => true,
           }
       }
   }
   ```

3. **Add texture mapping** in `client/src/world/rendering/materials.rs`:
   ```rust
   BlockId::YourNewBlock => {
       // Define texture coordinates
   }
   ```

4. **Test in-game**:
   - Add the block to player inventory
   - Test placement and breaking
   - Verify rendering

### Adding a New Item

1. **Define the item** in `shared/src/world/items.rs`:
   ```rust
   pub enum ItemId {
       // ... existing items
       YourNewItem,
   }
   ```

2. **Add item properties**:
   ```rust
   impl ItemId {
       pub fn max_stack_size(&self) -> u32 {
           match self {
               // ... existing items
               ItemId::YourNewItem => 64,
           }
       }
   }
   ```

3. **Add UI icon** (if different from block texture)

### Adding a New Network Message

1. **Define message** in `shared/src/messages/`:
   ```rust
   #[derive(Serialize, Deserialize, Debug, Clone)]
   pub struct YourMessage {
       pub data: String,
   }
   ```

2. **Add to message enum**:
   ```rust
   pub enum ServerToClientMessage {
       // ... existing messages
       YourMessage(YourMessage),
   }
   ```

3. **Implement server handler** in `server/src/network/dispatcher.rs`:
   ```rust
   ServerToClientMessage::YourMessage(msg) => {
       // Handle message
   }
   ```

4. **Implement client handler** in `client/src/network/`:
   ```rust
   // Process received message
   ```

### Adding a New UI Element

1. **Create component file** in `client/src/ui/`:
   ```rust
   use bevy::prelude::*;
   
   #[derive(Component)]
   pub struct YourUIElement;
   
   pub fn setup_your_ui(mut commands: Commands) {
       commands.spawn((
           Text::new("Your UI"),
           YourUIElement,
       ));
   }
   
   pub fn update_your_ui(
       mut query: Query<&mut Text, With<YourUIElement>>,
   ) {
       for mut text in &mut query {
           // Update UI
       }
   }
   ```

2. **Register systems** in appropriate game state

### Modifying World Generation

1. **Edit generation logic** in `server/src/world/generation.rs`:
   ```rust
   pub fn generate_chunk(seed: i32, chunk_pos: IVec2) -> ServerChunk {
       // Modify terrain generation
   }
   ```

2. **Test generation**:
   - Create new world
   - Explore different biomes
   - Check performance

3. **Consider**:
   - Biome transitions
   - Feature placement (trees, etc.)
   - Performance impact

### Adding a New Mob

1. **Define mob type** in `shared/src/world/mobs.rs`:
   ```rust
   pub enum MobType {
       // ... existing mobs
       YourNewMob,
   }
   ```

2. **Implement behavior** in `server/src/mob/behavior.rs`:
   ```rust
   pub fn your_mob_ai_system(
       mut mob_query: Query<(&mut Transform, &MobType)>,
   ) {
       // Implement AI logic
   }
   ```

3. **Add rendering** in `client/src/mob/`:
   ```rust
   pub fn spawn_your_mob(
       commands: &mut Commands,
       meshes: &mut ResMut<Assets<Mesh>>,
       materials: &mut ResMut<Assets<StandardMaterial>>,
   ) {
       // Spawn mob entity with visuals
   }
   ```

4. **Add spawn logic** in server world generation

## Testing and Debugging

### Debug Tools

Rustcraft includes several built-in debug tools:

#### F3 - FPS Counter
Shows frames per second and performance metrics.

#### F4 - Chunk Debug
Visualizes chunk boundaries and loading status.

#### F5 - Toggle Perspective
Switches between first-person and third-person view.

#### F6 - Block Debug
Shows current block information and coordinates.

#### F7 - Raycast Debug
Visualizes block selection raycast.

### Using the Inspector

The game includes `bevy-inspector-egui` for runtime inspection:

```rust
// The inspector is automatically available in debug builds
// Press the configured key to toggle inspector UI
```

### Logging

The project uses Rust's `log` crate:

```rust
use log::{debug, info, warn, error};

debug!("Detailed debug information");
info!("General information");
warn!("Warning message");
error!("Error message");
```

Logs appear in the console when running the game.

### Common Debugging Scenarios

#### Player Falling Through World
1. Check collision detection in `shared/src/players/collision.rs`
2. Verify chunk is loaded before player spawns
3. Check block solidity properties

#### Rendering Issues
1. Check mesh generation in `client/src/world/rendering/meshing.rs`
2. Verify texture coordinates
3. Check material properties
4. Use F6 to see block data

#### Network Desync
1. Check message serialization/deserialization
2. Verify server authority for state
3. Check client prediction reconciliation
4. Enable network logging

#### Performance Problems
1. Use F3 to monitor FPS
2. Check render distance (O/P keys)
3. Profile with `cargo flamegraph`
4. Check chunk generation performance

### Testing Multiplayer

1. **Start server**:
   ```bash
   ./run-server.sh --port 8000 --world testworld
   ```

2. **Connect multiple clients**:
   ```bash
   # Client 1
   ./run1.sh
   # In-game: Multiplayer → Connect to localhost:8000
   
   # Client 2
   ./run2.sh
   # In-game: Multiplayer → Connect to localhost:8000
   ```

3. **Test scenarios**:
   - Block placement/breaking synchronization
   - Player movement visibility
   - Chat messages
   - Inventory changes
   - World saving/loading

## Best Practices

### Code Style

1. **Follow Rust conventions**:
   - Use `snake_case` for functions and variables
   - Use `PascalCase` for types and enums
   - Use `SCREAMING_SNAKE_CASE` for constants

2. **Format before committing**:
   ```bash
   cargo fmt
   ```

3. **Check for warnings**:
   ```bash
   cargo clippy
   ```

### Bevy-Specific Patterns

1. **Use queries efficiently**:
   ```rust
   // Good: Specific query
   fn system(query: Query<&Transform, With<Player>>) {}
   
   // Avoid: Overly broad query
   fn system(query: Query<&Transform>) {}
   ```

2. **Leverage change detection**:
   ```rust
   fn system(query: Query<&Transform, Changed<Transform>>) {
       // Only runs for changed entities
   }
   ```

3. **Use events for communication**:
   ```rust
   #[derive(Event)]
   struct MyEvent;
   
   fn sender(mut events: EventWriter<MyEvent>) {
       events.send(MyEvent);
   }
   
   fn receiver(mut events: EventReader<MyEvent>) {
       for event in events.read() {
           // Handle event
       }
   }
   ```

### Performance Considerations

1. **Minimize allocations in hot paths**:
   ```rust
   // Avoid in update systems
   let vec = vec![1, 2, 3]; // Allocates every frame
   
   // Prefer
   const ARRAY: [i32; 3] = [1, 2, 3]; // No allocation
   ```

2. **Use appropriate data structures**:
   - `HashMap` for fast lookups
   - `Vec` for sequential access
   - `BTreeMap` for sorted iteration

3. **Batch operations**:
   ```rust
   // Good: Batch mesh updates
   commands.spawn_batch(entities);
   
   // Avoid: Individual spawns in loop
   for entity in entities {
       commands.spawn(entity);
   }
   ```

### Networking Best Practices

1. **Compress large messages**:
   ```rust
   use shared::game_message_to_payload;
   let payload = game_message_to_payload(&message); // Auto-compresses
   ```

2. **Minimize network traffic**:
   - Send deltas, not full state
   - Use appropriate channel for message type
   - Batch updates when possible

3. **Validate server-side**:
   ```rust
   // Always validate client input on server
   if is_valid_position(position) {
       // Update position
   }
   ```

### Error Handling

1. **Use `Result` for fallible operations**:
   ```rust
   fn load_world(path: &Path) -> Result<World, WorldError> {
       // ...
   }
   ```

2. **Log errors appropriately**:
   ```rust
   match load_world(path) {
       Ok(world) => info!("World loaded successfully"),
       Err(e) => error!("Failed to load world: {}", e),
   }
   ```

3. **Handle network errors gracefully**:
   ```rust
   // Don't crash on network errors
   // Show user-friendly error message
   ```

## Troubleshooting

### Compilation Issues

#### "Linker error" or "cannot find -lvulkan"
Install Vulkan drivers and development libraries. See README.md for platform-specific instructions.

#### Slow compilation
Use dynamic linking in development:
```bash
cargo build --features=bevy/dynamic_linking
```

#### "Nightly required" errors
Switch to nightly toolchain:
```bash
rustup default nightly
```

### Runtime Issues

#### Black screen on startup
1. Check Vulkan driver installation
2. Verify GPU compatibility
3. Check console for error messages
4. Try updating graphics drivers

#### Game crashes on world generation
1. Check available memory
2. Reduce render distance
3. Check console for panic messages
4. Verify world save file isn't corrupted

#### Cannot connect to server
1. Verify server is running
2. Check firewall settings
3. Confirm correct IP and port
4. Check server logs for errors

#### Blocks not rendering
1. Verify textures are in `data/` folder
2. Check texture path configuration
3. Use F6 debug to verify block data
4. Check mesh generation logs

### Performance Issues

#### Low FPS
1. Reduce render distance (O key)
2. Close other applications
3. Check GPU utilization
4. Build in release mode for testing

#### Stuttering
1. Check for excessive logging
2. Verify no disk I/O during gameplay
3. Check for memory leaks
4. Profile with performance tools

#### Server lag
1. Check server tick rate
2. Verify network bandwidth
3. Check world generation performance
4. Reduce concurrent chunk generation

## Development Tools

### Recommended IDE Setup

**Visual Studio Code**:
- rust-analyzer extension
- CodeLLDB for debugging
- Better TOML for Cargo.toml

**CLion / RustRover**:
- Built-in Rust support
- Excellent debugging

### Useful Commands

```bash
# Build debug
cargo build

# Build release
cargo build --release

# Run client (debug)
cargo run --bin client

# Run server (debug)
cargo run --bin server

# Check code without building
cargo check

# Format code
cargo fmt

# Lint code
cargo clippy

# Run with specific features
cargo run --features=bevy/dynamic_linking

# Clean build artifacts
cargo clean

# Update dependencies
cargo update

# Generate documentation
cargo doc --open
```

### Profiling

For performance analysis:

```bash
# Install flamegraph
cargo install flamegraph

# Generate flamegraph (Linux)
cargo flamegraph --bin client

# Use perf (Linux)
perf record -g cargo run --release
perf report
```

## Contributing Guidelines

### Before Submitting a PR

1. **Format your code**: `cargo fmt`
2. **Check for warnings**: `cargo check`
3. **Test your changes**: Run the game and verify functionality
4. **Write clear commit messages**: Follow Conventional Commits
5. **Update documentation**: If adding features or changing behavior

### PR Description Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Performance improvement
- [ ] Documentation update

## Testing
How were these changes tested?

## Screenshots (if applicable)
Add screenshots for visual changes
```

### Code Review Process

1. Automated checks run on PR
2. Maintainer review
3. Address feedback
4. Merge when approved

## Resources

### Bevy Learning Resources
- [Official Bevy Book](https://bevyengine.org/learn/book/introduction/)
- [Bevy Examples](https://github.com/bevyengine/bevy/tree/main/examples)
- [Bevy Cheat Book](https://bevy-cheatbook.github.io/)

### Rust Learning Resources
- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [Rustlings (exercises)](https://github.com/rust-lang/rustlings)

### Voxel Game Development
- [Voxel Engine Basics](https://www.youtube.com/watch?v=Ab8TOSFfNp4)
- [Greedy Meshing Explained](https://0fps.net/2012/06/30/meshing-in-a-minecraft-game/)
- [Minecraft Protocol Documentation](https://wiki.vg/Protocol)

## Getting Help

- **GitHub Issues**: Report bugs or request features
- **Discussions**: Ask questions in GitHub Discussions
- **Discord**: Join the community Discord (if available)
- **Code Comments**: Check inline documentation in the code

---

Welcome to the Rustcraft development community! Happy coding! 🦀⛏️

# Refactoring Suggestions for Rustcraft

This document outlines opportunities for refactoring and simplification across the client, shared, and server codebases. Suggestions are organized by priority and potential impact.

---

## Table of Contents
- [High Priority](#high-priority)
- [Medium Priority](#medium-priority)
- [Low Priority](#low-priority)
- [Code Organization](#code-organization)

---

## High Priority

### 1. Duplicate WorldMap Trait Implementations [COMPLETE]

**Files:** 
- [client/src/world/data.rs](../client/src/world/data.rs)
- [shared/src/world/data.rs](../shared/src/world/data.rs)

**Issue:** Both `ClientWorldMap` and `ServerChunkWorldMap` implement the `WorldMap` trait with nearly identical coordinate conversion logic:

```rust
// Repeated in both files
let cx: i32 = block_to_chunk_coord(x);
let cy: i32 = block_to_chunk_coord(y);
let cz: i32 = block_to_chunk_coord(z);
let sub_x: i32 = ((x % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;
// ...
```

**Recommendation:** Extract the common coordinate transformation logic into helper methods in the shared crate:

```rust
// shared/src/world/utils.rs
pub fn global_to_chunk_local(position: &IVec3) -> (IVec3, IVec3) {
    let chunk_pos = global_block_to_chunk_pos(position);
    let local_pos = to_local_pos(position);
    (chunk_pos, local_pos)
}
```

Consider using a generic chunk storage trait that both can implement.

---

### 2. Input Action Mapping Duplication [COMPLETE]

**Files:**
- [client/src/input/data.rs](../client/src/input/data.rs) - `GameAction` enum
- [shared/src/messages/player.rs](../shared/src/messages/player.rs) - `NetworkAction` enum

**Issue:** There are two separate enums for actions - `GameAction` (client-side UI actions) and `NetworkAction` (network-synchronized actions). The mapping between them in [controller.rs](../client/src/player/controller.rs) is repetitive:

```rust
if is_action_pressed(GameAction::MoveBackward, &keyboard_input, &key_map) {
    frame_inputs.0.inputs.insert(NetworkAction::MoveBackward);
}
if is_action_pressed(GameAction::MoveForward, &keyboard_input, &key_map) {
    frame_inputs.0.inputs.insert(NetworkAction::MoveForward);
}
// ... repeated for each action
```

**Recommendation:** Create a mapping table or derive macro that automatically converts between the two:

```rust
// Define action pairs
const ACTION_MAPPING: &[(GameAction, NetworkAction)] = &[
    (GameAction::MoveBackward, NetworkAction::MoveBackward),
    (GameAction::MoveForward, NetworkAction::MoveForward),
    // ...
];

// Use in system
for (game_action, network_action) in ACTION_MAPPING {
    if is_action_pressed(*game_action, &keyboard_input, &key_map) {
        frame_inputs.0.inputs.insert(*network_action);
    }
}
```

---

### 3. Tree Generation Code Repetition [COMPLETE]

**File:** [server/src/world/generation.rs](../server/src/world/generation.rs)

**Issue:** `generate_tree` and `generate_big_tree` functions contain extensive duplicated boundary checking logic:

```rust
// Repeated pattern ~30 times
if x >= 0 && x < CHUNK_SIZE && z >= 0 && z < CHUNK_SIZE && trunk_y >= 0 && trunk_y < CHUNK_SIZE {
    chunk.map.insert(...);
}
```

**Recommendation:** Extract boundary checking into a helper:

```rust
fn try_place_block(chunk: &mut ServerChunk, pos: IVec3, block: BlockData) {
    if (0..CHUNK_SIZE).contains(&pos.x) 
        && (0..CHUNK_SIZE).contains(&pos.y) 
        && (0..CHUNK_SIZE).contains(&pos.z) {
        chunk.map.insert(pos, block);
    }
}
```

Consider a `TreeBuilder` struct pattern for more complex tree generation logic.

---

### 4. Mob Physics Duplicates Player Physics [COMPLETE]

**Files:**
- [shared/src/players/movement.rs](../shared/src/players/movement.rs)
- [server/src/mob/behavior.rs](../server/src/mob/behavior.rs)

**Issue:** Mob movement physics in `mob_behavior_system` duplicates player movement physics, including gravity handling, collision detection, and jump logic.

**Recommendation:** Create a shared `PhysicsBody` component and generic physics simulation system:

```rust
// shared/src/physics/mod.rs
pub struct PhysicsBody {
    pub position: Vec3,
    pub velocity: Vec3,
    pub on_ground: bool,
    pub dimensions: Vec3, // width, height, depth
}

pub fn simulate_physics(body: &mut PhysicsBody, world_map: &impl WorldMap, delta: f32) {
    // Common gravity, collision, velocity clamping logic
}
```

---

## Medium Priority

### 5. System Parameter Tuples in Controllers

**File:** [client/src/player/interactions.rs](../client/src/player/interactions.rs)

**Issue:** Large tuples for queries and resources make the code harder to read:

```rust
pub fn handle_block_interactions(
    queries: (
        Query<&mut Player, With<CurrentPlayerMarker>>,
        Query<&mut Transform, With<CurrentPlayerMarker>>,
        Query<&Transform, (With<Camera>, Without<CurrentPlayerMarker>)>,
        Query<&MobMarker>,
    ),
    resources: (
        ResMut<ClientWorldMap>,
        Res<ButtonInput<MouseButton>>,
        Res<UIMode>,
        Res<ViewMode>,
        ResMut<TargetedMob>,
        ResMut<CurrentFrameInputs>,
    ),
    // ...
)
```

**Recommendation:** Use `SystemParam` derive macro to create reusable parameter bundles:

```rust
#[derive(SystemParam)]
pub struct PlayerQueries<'w, 's> {
    player: Query<'w, 's, &'static mut Player, With<CurrentPlayerMarker>>,
    transform: Query<'w, 's, &'static mut Transform, With<CurrentPlayerMarker>>,
    camera: Query<'w, 's, &'static Transform, (With<Camera>, Without<CurrentPlayerMarker>)>,
}
```

---

### 6. Magic Numbers Throughout Codebase

**Files:** Multiple files contain hardcoded values

**Examples:**
- Camera eye height offset: `0.8` in [camera/controller.rs](../client/src/camera/controller.rs#L54)
- Third-person distance: `10.0` in [camera/controller.rs](../client/src/camera/controller.rs#L65)
- Angle clamp limits: `-89.0f32.to_radians()` repeated multiple times
- Fall limit: `-50.0` in [movement.rs](../shared/src/players/movement.rs#L131)
- Trunk heights: `3`, `4`, etc. in generation.rs

**Recommendation:** Centralize constants in `shared/src/constants.rs` or create domain-specific constant modules:

```rust
// shared/src/constants.rs
pub mod camera {
    pub const EYE_HEIGHT_OFFSET: f32 = 0.8;
    pub const THIRD_PERSON_DISTANCE: f32 = 10.0;
    pub const MAX_PITCH_ANGLE: f32 = 89.0_f32.to_radians();
}

pub mod world {
    pub const FALL_LIMIT: f32 = -50.0;
    pub const MIN_TRUNK_HEIGHT: u8 = 3;
    pub const MAX_TRUNK_HEIGHT: u8 = 5;
}
```

---

### 7. Debug Toggle Systems Are Repetitive [COMPLETE]

**File:** [client/src/player/controller.rs](../client/src/player/controller.rs)

**Issue:** Multiple toggle systems follow identical patterns:

```rust
pub fn toggle_chunk_debug_mode_system(...) {
    if is_action_just_pressed(GameAction::ToggleChunkDebugMode, &keyboard_input, &key_map) {
        debug_options.toggle_chunk_debug_mode();
    }
}

pub fn toggle_raycast_debug_mode_system(...) {
    if is_action_just_pressed(GameAction::ToggleRaycastDebugMode, &keyboard_input, &key_map) {
        debug_options.toggle_raycast_debug_mode();
    }
}
```

**Recommendation:** Create a generic toggle system or use a data-driven approach:

```rust
pub fn toggle_debug_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    mut debug_options: ResMut<DebugOptions>,
) {
    const TOGGLES: &[(GameAction, fn(&mut DebugOptions))] = &[
        (GameAction::ToggleChunkDebugMode, DebugOptions::toggle_chunk_debug_mode),
        (GameAction::ToggleRaycastDebugMode, DebugOptions::toggle_raycast_debug_mode),
    ];
    
    for (action, toggle_fn) in TOGGLES {
        if is_action_just_pressed(*action, &keyboard_input, &key_map) {
            toggle_fn(&mut debug_options);
        }
    }
}
```

---

### 8. `game.rs` Plugin Registration is Monolithic

**File:** [client/src/game.rs](../client/src/game.rs)

**Issue:** `game_plugin` is 200+ lines with all resources, events, and systems registered in one function. This makes it difficult to understand system ordering and dependencies.

**Recommendation:** Split into sub-plugins by feature:

```rust
pub fn game_plugin(app: &mut App) {
    app.add_plugins((
        GameResourcesPlugin,
        GameInputPlugin,
        GameWorldPlugin,
        GameUIPlugin,
        GameNetworkPlugin,
        GameDebugPlugin,
    ));
}

// client/src/plugins/input.rs
pub struct GameInputPlugin;

impl Plugin for GameInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            update_frame_inputs_system,
            handle_block_interactions,
            player_movement_system,
            camera_control_system,
        ).chain().run_if(in_state(GameState::Game)));
    }
}
```

---

### 9. Chunk Sent Tracking Uses Vec Instead of HashSet [COMPLETE]

**File:** [shared/src/world/data.rs](../shared/src/world/data.rs)

**Issue:** `ServerChunk.sent_to_clients: Vec<PlayerId>` uses linear search for contains checks:

```rust
if chunk.sent_to_clients.contains(&player.id) {
    continue;
}
```

**Recommendation:** Use `HashSet<PlayerId>` for O(1) lookups:

```rust
pub struct ServerChunk {
    pub map: HashMap<IVec3, BlockData>,
    pub ts: u64,
    pub sent_to_clients: HashSet<PlayerId>, // Changed from Vec
}
```

---

### 10. French Comments in Codebase [COMPLETE]

**File:** [shared/src/players/data.rs](../shared/src/players/data.rs)

**Issue:** Mixed language comments reduce readability for international contributors:

```rust
// Ajoute un item à l'inventaire du joueur
pub fn add_item_to_inventory(&mut self, mut stack: ItemStack) {
```

**Recommendation:** Translate all comments to English for consistency.

---

## Low Priority

### 11. Unused/Dead Code [COMPLETE]

**Files:** Various

**Examples:**
- `bounce_ray` function in [interactions.rs](../client/src/player/interactions.rs#L100) - appears to be debug code that draws nothing
- Commented-out code blocks in multiple files (e.g., debug prints in movement.rs)
- `BlockId::is_biome_colored()` always returns `false` in [blocks.rs](../shared/src/world/blocks.rs#L104)

**Recommendation:** 
- Remove unused debug code or gate behind a feature flag
- Run `cargo clippy` with `warn(dead_code)` to identify unused items
- Delete commented-out code (use version control to recover if needed)

---

### 12. Error Handling with `.unwrap()`

**Files:** Multiple files use `.unwrap()` on Results and Options

**Examples:**
- [setup.rs](../client/src/network/setup.rs#L191): `socket.local_addr().unwrap()`
- [init.rs](../server/src/init.rs#L75): `socket.local_addr().unwrap()`
- Query unwraps throughout controller code

**Recommendation:** Add proper error handling or use `expect()` with descriptive messages:

```rust
// Instead of
let addr = socket.local_addr().unwrap();

// Use
let addr = socket.local_addr().expect("Failed to get socket address");

// Or handle gracefully
let addr = match socket.local_addr() {
    Ok(addr) => addr,
    Err(e) => {
        error!("Failed to get socket address: {}", e);
        return;
    }
};
```

---

### 13. Inconsistent Query Error Handling [PARTIAL-COMPLETE]

**Files:** [client/src/player/controller.rs](../client/src/player/controller.rs), [camera/controller.rs](../client/src/camera/controller.rs)

**Issue:** Mixed patterns for handling query results:

```rust
// Pattern 1: Using .single().unwrap()
let camera = camera.single().unwrap();

// Pattern 2: Checking .is_err()
let res = player_query.single_mut();
if res.is_err() {
    debug!("player not found");
    return;
}
let (mut player, mut player_transform) = player_query.single_mut().unwrap();
```

**Recommendation:** Standardize on one pattern, preferably using `let-else`:

```rust
let Ok((mut player, mut player_transform)) = player_query.single_mut() else {
    debug!("Player not found");
    return;
};
```

---

### 14. HUD Setup Code Repetition

**File:** [client/src/ui/hud/debug/setup.rs](../client/src/ui/hud/debug/setup.rs)

**Issue:** Text node creation is repetitive:

```rust
let default_text_bundle = || {
    (
        Text::new("..."),
        TextFont { font_size: 16.0, ..default() },
        TextColor(Color::WHITE),
    )
};
let coords_text = commands.spawn((CoordsText, default_text_bundle())).id();
let blocks_number_text = commands.spawn((BlocksNumberText, default_text_bundle())).id();
// ... repeated many times
```

**Recommendation:** Create a builder pattern or macro for HUD text elements:

```rust
fn spawn_debug_text<T: Component>(
    commands: &mut Commands,
    marker: T,
    initial_text: &str,
) -> Entity {
    commands.spawn((
        marker,
        Text::new(initial_text),
        TextFont { font_size: 16.0, ..default() },
        TextColor(Color::WHITE),
    )).id()
}
```

---

### 15. Keybinding Defaults Defined Twice [COMPLETE]

**File:** [client/src/input/keyboard.rs](../client/src/input/keyboard.rs)

**Issue:** Default keybindings are defined in code and also expected to be overridden from a file. The defaults are verbose and could conflict with saved bindings.

**Recommendation:** 
- Use `#[derive(Default)]` with `serde(default)` for cleaner defaults
- Consider a builder pattern for keybinding configuration
- Ensure saved bindings fully override (not merge with) defaults

---

## Code Organization

### 16. Module Re-exports Could Be Cleaner

**Files:** Various `mod.rs` files

**Issue:** Many mod.rs files use verbose `pub use *` patterns:

```rust
pub use materials::*;
pub use render::*;
pub use render_distance::*;
```

**Recommendation:** Be more selective with re-exports to reduce namespace pollution and improve compile times. Consider using `pub use module::SpecificType` for commonly used items only.

---

### 17. Consider Feature Flags for Debug Systems

**Files:** Debug-related code throughout client

**Recommendation:** Gate debug systems behind a `debug-tools` feature flag:

```toml
# Cargo.toml
[features]
debug-tools = []
```

```rust
#[cfg(feature = "debug-tools")]
app.add_systems(Update, (
    fps_text_update_system,
    coords_text_update_system,
    chunk_ghost_update_system,
    // ...
));
```

---

## Summary

| Priority | Issue | Impact | Effort |
|----------|-------|--------|--------|
| High | WorldMap trait duplication | Maintainability | Medium |
| High | Input action mapping | DRY principle | Low |
| High | Tree generation repetition | Maintainability | Medium |
| High | Mob/Player physics duplication | Maintainability, Bugs | High |
| Medium | Large system parameter tuples | Readability | Medium |
| Medium | Magic numbers | Maintainability | Low |
| Medium | Debug toggle repetition | DRY principle | Low |
| Medium | Monolithic game plugin | Maintainability | Medium |
| Medium | Vec for sent_to_clients | Performance | Low |
| Medium | French comments | Accessibility | Low |
| Low | Dead code | Code cleanliness | Low |
| Low | `.unwrap()` usage | Robustness | Medium |
| Low | Inconsistent query handling | Consistency | Low |
| Low | HUD setup repetition | DRY principle | Low |
| Low | Keybinding defaults | Configuration | Low |

---

## Additional Suggestions (December 2025)

### 18. Asset Loading Functions Are Repetitive

**File:** [client/src/ui/assets.rs](../client/src/ui/assets.rs)

**Issue:** Multiple `load_*` functions follow identical patterns:

```rust
pub fn load_play_icon(asset_server: &Res<AssetServer>) -> Handle<Image> {
    asset_server.load(PLAY_ICON_PATH)
}
pub fn load_trash_icon(asset_server: &Res<AssetServer>) -> Handle<Image> {
    asset_server.load(TRASH_ICON_PATH)
}
// ... 6 more identical functions
```

**Recommendation:** Use a macro or generic function to reduce boilerplate:

```rust
macro_rules! asset_loader {
    ($fn_name:ident, $path:expr, $type:ty) => {
        pub fn $fn_name(asset_server: &Res<AssetServer>) -> Handle<$type> {
            asset_server.load($path)
        }
    };
}

asset_loader!(load_play_icon, PLAY_ICON_PATH, Image);
asset_loader!(load_trash_icon, TRASH_ICON_PATH, Image);
```

Or consolidate into a single `AssetPaths` struct with lazy loading.

---

### 19. TODO/FIXME Comments Need Resolution

**Files:** Multiple

**Issue:** There are 12+ TODO/FIXME comments scattered throughout the codebase that indicate incomplete features or known issues:

- `client/src/player/interactions.rs:72` - "TODO: Attack the targeted"
- `client/src/mob/fox.rs:282` - "TODO: only update the color of the targeted mob"
- `client/src/network/setup.rs:92` - "TODO: change username"
- `server/src/mob/behavior.rs:21` - "TODO: FIX mob position"
- `server/src/network/dispatcher.rs:145` - "TODO: add cleanup system if no heartbeat"
- `server/src/network/dispatcher.rs:198` - "TODO: add permission checks"

**Recommendation:** 
- Triage TODOs into GitHub issues with proper priority labels
- Remove resolved TODOs or add issue references (e.g., `// TODO(#123): ...`)
- Consider using `todo!()` macro for critical unimplemented paths

---

### 20. `#[allow(dead_code)]` Annotations Mask Unused Code

**Files:** 
- [client/src/mob/mod.rs](../client/src/mob/mod.rs)
- [server/src/init.rs](../server/src/init.rs)

**Issue:** Multiple `#[allow(dead_code)]` annotations hide potentially unused fields:

```rust
pub struct MobRoot {
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub id: u128,
}
```

**Recommendation:**
- If fields are used for debugging, gate behind `#[cfg(debug_assertions)]`
- If fields are planned for future use, document the intended purpose
- If fields are genuinely unused, remove them
- Audit all `#[allow(dead_code)]` annotations periodically

---

### 21. Broadcast World Clones Entire Mobs Collection

**File:** [server/src/world/broadcast_world.rs](../server/src/world/broadcast_world.rs#L92)

**Issue:** The broadcast system clones the entire mobs collection on every update:

```rust
let mobs = world_map.mobs.clone();
// ... later ...
mobs: mobs.clone(),
```

**Recommendation:** 
- Use `Arc<HashMap>` or `Rc` for shared read access
- Only send mob updates for mobs near each player (already partially done)
- Consider delta updates instead of full state

---

### 22. Constants Scattered Across Multiple Files

**Files:** 
- [shared/src/constants.rs](../shared/src/constants.rs)
- [client/src/constants.rs](../client/src/constants.rs)
- [server/src/world/broadcast_world.rs](../server/src/world/broadcast_world.rs#L16-L44)

**Issue:** Game constants are defined in multiple locations with similar names:

```rust
// shared/src/constants.rs
pub const DEFAULT_RENDER_DISTANCE: i32 = 8;

// client/src/constants.rs  
pub const DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS: u32 = if cfg!(debug_assertions) { 2 } else { 4 };

// server/src/world/broadcast_world.rs
const MAX_CHUNKS_PER_UPDATE: usize = 50;
const CHUNKS_PER_RENDER_DISTANCE: i32 = 6;
```

**Recommendation:** 
- Consolidate all game configuration constants into `shared/src/constants.rs`
- Use sub-modules for organization: `constants::rendering`, `constants::physics`, etc.
- Consider a configuration file (RON) for runtime-adjustable values

---

### 23. Mob Behavior Has Duplicated Movement Logic

**File:** [server/src/mob/behavior.rs](../server/src/mob/behavior.rs)

**Issue:** The `MobAction::Walk` branch contains repeated movement attempts with similar patterns:

```rust
if !try_move(&mut body, &world_map.chunks, displacement, true) {
    // ...
} else if body.on_ground && (body.velocity.x != 0.0 && body.velocity.z != 0.0) {
    // jump logic
} else if body.on_ground {
    // Try to move in the other direction
    if !try_move(&mut body, &world_map.chunks, Vec3::new(displacement.x, 0.0, 0.0), true) {
        // ...
    } else if !try_move(&mut body, &world_map.chunks, Vec3::new(0.0, 0.0, displacement.z), true) {
        // ...
    } else {
        // jump again
    }
}
```

**Recommendation:** Extract pathfinding/obstacle avoidance into a dedicated helper:

```rust
fn attempt_movement_with_avoidance(
    body: &mut PhysicsBody,
    chunks: &ServerChunkWorldMap,
    displacement: Vec3,
) -> MovementResult {
    // Centralized movement + obstacle avoidance logic
}
```

---

### 24. Block Properties Use Large Match Statements

**File:** [shared/src/world/blocks.rs](../shared/src/world/blocks.rs)

**Issue:** Each `BlockId` method uses a match statement that must be updated for every new block:

```rust
pub fn get_break_time(&self) -> u8 {
    6 * match *self {
        Self::Dirt => 5,
        Self::Debug => 7,
        Self::Grass => 6,
        // ... 15+ more cases
        _ => 100,
    }
}
```

**Recommendation:** Use a data-driven approach with a static lookup table:

```rust
struct BlockProperties {
    break_time: u8,
    hitbox: BlockHitbox,
    visibility: BlockTransparency,
    drops: &'static [(u32, ItemId, u32)],
}

static BLOCK_PROPERTIES: phf::Map<BlockId, BlockProperties> = phf_map! {
    BlockId::Dirt => BlockProperties { break_time: 30, ... },
    // ...
};

impl BlockId {
    pub fn properties(&self) -> &'static BlockProperties {
        BLOCK_PROPERTIES.get(self).unwrap_or(&DEFAULT_PROPERTIES)
    }
}
```

---

### 25. Menu System Has Deep Nesting in `menu_plugin`

**File:** [client/src/ui/menus/mod.rs](../client/src/ui/menus/mod.rs)

**Issue:** The `menu_plugin` function chains multiple `add_systems` calls with inconsistent structure. Some menus have setup/action pairs, others don't.

**Recommendation:** Create a `MenuPlugin` trait for consistent menu implementation:

```rust
trait MenuPlugin {
    fn state() -> MenuState;
    fn setup_system() -> impl IntoSystemConfigs<()>;
    fn update_systems() -> impl IntoSystemConfigs<()>;
    fn on_exit_systems() -> Option<impl IntoSystemConfigs<()>> { None }
}

// Then register uniformly:
fn register_menu<M: MenuPlugin>(app: &mut App) {
    app.add_systems(OnEnter(M::state()), M::setup_system())
       .add_systems(Update, M::update_systems().run_if(in_state(M::state())));
    if let Some(exit) = M::on_exit_systems() {
        app.add_systems(OnExit(M::state()), exit);
    }
}
```

---

### 26. Network Message Handling Could Use Command Pattern

**File:** [server/src/network/dispatcher.rs](../server/src/network/dispatcher.rs)

**Issue:** The `server_update_system` has a large match statement handling all message types inline:

```rust
match message {
    ClientToServerMessage::AuthRegisterRequest(auth_req) => {
        // 40+ lines of handling
    }
    ClientToServerMessage::ChatMessage(chat_msg) => {
        // 15+ lines
    }
    // ... more cases
}
```

**Recommendation:** Use a handler map or command pattern:

```rust
trait MessageHandler<M> {
    fn handle(&self, ctx: &mut MessageContext, msg: M);
}

// Register handlers
let handlers: HashMap<TypeId, Box<dyn MessageHandler>> = ...;

// Dispatch
if let Some(handler) = handlers.get(&message.type_id()) {
    handler.handle(&mut ctx, message);
}
```

---

### 27. Consider Extracting Common UI Patterns

**Files:** Various UI files in `client/src/ui/`

**Issue:** Button creation, text styling, and layout patterns are repeated across menus. Each menu manually constructs similar UI hierarchies.

**Recommendation:** Create a UI builder/factory module:

```rust
// client/src/ui/builder.rs
pub struct UiBuilder<'a> {
    commands: &'a mut Commands,
    asset_server: &'a AssetServer,
}

impl<'a> UiBuilder<'a> {
    pub fn menu_button(&mut self, text: &str, action: MenuButtonAction) -> Entity { ... }
    pub fn text_input(&mut self, placeholder: &str) -> Entity { ... }
    pub fn scrollable_list(&mut self) -> Entity { ... }
}
```

---

## Summary (Updated)

| Priority | Issue | Impact | Effort |
|----------|-------|--------|--------|
| High | WorldMap trait duplication | Maintainability | Medium |
| High | Input action mapping | DRY principle | Low |
| High | Tree generation repetition | Maintainability | Medium |
| High | Mob/Player physics duplication | Maintainability, Bugs | High |
| Medium | Large system parameter tuples | Readability | Medium |
| Medium | Magic numbers | Maintainability | Low |
| Medium | Debug toggle repetition | DRY principle | Low |
| Medium | Monolithic game plugin | Maintainability | Medium |
| Medium | Vec for sent_to_clients | Performance | Low |
| Medium | French comments | Accessibility | Low |
| Medium | Asset loading repetition | DRY principle | Low |
| Medium | TODO comments need resolution | Technical debt | Medium |
| Medium | Broadcast clones mobs | Performance | Medium |
| Medium | Block properties match statements | Maintainability | Medium |
| Low | Dead code | Code cleanliness | Low |
| Low | `.unwrap()` usage | Robustness | Medium |
| Low | Inconsistent query handling | Consistency | Low |
| Low | HUD setup repetition | DRY principle | Low |
| Low | Keybinding defaults | Configuration | Low |
| Low | `#[allow(dead_code)]` audit | Code cleanliness | Low |
| Low | Constants scattered | Organization | Low |
| Low | Menu system structure | Maintainability | Medium |
| Low | Network message handling | Extensibility | High |
| Low | UI pattern extraction | DRY principle | High |

---

## Next Steps

1. Start with high-priority items that have low effort (input action mapping, tree generation helper)
2. Create tracking issues for larger refactors (physics unification, plugin split)
3. Run `cargo clippy --all-targets` to identify additional issues
4. Consider adding CI checks for code quality metrics
5. **NEW:** Triage TODO comments into GitHub issues
6. **NEW:** Audit `#[allow(dead_code)]` annotations
7. **NEW:** Profile broadcast system for performance bottlenecks

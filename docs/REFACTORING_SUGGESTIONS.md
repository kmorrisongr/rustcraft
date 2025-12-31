# Refactoring Suggestions for Rustcraft

This document outlines opportunities for refactoring and simplification across the client, shared, and server codebases. Suggestions are organized by priority and potential impact.

---

## Table of Contents
- [Medium Priority](#medium-priority)
- [Low Priority](#low-priority)
- [Code Organization](#code-organization)

---

## Medium Priority

### 1. Magic Numbers Throughout Codebase

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

### 2. `game.rs` Plugin Registration is Monolithic

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

## Low Priority

### 3. Inconsistent Query Error Handling [PARTIAL-COMPLETE]

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

## Code Organization

### 4. Module Re-exports Could Be Cleaner

**Files:** Various `mod.rs` files

**Issue:** Many mod.rs files use verbose `pub use *` patterns:

```rust
pub use materials::*;
pub use render::*;
pub use render_distance::*;
```

**Recommendation:** Be more selective with re-exports to reduce namespace pollution and improve compile times. Consider using `pub use module::SpecificType` for commonly used items only.

---

### 5. Consider Feature Flags for Debug Systems

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

## Additional Suggestions (December 2025)

### 6. Asset Loading Functions Are Repetitive

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

### 7. TODO/FIXME Comments Need Resolution

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

### 8. `#[allow(dead_code)]` Annotations Mask Unused Code

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

### 9. Broadcast World Clones Entire Mobs Collection

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

### 10. Constants Scattered Across Multiple Files

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

### 11. Block Properties Use Large Match Statements

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

### 12. Menu System Has Deep Nesting in `menu_plugin`

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

### 13. Network Message Handling Could Use Command Pattern

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

### 14. Consider Extracting Common UI Patterns

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

## Next Steps

1. Start with medium-priority items that have low effort (magic numbers, monolithic game plugin)
2. Create tracking issues for larger refactors (data-driven block properties, UI pattern extraction)
3. Run `cargo clippy --all-targets` to identify additional issues
4. Consider adding CI checks for code quality metrics
5. Triage TODO comments into GitHub issues
6. Audit `#[allow(dead_code)]` annotations
7. Profile broadcast system for performance bottlenecks

# Custom Terrain Generation System - Implementation Plan

## Table of Contents
- [Executive Summary](#executive-summary)
- [Goals and Non-Goals](#goals-and-non-goals)
- [Design Philosophy](#design-philosophy)
- [Technology Choices](#technology-choices)
- [System Architecture](#system-architecture)
- [Detailed Specifications](#detailed-specifications)
- [File Structure](#file-structure)
- [Implementation Phases](#implementation-phases)
- [Security & Sandboxing](#security--sandboxing)
- [Implementation Gotchas](#implementation-gotchas)
- [Migration Path](#migration-path)
- [Testing Strategy](#testing-strategy)
- [Future Extensions](#future-extensions)
- [Appendix: Quick Reference](#appendix-quick-reference)
- [Summary](#summary)

---

## Executive Summary

This document outlines a plan to make Rustcraft's terrain generation fully customizable through a **hybrid data + scripting approach**:

- **Simple biomes**: Defined entirely in RON configuration files (no code required)
- **Complex biomes**: Use [Rhai](https://rhai.rs/) scripts for custom generation logic

The system will support:

1. **Custom biomes** with configurable terrain, layers, and flora
2. **Height-based biomes** for underground caves, sky islands, etc.
3. **Custom terrain algorithms** via sandboxed Rhai scripts
4. **Rule overrides** for existing biomes (e.g., sea level, height variation)
5. **Safe extensibility** with resource limits and API sandboxing

The approach prioritizes **simplicity** (data-first, scripts when needed), **safety** (sandboxed execution), and **expressiveness** (full scripting for power users).

---

## Goals and Non-Goals

### Goals
- Allow modders to define new biomes via configuration files (simple cases)
- Support custom terrain algorithms via sandboxed Rhai scripts (complex cases)
- Enable modification of global world parameters (sea level, biome distribution)
- Provide built-in noise functions accessible from scripts (`perlin`, `ridged`, etc.)
- Maintain backward compatibility with existing worlds
- Ensure all scripting is sandboxed (no file/network/system access)

### Non-Goals
- Arbitrary code execution outside sandbox (security requirement)
- Custom block types (separate feature requiring texture/rendering changes)
- Custom mob spawning rules (separate system)
- Visual scripting / node-based editor (potential future enhancement)

---

## Design Philosophy

### Data-First, Scripts When Needed
The system uses a **progressive complexity** model:

1. **Level 1 - Data Only**: Simple biomes defined entirely in RON files
   - No scripting knowledge required
   - Covers 80% of use cases (adjust heights, blocks, flora chances)
   
2. **Level 2 - Scripted**: Complex biomes add Rhai scripts for custom logic
   - Full control over terrain generation
   - Access to noise functions, math, conditionals
   - Still sandboxed (no file/network access)

### Why Not Pure Data?
A custom declarative DSL for noise composition would require:
- Building a custom interpreter (~500+ lines)
- Designing DSL syntax (learning curve for modders)
- Extending DSL for every new feature

Rhai provides:
- Familiar JavaScript-like syntax
- ~50 lines to integrate
- Modders can implement features we haven't anticipated

### Graceful Degradation
Invalid or missing configurations fall back to defaults with warnings logged:
- Unknown biome references → use `Plains`
- Script errors → log error, use data-driven fallback
- Out-of-range values → clamp to valid range

---

## Technology Choices

### Configuration Format: RON

**Rationale:**
- Integrates cleanly with Rust via serde (widely used in Rust ecosystem)
- Native Rust integration via serde
- Human-readable and editor-friendly
- Supports comments (unlike JSON)

### Scripting Runtime: Rhai

[Rhai](https://rhai.rs/) is a Rust-native scripting language designed for embedding.

**Why Rhai:**
- **Pure Rust**: No external dependencies or FFI
- **Safe by default**: No file/network/system access built-in
- **Familiar syntax**: JavaScript-like, easy for modders
- **Resource limits**: Built-in max operations, memory, call depth
- **Fast integration**: ~50 lines to set up
- **Bevy ecosystem**: Used by other Bevy projects

**Alternatives Considered:**
| Technology | Pros | Cons |
|------------|------|------|
| Lua | Well-known, fast (LuaJIT) | External dependency, more sandboxing work |
| WASM | Near-native speed, strong isolation | Complex toolchain for modders |
| Custom DSL | Maximum safety | High implementation cost |

---

## System Architecture

> **Note:** Rustcraft uses 16×16×16 chunks (not Minecraft's 16×16×320 sections). This means vertical biome boundaries can align cleanly with chunk boundaries, simplifying chunk generation and enabling efficient per-chunk biome caching.

### High-Level Data Flow

```
┌─────────────────────────────────────────────────────────┐
│  terrain_config/                                         │
│  ├── world.ron           (global settings + biome map)   │
│  ├── biomes/                                            │
│  │   ├── plains.ron      (simple - data only)            │
│  │   ├── caves.ron       (underground biome)             │
│  │   └── volcanic.ron    (complex - references script)   │
│  └── scripts/                                           │
│      └── volcanic.rhai   (custom generation logic)       │
└─────────────────────────────────────────────────────────┘
                      │ Load & Validate
                      ▼
┌─────────────────────────────────────────────────────────┐
│  TerrainConfig (Bevy Resource)                          │
│  ├── WorldSettings                                      │
│  ├── BiomeRegistry (HashMap<String, BiomeConfig>)       │
│  └── ScriptEngine (Rhai Engine)                         │
│      ├── Compiled scripts (cached AST)                  │
│      └── Exposed API (perlin, ridged, lerp, etc.)       │
└─────────────────────────────────────────────────────────┘
                      │ Generate Chunks (16×16×16)
                      ▼
┌─────────────────────────────────────────────────────────┐
│  ChunkGenerator                                         │
│  For each block position (x, y, z):                     │
│    1. Determine biome at (x, y, z)                      │
│       a. Filter biome_climate_map by y_range            │
│       b. Among matching entries, find closest climate   │
│       c. More specific y_range wins ties                │
│    2. If biome.script exists:                           │
│         height = call script.get_height(x, z, seed)     │
│       Else:                                             │
│         height = data-driven calculation                │
│    3. Place blocks according to biome layers            │
│    4. Generate flora based on biome rules               │
└─────────────────────────────────────────────────────────┘
```

### Module Structure

```
shared/src/
├── terrain/
│   ├── mod.rs           # Module exports
│   ├── config.rs        # RON configuration structures
│   ├── scripting.rs     # Rhai engine setup and API
│   ├── noise.rs         # Noise functions exposed to scripts
│   └── loader.rs        # File loading and validation

server/src/world/
├── generation.rs        # Modified to use TerrainConfig
└── terrain_loader.rs    # Server initialization
```

---

## Detailed Specifications

### 1. World Settings (`terrain_config/world.ron`)

Global parameters that affect all terrain generation:

```ron
// terrain_config/world.ron
(
    // Core terrain parameters
    sea_level: 62,
    bedrock_level: 0,
    world_height: 256,
    
    // Biome distribution
    biome_scale: 0.01,           // Controls biome size (smaller = larger biomes)
    temperature_seed_offset: 1,
    humidity_seed_offset: 2,
    
    // Biome climate mapping (point-based, closest wins within y_range)
    // y_range is optional: None means all heights, Some((min, max)) restricts to that range
    // When multiple biomes match at a position, the one with the narrowest y_range wins
    biome_climate_map: [
        // Underground biomes (selected by y, not climate)
        (biome: "deep_caves",      climate: (temp: 0.5, humid: 0.5), y_range: Some((0, 20))),
        (biome: "caves",           climate: (temp: 0.5, humid: 0.5), y_range: Some((20, 50))),
        
        // Surface biomes (default y_range covers surface and above)
        (biome: "deep_ocean",      climate: (temp: 0.5, humid: 0.95), y_range: None),
        (biome: "ocean",           climate: (temp: 0.5, humid: 0.85), y_range: None),
        (biome: "shallow_ocean",   climate: (temp: 0.5, humid: 0.75), y_range: None),
        (biome: "desert",          climate: (temp: 0.8, humid: 0.2),  y_range: None),
        (biome: "forest",          climate: (temp: 0.7, humid: 0.5),  y_range: None),
        (biome: "plains",          climate: (temp: 0.5, humid: 0.4),  y_range: None),
        (biome: "flower_plains",   climate: (temp: 0.5, humid: 0.55), y_range: None),
        (biome: "medium_mountain", climate: (temp: 0.4, humid: 0.3),  y_range: None),
        (biome: "high_mountain_grass",  climate: (temp: 0.2, humid: 0.2),  y_range: None),
        (biome: "ice_plain",       climate: (temp: 0.1, humid: 0.4),  y_range: None),
        
        // Sky biomes (high altitude only)
        (biome: "sky_islands",     climate: (temp: 0.5, humid: 0.3), y_range: Some((200, 256))),
    ],
)
```

### 2. Simple Biome (Data-Only)

Biomes that don't need custom logic use pure RON configuration:

```ron
// terrain_config/biomes/plains.ron
(
    id: "plains",
    display_name: "Plains",
    
    // Terrain shape (data-driven)
    terrain: (
        base_height: 64,
        height_variation: 4,
        noise_scale: 0.01,       // Perlin noise scale
    ),
    
    // Block layers (depth from surface)
    layers: [
        (depth: 0, block: "Grass"),
        (depth: 1, block: "Dirt"),
        (depth: 2, block: "Dirt"),
        (depth: 3, block: "Dirt"),
        // Below: Stone (default)
    ],
    
    // Flora generation rules
    flora: [
        (
            flora_type: Flower,
            chance: 0.02,
            surface_blocks: ["Grass"],
            variants: [(block: "Dandelion", weight: 1), (block: "Poppy", weight: 1)],
        ),
        (
            flora_type: TallGrass,
            chance: 0.1,
            surface_blocks: ["Grass"],
        ),
    ],
    
    // No script = uses data-driven generation
    script: None,
)
```

### 3. Complex Biome (With Script)

Biomes needing custom terrain logic reference a Rhai script:

```ron
// terrain_config/biomes/volcanic.ron
(
    id: "volcanic",
    display_name: "Volcanic Islands",
    
    // Fallback values (used if script fails)
    terrain: (
        base_height: 45,
        height_variation: 25,
        noise_scale: 0.08,
    ),
    
    layers: [
        (depth: 0, block: "Stone"),
        (depth: 1, block: "Stone"),
    ],
    
    flora: [],  // Barren landscape
    
    // Reference to script for custom logic
    script: Some("volcanic.rhai"),
)
```

### 3b. Underground Biome Example

Biomes with `y_range` in the climate map control underground generation:

```ron
// terrain_config/biomes/caves.ron
(
    id: "caves",
    display_name: "Caves",
    
    // Underground biomes typically don't define terrain height
    // (surface biome determines that), just block placement
    terrain: (
        base_height: 0,       // Not used for underground biomes
        height_variation: 0,
        noise_scale: 0.1,
    ),
    
    // Layers define what blocks appear at this depth
    layers: [
        (depth: 0, block: "Stone"),
    ],
    
    flora: [],
    
    // Script can carve caves, place ores, etc.
    script: Some("caves.rhai"),
)
```

```rhai
// terrain_config/scripts/caves.rhai

// For underground biomes, get_height isn't used for terrain surface
// Instead, it can return a "density" value for 3D noise carving
fn get_height(x, z, seed) {
    // Return surface biome's height (not used for caves)
    64
}

// Custom block placement creates cave structure
fn get_surface_block(x, y, z, terrain_height, seed) {
    // 2D FBM noise used as cave density field; vertical variation comes from y-dependent threshold
    let cave_noise = perlin_fbm(x * 0.05, z * 0.05, seed, 0.1, 3, 0.5);
    let cave_threshold = 0.4 + (y.to_float() / 256.0) * 0.2;  // Fewer caves deeper
    
    if cave_noise > cave_threshold {
        "Air"  // Carve out cave
    } else {
        // Ore generation based on depth
        let ore_noise = perlin(x, z, seed + 1000, 0.2);
        if y < 16 && ore_noise > 0.9 {
            "Bedrock"  // Placeholder for diamond ore
        } else {
            "Stone"
        }
    }
}
```

```rhai
// terrain_config/scripts/volcanic.rhai
// Called for each (x, z) position to determine terrain height
// Must be deterministic (same inputs = same output)
fn get_height(x, z, seed) {
    // Combine base terrain with sharp volcanic ridges
    let base = perlin(x, z, seed, 0.05);           // Large-scale shape
    let ridges = ridged(x, z, seed + 1, 0.08);     // Sharp peaks
    let detail = perlin(x, z, seed + 2, 0.2);      // Fine detail
    
    // Volcanic peaks: square the ridges for sharper effect
    let volcanic = ridges * ridges;
    
    // Combine: base terrain + volcanic peaks + subtle detail
    let height = 45 + (base * 10) + (volcanic * 30) + (detail * 2);
    
    // Return final height (will be converted to integer)
    height
}

// Optional: Custom surface block selection
// If not defined, uses layers from RON config
fn get_surface_block(x, y, z, terrain_height, seed) {
    if y == terrain_height {
        if terrain_height > 80 {
            "Cobblestone"   // Volcanic peaks (hot rock)
        } else if terrain_height < SEA_LEVEL - 5 {
            "Sand"          // Underwater
        } else {
            "Stone"         // Normal surface
        }
    } else if y > terrain_height - 4 {
        "Stone"
    } else {
        "Stone"
    }
}
```

### 4. Script API Reference

Functions exposed to Rhai scripts:

```rhai
// === Noise Functions ===
// All noise functions return values in range [-1.0, 1.0]
// Note: Terrain height scripts (get_height) are typically called once per (x, z) column.
//       For underground / 3D biomes, get_surface_block may be called per-block (x, y, z)
//       to support operations like cave carving and other volumetric edits.

perlin(x, z, seed, scale)           // Classic Perlin noise
ridged(x, z, seed, scale)           // Ridged multifractal (sharp ridges)
simplex(x, z, seed, scale)          // Simplex noise (smoother than Perlin)

// Multi-octave versions (more detail, combines multiple noise layers)
perlin_fbm(x, z, seed, scale, octaves, persistence)
ridged_fbm(x, z, seed, scale, octaves, persistence)


// === Math Utilities ===

lerp(a, b, t)                       // Linear interpolation
clamp(value, min, max)              // Clamp to range
smoothstep(edge0, edge1, x)         // Smooth interpolation
remap(value, in_min, in_max, out_min, out_max)  // Remap range


// === Constants (read-only) ===

SEA_LEVEL                           // From world settings (e.g., 62)
WORLD_HEIGHT                        // From world settings (e.g., 256)
CHUNK_SIZE                          // Always 16


// === Block IDs (strings) ===
// Return these from get_surface_block():
// Current blocks: "Air", "Dirt", "Grass", "Stone", "Sand", "Snow", "Ice",
//   "Water", "Bedrock", "Cobblestone", "Glass", "Cactus", "OakLog",
//   "OakLeaves", "OakPlanks", "SpruceLog", "SpruceLeaves",
//   "Dandelion", "Poppy", "TallGrass"
// Invalid block names fall back to "Stone" with a warning logged
```

### 5. Script Function Signatures

| Function | Required | Signature | Purpose |
|----------|----------|-----------|--------|
| `get_height` | **Yes** | `fn(x: i64, z: i64, seed: i64) -> f64` | Terrain height at column |
| `get_surface_block` | No | `fn(x, y, z, height, seed) -> String` | Override block placement |

If optional functions are not defined, the system uses the RON config values.

> **Note:** `get_flora()` support is planned for a future release. For now, use RON-based flora rules.

---

## File Structure

### Default Configuration Location

```
data/
├── terrain_config/
│   ├── world.ron                    # Global world settings + biome map
│   ├── biomes/
│   │   ├── plains.ron               # Simple surface biome (data-only)
│   │   ├── forest.ron
│   │   ├── desert.ron
│   │   ├── ocean.ron
│   │   ├── mountains.ron
│   │   ├── caves.ron                # Underground biome
│   │   ├── deep_caves.ron           # Deep underground biome
│   │   ├── volcanic.ron             # Complex biome (references script)
│   │   └── ... (other biomes)
│   └── scripts/
│       ├── volcanic.rhai            # Custom surface generation
│       ├── caves.rhai               # Cave carving logic
│       └── ... (other scripts)
│
└── mods/                            # User modifications (future)
    └── my_terrain_pack/
        ├── biomes/
        │   └── custom_biome.ron
        └── scripts/
            └── custom_biome.rhai
```

### Configuration Loading Order

1. Load `terrain_config/world.ron` for global settings
2. Scan `terrain_config/biomes/*.ron` and register all biomes
3. For biomes with `script: Some("name.rhai")`, load and compile script
4. Validate all biome references in `biome_climate_map` exist
5. (Future) Load mods from `mods/*/` in alphabetical order

---

## Implementation Phases

### Phase 1: Data-Driven Biomes (Foundation)
**Duration:** 2 weeks

1. **Define RON schema types**
   ```rust
   #[derive(Deserialize)]
   struct WorldSettings {
       sea_level: i32,
       biome_scale: f64,
       biome_climate_map: Vec<BiomeClimateEntry>,
       // ...
   }
   
   #[derive(Deserialize)]
   struct BiomeClimateEntry {
       biome: String,
       climate: Climate,
       y_range: Option<(i32, i32)>,  // None = all heights
   }
   
   #[derive(Deserialize)]
   struct TerrainSettings {
       base_height: i32,
       height_variation: i32,
       noise_scale: f64,
   }
   
   #[derive(Deserialize)]
   struct BiomeConfig {
       id: String,
       terrain: TerrainSettings,
       layers: Vec<BlockLayer>,
       flora: Vec<FloraRule>,
       script: Option<String>,
   }
   ```

2. **Create `TerrainConfig` Bevy resource**
   - Load `terrain_config/world.ron` and `terrain_config/biomes/*.ron` at startup
   - Implement 3D biome selection (filter by y_range, then climate)
   - Create `BlockId` string mapping

3. **Modify `generation.rs` to use config**
   - Replace `get_biome_data()` with `BiomeRegistry` lookup
   - Replace hardcoded flora thresholds with config values
   - Extract `SEA_LEVEL` from `world.ron`

**Deliverable:** All existing biomes defined in RON; game behavior unchanged

---

### Phase 2: Rhai Integration
**Duration:** 1.5 weeks

1. **Add Rhai dependency**
   ```toml
   # shared/Cargo.toml
   [dependencies]
   rhai = { version = "1.16", features = ["sync"] }  # sync required for Bevy Resource
   ```

2. **Create script engine with terrain API**
   ```rust
   // shared/src/terrain/scripting.rs
   use rhai::{Engine, Scope, AST};
   
   pub fn create_terrain_engine() -> Engine {
       let mut engine = Engine::new();
       
       // Register noise functions
       engine.register_fn("perlin", noise_perlin);
       engine.register_fn("ridged", noise_ridged);
       engine.register_fn("simplex", noise_simplex);
       engine.register_fn("perlin_fbm", noise_perlin_fbm);
       engine.register_fn("ridged_fbm", noise_ridged_fbm);
       
       // Register math utilities
       engine.register_fn("lerp", math_lerp);
       engine.register_fn("clamp", math_clamp);
       engine.register_fn("smoothstep", math_smoothstep);
       engine.register_fn("remap", math_remap);
       
       // Safety limits
       engine.set_max_operations(100_000);
       engine.set_max_expr_depths(64, 64);
       engine.set_max_call_levels(32);
       engine.set_max_string_size(10_000);
       
       engine
   }
   ```

3. **Compile scripts at load time**
   - Parse `.rhai` files into AST
   - Cache compiled ASTs in `TerrainConfig`
   - Validate required function (`get_height`) exists

4. **Integrate with chunk generation**
   ```rust
   // Height is computed per-column (x, z), then cached for block placement
   fn get_terrain_height(x: i32, z: i32, biome: &BiomeConfig, seed: u32, engine: &Engine) -> i32 {
       if let Some(ast) = &biome.compiled_script {
           // Call script (once per column, not per block)
           let mut scope = Scope::new();  // Fresh scope per call
           let result: f64 = engine.call_fn(
               &mut scope, ast, "get_height",
               (x as i64, z as i64, seed as i64)
           ).unwrap_or_else(|e| {
               warn!("Script error in {}: {}", biome.id, e);
               biome.terrain.base_height as f64  // Fallback
           });
           result.round() as i32
       } else {
           // Data-driven calculation
           data_driven_height(x, z, &biome.terrain, seed)
       }
   }
   
   // In chunk generation, cache heights in a 16x16 array
   fn generate_chunk_heights(chunk_x: i32, chunk_z: i32, ...) -> [[i32; 16]; 16] {
       let mut heights = [[0i32; 16]; 16];
       for lx in 0..16 {
           for lz in 0..16 {
               let world_x = chunk_x * 16 + lx as i32;
               let world_z = chunk_z * 16 + lz as i32;
               heights[lx][lz] = get_terrain_height(world_x, world_z, biome, seed, engine);
           }
       }
       heights
   }
   ```

**Deliverable:** Scripts can override terrain height generation

---

### Phase 3: Complete Script Support
**Duration:** 1.5 weeks

1. **Implement optional script function**
   - `get_surface_block()` - custom block selection per position

2. **Create example scripted biomes**
   - Volcanic islands (sharp ridged peaks)
   - Floating islands (inverted terrain sections)
   - Terraced hills (stepped terrain)

3. **Add script hot-reload for development**
   ```rust
   // Debug builds only
   #[cfg(debug_assertions)]
   fn watch_script_changes(config: &mut TerrainConfig) {
       // Recompile modified .rhai files
   }
   ```

**Deliverable:** Full script API functional with examples

---

### Phase 4: Polish & Documentation
**Duration:** 1 week

1. **Validation and error messages**
   - Validate biome references in `biome_climate_map`
   - Helpful error messages with file/line numbers
   - Warn on unused scripts

2. **Documentation**
   - Modder's guide with tutorials
   - Script API reference
   - Example biomes (simple and complex)

3. **Debug tooling**
   - F-key to show current biome config
   - Console command to reload terrain config

**Deliverable:** Production-ready, documented system

---

### Timeline Summary

| Phase | Duration | Cumulative |
|-------|----------|------------|
| Phase 1: Data-Driven Biomes | 2 weeks | 2 weeks |
| Phase 2: Rhai Integration | 1.5 weeks | 3.5 weeks |
| Phase 3: Complete Script Support | 1.5 weeks | 5 weeks |
| Phase 4: Polish & Documentation | 1 week | **6 weeks** |

---

## Security & Sandboxing

### Rhai Safety Model

Rhai is designed for safe embedding. By default, scripts have **no access** to:
- File system
- Network
- System calls
- Process spawning
- Environment variables

Scripts can only call functions explicitly registered by the host.

### Resource Limits

```rust
let mut engine = Engine::new();

// Prevent infinite loops
engine.set_max_operations(100_000);  // Max bytecode operations per call

// Prevent stack overflow
engine.set_max_call_levels(32);      // Max function call depth
engine.set_max_expr_depths(64, 64);  // Max expression nesting

// Prevent memory exhaustion
engine.set_max_string_size(10_000);  // Max string length
engine.set_max_array_size(1_000);    // Max array elements
engine.set_max_map_size(500);        // Max map entries
```

### API Sandboxing

Only expose safe, deterministic, pure functions:

```rust
// ✅ SAFE: Pure functions, no side effects
engine.register_fn("perlin", noise_perlin);      // Noise generation
engine.register_fn("lerp", math_lerp);           // Math utilities
engine.register_fn("clamp", math_clamp);

// ❌ NEVER EXPOSE:
// engine.register_fn("read_file", ...);          // File access
// engine.register_fn("http_get", ...);           // Network
// engine.register_fn("exec", ...);               // Process spawning
// engine.register_fn("random", ...);             // Non-deterministic!
```

### Determinism Requirement

Terrain generation **must** be deterministic (same seed = same world):

```rhai
// ❌ BAD: Non-deterministic (different every call)
fn get_height(x, z, seed) {
    random() * 100  // WRONG - world changes on reload!
}

// ✅ GOOD: Seed-based (reproducible)
fn get_height(x, z, seed) {
    perlin(x, z, seed, 0.1) * 100  // Same seed = same result
}
```

The exposed `perlin()`, `ridged()`, etc. functions use the seed parameter for determinism.

### Error Handling

Script errors should never crash the game:

```rust
match engine.call_fn(&mut scope, &ast, "get_height", args) {
    Ok(height) => height,
    Err(e) => {
        // Log error with context
        error!("Script error in biome '{}': {}", biome.id, e);
        // Use data-driven fallback
        biome.terrain.base_height as f64
    }
}
```

### Threat Model

| Threat | Mitigation |
|--------|------------|
| Infinite loops | `set_max_operations(100_000)` |
| Stack overflow | `set_max_call_levels(32)` |
| Memory exhaustion | Size limits on strings/arrays/maps |
| File system access | Not exposed in API |
| Network access | Not exposed in API |
| Non-deterministic worlds | No `random()` function; use seeded noise |

---

## Implementation Gotchas

### 1. 3D Biome Selection Algorithm

With height-based biomes, selection becomes more complex:

```rust
fn select_biome(x: i32, y: i32, z: i32, climate: &Climate, map: &[BiomeClimateEntry]) -> &str {
    // 1. Filter entries whose y_range contains this y
    let candidates: Vec<_> = map.iter()
        .filter(|e| match e.y_range {
            None => true,  // No range = matches all heights
            Some((min, max)) => y >= min && y < max,
        })
        .collect();
    
    // Fallback if no biomes match this y coordinate
    if candidates.is_empty() {
        return "plains";
    }
    
    // 2. Among candidates, prefer more specific y_range (narrower range wins)
    // 3. Among equal specificity, find closest climate match
    candidates.iter()
        .min_by(|a, b| {
            // Specificity: narrower y_range = more specific
            let spec_a = a.y_range.map(|(min, max)| max - min).unwrap_or(i32::MAX);
            let spec_b = b.y_range.map(|(min, max)| max - min).unwrap_or(i32::MAX);
            
            // Compare specificity first, then climate distance
            spec_b.cmp(&spec_a)
                .then_with(|| climate_distance(climate, &a.climate)
                    .partial_cmp(&climate_distance(climate, &b.climate))
                    .unwrap())
        })
        .map(|e| e.biome.as_str())
        .unwrap_or("plains")  // Should never reach here with non-empty candidates
}
```

### 2. Surface vs Underground Biome Interaction

For a column at (x, z):
1. **Surface biome** (no y_range or y_range covering surface) determines terrain height
2. **Underground biomes** only affect block placement below the surface
3. Scripts for underground biomes should focus on `get_surface_block()`, not `get_height()`

### 3. Biome Boundary Interpolation

The current `generation.rs` uses `interpolated_height()` to blend terrain at biome boundaries. **Scripted biomes present a challenge**: if biome A uses a script and biome B is data-driven, how do we blend?

**MVP approach**: For scripted biomes, skip interpolation and use the primary biome's height directly. This may cause sharper biome transitions but avoids complexity.

**Future improvement**: Sample heights from both biomes (calling scripts for both) and blend. This requires careful performance tuning.

### 4. BlockId Validation at Load Time

To avoid spamming warnings during generation ("invalid block 'Obsdian' in biome X" thousands of times), validate all block name strings when loading RON configs:

```rust
fn validate_biome_config(config: &BiomeConfig) -> Result<(), ConfigError> {
    for layer in &config.layers {
        if BlockId::from_str(&layer.block).is_err() {
            return Err(ConfigError::InvalidBlock {
                biome: config.id.clone(),
                block: layer.block.clone(),
            });
        }
    }
    Ok(())
}
```

### 5. Test Fixture Location

Integration tests reference `test_fixtures/`. These should live in:
```
shared/tests/fixtures/terrain_config/
├── world.ron
├── biomes/
│   └── test_biome.ron
└── scripts/
    └── test_script.rhai
```

### 6. Rhai Scope Lifetime

Rhai's `Scope` cannot be stored in the `TerrainConfig` resource—it must be created fresh for each script call. The `Engine` and compiled `AST` can be stored and reused.

### 7. Data-Driven Height Formula

The data-driven path should use:
```rust
fn data_driven_height(x: i32, z: i32, terrain: &TerrainSettings, seed: u32) -> i32 {
    let noise = perlin_noise(x as f64 * terrain.noise_scale, 
                              z as f64 * terrain.noise_scale, 
                              seed);
    terrain.base_height + ((noise * terrain.height_variation as f64).round() as i32)
}
```

This keeps the formula simple and predictable for modders.

---

## Migration Path

### Backward Compatibility

1. **Default configs mirror current behavior**
   - Extract all hardcoded values to default RON files
   - Game behaves identically with default configs

2. **Existing worlds continue to work**
   - World seed determines terrain (unchanged)
   - Newly generated chunks use the current terrain config; previously generated chunks remain as-is
   - Note: if an existing chunk is deleted and regenerated after a config change, its terrain may differ from neighboring older chunks even with the same seed
   - Mitigation: track a world/terrain-generator or chunk-format version in world metadata so engines can (a) continue to use the original settings for regeneration or (b) explicitly migrate worlds while warning about visible chunk boundaries

3. **Gradual adoption**
   - Existing code paths remain until deprecated
   - Config system is additive, not replacement

### Migration Steps

1. **v0.2.0**: Introduce config system alongside hardcoded values
2. **v0.3.0**: Deprecate direct modification of `generation.rs`
3. **v1.0.0**: Remove hardcoded terrain values; config-only

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_biome_config_parsing() {
    let ron = r#"(
        id: "test",
        display_name: "Test Biome",
        terrain: (base_height: 64, height_variation: 4, noise_scale: 0.01),
        layers: [(depth: 0, block: "Grass")],
        flora: [],
        script: None,
    )"#;
    let config: BiomeConfig = ron::from_str(ron).unwrap();
    assert_eq!(config.id, "test");
    assert_eq!(config.terrain.base_height, 64);
}

#[test]
fn test_script_compilation() {
    let engine = create_terrain_engine();
    let script = r#"
        fn get_height(x, z, seed) {
            perlin(x, z, seed, 0.1) * 10 + 64
        }
    "#;
    let ast = engine.compile(script).expect("Script should compile");
    let mut scope = Scope::new();
    let result: Result<f64, _> = engine.call_fn(&mut scope, &ast, "get_height", (0_i64, 0_i64, 0_i64));
    assert!(result.is_ok(), "get_height function should exist and be callable");
}

#[test]
fn test_script_execution() {
    let engine = create_terrain_engine();
    let ast = engine.compile(r#"
        fn get_height(x, z, seed) { 64 }
    "#).unwrap();
    
    let mut scope = Scope::new();
    let result: f64 = engine.call_fn(&mut scope, &ast, "get_height", (0i64, 0i64, 12345i64)).unwrap();
    assert_eq!(result, 64.0);
}

#[test]
fn test_noise_determinism() {
    let engine = create_terrain_engine();
    let ast = engine.compile(r#"
        fn get_height(x, z, seed) {
            perlin(x, z, seed, 0.1) * 100
        }
    "#).unwrap();
    
    let mut scope = Scope::new();
    let result1: f64 = engine.call_fn(&mut scope, &ast, "get_height", (100i64, 200i64, 42i64)).unwrap();
    let result2: f64 = engine.call_fn(&mut scope, &ast, "get_height", (100i64, 200i64, 42i64)).unwrap();
    
    assert_eq!(result1, result2, "Noise must be deterministic");
}
```

### Integration Tests

```rust
#[test]
fn test_generation_with_scripted_biome() {
    let config = TerrainConfig::load("tests/fixtures/terrain_config/").unwrap();
    let chunk = generate_chunk_with_config(IVec3::new(0, 4, 0), 12345, &config);
    
    // Verify chunk was generated
    assert!(!chunk.map.is_empty());
}

#[test]
fn test_default_config_matches_current_behavior() {
    // Generate chunk with new config system
    let config = TerrainConfig::default();
    let chunk_pos = IVec3::new(0, 4, 0);
    let seed = 12345;
    let new_chunk = generate_chunk_with_config(chunk_pos, seed, &config);
    
    // Generate chunk with old hardcoded system (current generation.rs)
    let old_chunk = generate_chunk(chunk_pos, seed);
    
    // The overall terrain shape/contents should match at representative positions,
    // but internal map representations may differ.
    let sample_positions = [
        IVec3::new(0, 0, 0),
        IVec3::new(1, 0, 0),
        IVec3::new(0, 0, 1),
        IVec3::new(8, 0, 8),
        IVec3::new(15, 0, 15),
    ];

    for pos in &sample_positions {
        let new_block = new_chunk.map.get(pos);
        let old_block = old_chunk.map.get(pos);
        assert_eq!(
            new_block, old_block,
            "terrain mismatch at sampled position {:?}",
            pos
        );
    }
}

#[test]
fn test_script_error_falls_back_to_data() {
    // Load config with intentionally broken script for testing
    let config = TerrainConfig::load("tests/fixtures/broken_script/").unwrap();
    
    // Should not panic; should log error and use data-driven fallback
    let chunk = generate_chunk_with_config(IVec3::new(0, 4, 0), 12345, &config);
    assert!(!chunk.map.is_empty());
}
```

### Script Safety Tests

```rust
#[test]
fn test_infinite_loop_protection() {
    let engine = create_terrain_engine();
    let ast = engine.compile(r#"
        fn get_height(x, z, seed) {
            loop { }  // Infinite loop
            64
        }
    "#).unwrap();
    
    let mut scope = Scope::new();
    let result = engine.call_fn::<f64>(&mut scope, &ast, "get_height", (0i64, 0i64, 0i64));
    
    // Should error, not hang
    assert!(result.is_err());
}

#[test]
fn test_no_file_access() {
    let engine = create_terrain_engine();
    
    // Should fail to compile - no file functions available
    let result = engine.compile(r#"
        fn get_height(x, z, seed) {
            read_file("secrets.txt");  // Should not exist
            64
        }
    "#);
    
    assert!(result.is_err());
}
```

---

## Future Extensions

### Planned Extensions (Post-MVP)

#### 1. Script Flora Placement (`get_flora`)
Allow scripts to override flora placement:
```rhai
fn get_flora(x, z, height, seed) {
    if perlin(x, z, seed + 50, 0.05) > 0.9 {
        "Cactus"
    } else {
        "None"
    }
}
```

#### 2. Worley Noise
Add cellular/Voronoi noise for terrain features like rock formations:
```rhai
worley(x, z, seed, scale)  // Returns distance to nearest cell point
```

#### 3. Mod System
Full mod loading with manifests, dependencies, and load ordering:
```
data/mods/my_pack/
├── manifest.ron
├── biomes/
└── scripts/
```

#### 4. Custom Structures
Define multi-block structures placeable by scripts:
```ron
// terrain_config/structures/giant_mushroom.ron
(
    id: "giant_mushroom",
    blocks: [
        (offset: (0, 0, 0), block: "MushroomStem"),
        (offset: (0, 1, 0), block: "MushroomStem"),
        (offset: (0, 2, 0), block: "MushroomCap"),
        // ...
    ],
)
```

```rhai
fn get_flora(x, z, height, seed) {
    if perlin(x, z, seed + 50, 0.05) > 0.8 {
        "structure:giant_mushroom"  // Place custom structure
    } else {
        "None"
    }
}
```

#### 5. Cave Generation
Building on the height-based biome system, expose dedicated cave generation:
```rhai
fn is_cave(x, y, z, seed) {
    let cave_noise = perlin_3d(x, y, z, seed + 100, 0.05);
    cave_noise > 0.6  // Hollow out if true
}
```

> **Note:** Basic cave generation via `get_surface_block()` in underground biomes is already supported. This extension adds a dedicated `is_cave()` hook for cleaner separation.

#### 6. Ore Distribution
Per-biome ore configuration:
```ron
ores: [
    (block: "DiamondOre", min_y: 5, max_y: 16, chance: 0.001, vein_size: (1, 4)),
    (block: "IronOre", min_y: 0, max_y: 64, chance: 0.01, vein_size: (2, 8)),
]
```

#### 7. Visual Script Editor
Node-based editor for non-programmers (generates Rhai code).

### Script API Extensions

Future functions to expose:
```rhai
// Cellular noise
worley(x, z, seed, scale)

// 3D noise for caves/ores
perlin_3d(x, y, z, seed, scale)

// Structure placement
place_structure(x, y, z, structure_id)

// Biome queries
get_neighbor_biome(x, z, direction)
get_distance_to_biome(x, z, biome_id)

// Advanced terrain
get_erosion_factor(x, z, seed)
get_continental_factor(x, z, seed)
```

---

## Appendix: Quick Reference

### File Locations
| Purpose | Path |
|---------|------|
| Global settings | `data/terrain_config/world.ron` |
| Biome definitions | `data/terrain_config/biomes/*.ron` |
| Custom scripts | `data/terrain_config/scripts/*.rhai` |
| User mods (future) | `data/mods/<mod_name>/` |

### Key Types
| Type | Purpose |
|------|---------|
| `WorldSettings` | Global terrain parameters |
| `BiomeConfig` | Single biome definition |
| `TerrainConfig` | Combined runtime configuration |
| `Engine` | Rhai script engine |
| `AST` | Compiled script |

### Script Functions
| Function | Required | Purpose |
|----------|----------|---------|  
| `get_height(x, z, seed)` | **Yes** | Return terrain height |
| `get_surface_block(x, y, z, height, seed)` | No | Override block placement |

### Exposed Script API
```rhai
// Noise
perlin(x, z, seed, scale) -> f64
ridged(x, z, seed, scale) -> f64
simplex(x, z, seed, scale) -> f64
perlin_fbm(x, z, seed, scale, octaves, persistence) -> f64
ridged_fbm(x, z, seed, scale, octaves, persistence) -> f64

// Math
lerp(a, b, t) -> f64
clamp(value, min, max) -> f64
smoothstep(edge0, edge1, x) -> f64
remap(value, in_min, in_max, out_min, out_max) -> f64

// Constants
SEA_LEVEL -> i64
WORLD_HEIGHT -> i64
CHUNK_SIZE -> i64
```

---

## Summary

This plan provides a **hybrid data + scripting** system for custom terrain generation:

1. **Simple biomes**: Pure RON configuration (no coding required)
2. **Complex biomes**: Rhai scripts for custom terrain algorithms
3. **Safe**: Sandboxed execution with resource limits
4. **Familiar**: JavaScript-like syntax for modders
5. **Performant**: Scripts compiled at load time, not interpreted per-block
6. **Future-proof**: Extensible API for structures, caves, ores

### Implementation Timeline

| Phase | Duration |
|-------|----------|
| Data-driven biomes | 2 weeks |
| Rhai integration | 1.5 weeks |
| Complete script support | 1.5 weeks |
| Polish & documentation | 1 week |
| **Total** | **6 weeks** |

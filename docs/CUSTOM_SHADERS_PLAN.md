# Custom Shaders System Plan

## Overview

This document outlines the plan to add support for user-customizable WGSL shaders in Rustcraft. Users will be able to place shader files in a `/shaders` directory within their app data folder to customize the game's visual appearance.

## Goals

1. **User Accessibility**: Allow non-programmers to easily install shader packs
2. **Moddability**: Enable shader developers to create and share visual customizations
3. **Performance Options**: Support multiple quality presets per shader pack
4. **Safety**: Validate shaders to prevent crashes or security issues
5. **Fallback**: Gracefully revert to default shaders if custom ones fail

---

## Phase 1: Foundation & Directory Structure

### 1.1 Shader Directory Setup

Create the shaders directory structure on first run:

```
<game_folder_path>/
├── shaders/
│   ├── packs/               # User-installed shader packs
│   │   ├── default/         # Built-in default shaders (copied from assets on first run)
│   │   └── <user_packs>/    # User-added shader packs
│   └── shader_settings.ron  # Active pack selection & settings
```

**Implementation Location**: `shared/src/lib.rs` - Extend `GameFolderPaths` struct

```rust
#[derive(Resource, Clone, Debug)]
pub struct GameFolderPaths {
    pub game_folder_path: PathBuf,
    pub assets_folder_path: PathBuf,
    pub shaders_folder_path: PathBuf,  // NEW
}
```

**First-Run Initialization**: Add to client startup in `client/src/main.rs`

```rust
fn ensure_shader_directories(paths: &GameFolderPaths, asset_server: &AssetServer) -> std::io::Result<()> {
    let packs_dir = paths.shaders_folder_path.join("packs");
    fs::create_dir_all(&packs_dir)?;
    
    // Copy default shaders from assets if not present
    let default_pack = packs_dir.join("default");
    if !default_pack.exists() {
        copy_default_shaders(&paths.assets_folder_path, &default_pack)?;
    }
    Ok(())
}
```

### 1.2 Shader Pack Manifest Format

Each shader pack requires a `shader_pack.ron` manifest:

```ron
ShaderPack(
    name: "Enhanced Visuals",
    version: "1.0.0",
    author: "ShaderDev",
    description: "Improved lighting and water effects",
    min_rustcraft_version: "0.5.0",
    
    // Shader definitions
    shaders: {
        "chunk_solid": ShaderDef(
            vertex: "chunk.vert.wgsl",
            fragment: "chunk.frag.wgsl",
            // Optional variants for quality presets
            variants: {
                "low": ShaderVariant(fragment: "chunk_low.frag.wgsl"),
                "high": ShaderVariant(fragment: "chunk_high.frag.wgsl"),
            }
        ),
        "water": ShaderDef(
            vertex: "water.vert.wgsl",
            fragment: "water.frag.wgsl",
        ),
        "sky": ShaderDef(
            vertex: "sky.vert.wgsl",
            fragment: "sky.frag.wgsl",
        ),
        "post_process": ShaderDef(
            vertex: "fullscreen.vert.wgsl",
            fragment: "post_process.frag.wgsl",
        ),
    },
    
    // Configurable uniforms exposed to UI
    settings: [
        Setting(name: "bloom_intensity", ty: Float, default: 0.5, min: 0.0, max: 2.0),
        Setting(name: "shadow_quality", ty: Int, default: 2, min: 0, max: 3),
        Setting(name: "water_reflections", ty: Bool, default: true),
    ],
)
```

---

## Phase 2: Shader Loading System

### 2.1 New Module Structure

Create a new shader management module:

```
client/src/
├── shaders/
│   ├── mod.rs              # Module exports, resources, plugin setup
│   ├── loader.rs           # Shader pack discovery, validation & loading
│   └── materials.rs        # Custom materials & render pipeline integration
```

This minimal structure can be expanded later as complexity grows.

### 2.2 Shader Pack Resource

```rust
// client/src/shaders/mod.rs

#[derive(Resource, Default)]
pub struct ActiveShaderPack {
    pub name: String,
    pub path: PathBuf,
    pub manifest: ShaderPackManifest,
    pub handles: HashMap<String, Handle<Shader>>,
    pub materials: HashMap<String, Handle<ShaderMaterial>>,
    pub settings: ShaderSettings,
}

#[derive(Resource, Default)]
pub struct AvailableShaderPacks {
    pub packs: Vec<ShaderPackInfo>,
}

pub struct ShaderPackInfo {
    pub name: String,
    pub path: PathBuf,
    pub manifest: ShaderPackManifest,
    pub is_valid: bool,
    pub validation_errors: Vec<String>,
}
```

### 2.3 Shader Loader System

```rust
// client/src/shaders/loader.rs

pub fn scan_shader_packs(
    paths: Res<GameFolderPaths>,
    mut available_packs: ResMut<AvailableShaderPacks>,
) {
    let packs_dir = paths.shaders_folder_path.join("packs");
    
    for entry in fs::read_dir(packs_dir).ok().into_iter().flatten() {
        if let Ok(entry) = entry {
            if entry.path().is_dir() {
                if let Some(pack_info) = validate_shader_pack(&entry.path()) {
                    available_packs.packs.push(pack_info);
                }
            }
        }
    }
}

fn validate_shader_pack(path: &Path) -> Option<ShaderPackInfo> {
    let manifest_path = path.join("shader_pack.ron");
    let manifest: ShaderPackManifest = ron::de::from_reader(
        File::open(&manifest_path).ok()?
    ).ok()?;
    
    let mut errors = Vec::new();
    
    // Validate all referenced shader files exist
    for (name, shader_def) in &manifest.shaders {
        if !path.join(&shader_def.vertex).exists() {
            errors.push(format!("Missing vertex shader: {}", shader_def.vertex));
        }
        if !path.join(&shader_def.fragment).exists() {
            errors.push(format!("Missing fragment shader: {}", shader_def.fragment));
        }
    }
    
    Some(ShaderPackInfo {
        name: manifest.name.clone(),
        path: path.to_path_buf(),
        manifest,
        is_valid: errors.is_empty(),
        validation_errors: errors,
    })
}
```

### 2.4 Bevy Integration

Bevy uses WGSL (WebGPU Shading Language) natively, which provides:

- Native Bevy support with no translation layer needed
- Better error messages and tooling
- Modern shader syntax designed for WebGPU
- Cross-platform compatibility

```rust
// client/src/shaders/pipeline.rs

use bevy::render::render_resource::{AsBindGroup, ShaderRef};

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct CustomChunkMaterial {
    #[uniform(0)]
    pub time: f32,
    #[uniform(0)]
    pub fog_color: Vec4,
    #[uniform(0)]
    pub fog_density: f32,
    
    #[texture(1)]
    #[sampler(2)]
    pub texture_atlas: Handle<Image>,
    
    // Custom settings from shader pack
    #[uniform(3)]
    pub custom_settings: CustomShaderSettings,
}

impl Material for CustomChunkMaterial {
    fn vertex_shader() -> ShaderRef {
        // Shader path set dynamically via ActiveShaderPack resource
        // Default: "shaders/packs/default/chunk.vert.wgsl"
        ShaderRef::Default  // Overridden at runtime
    }
    
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Default  // Overridden at runtime
    }
}

// Note: Bevy's Material trait uses static methods, so dynamic shader
// selection requires creating separate material types per pack, or
// using Bevy's lower-level RenderPipelineDescriptor API.
```

---

## Phase 3: Shader Pipeline Categories

### 3.1 Shader Categories to Support

| Category | Purpose | Current Implementation | Custom Shader Potential |
|----------|---------|----------------------|------------------------|
| **Chunk Solid** | Opaque block rendering | `StandardMaterial` | Custom lighting, ambient occlusion |
| **Chunk Transparent** | Glass, water, leaves | `StandardMaterial` with alpha | Refraction, wave animation |
| **Sky** | Atmosphere rendering | `bevy_atmosphere` | Custom sky colors, clouds |
| **Post-Process** | Screen effects | None | Bloom, DOF, color grading |
| **Entity** | Players, mobs | `StandardMaterial` | Outline effects, cel shading |
| **UI** | HUD elements | Bevy UI | Animated UI backgrounds |

### 3.2 Implementation Priority

1. **Phase 3a**: Chunk shaders (blocks) - Highest visual impact
2. **Phase 3b**: Post-processing effects - Easy wins (bloom, vignette)
3. **Phase 3c**: Sky/atmosphere customization
4. **Phase 3d**: Entity shaders
5. **Phase 3e**: UI shaders (lowest priority)

---

## Phase 4: Default Shader Pack

### 4.1 Bundled Default Shaders

Create a default shader pack that ships with the game:

```
data/shaders/default/
├── shader_pack.ron
├── chunk.vert.wgsl
├── chunk.frag.wgsl
├── water.vert.wgsl
├── water.frag.wgsl
├── sky.vert.wgsl
├── sky.frag.wgsl
└── post/
    ├── fullscreen.vert.wgsl
    └── tonemap.frag.wgsl
```

### 4.2 Default Chunk Shader (Example)

```wgsl
// chunk.frag.wgsl - Default chunk fragment shader

struct ChunkUniforms {
    time: f32,
    fog_color: vec4<f32>,
    fog_near: f32,
    fog_far: f32,
    ambient_strength: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: ChunkUniforms;

@group(1) @binding(0)
var texture_atlas: texture_2d<f32>;
@group(1) @binding(1)
var texture_sampler: sampler;

struct FragmentInput {
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) ao: f32,
}

@fragment
fn fragment(in: FragmentInput) -> @location(0) vec4<f32> {
    // Sample texture
    let base_color = textureSample(texture_atlas, texture_sampler, in.uv);
    
    // Basic lighting
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let diffuse = max(dot(in.world_normal, light_dir), 0.0);
    let ambient = uniforms.ambient_strength;
    let lighting = ambient + diffuse * (1.0 - ambient);
    
    // Ambient occlusion
    let ao = in.ao;
    
    // Apply lighting
    var color = base_color.rgb * lighting * ao;
    
    // Distance fog
    let view_distance = length(in.world_position);
    let fog_factor = clamp(
        (view_distance - uniforms.fog_near) / (uniforms.fog_far - uniforms.fog_near),
        0.0, 1.0
    );
    color = mix(color, uniforms.fog_color.rgb, fog_factor);
    
    return vec4<f32>(color, base_color.a);
}
```

---

## Phase 5: UI Integration

### 5.1 Shader Settings Menu

Add a new settings category for shader configuration:

```
client/src/ui/menus/
├── settings/
│   ├── mod.rs
│   ├── video.rs
│   ├── audio.rs
│   ├── controls.rs
│   └── shaders.rs      # NEW - Shader settings UI
```

### 5.2 Shader Menu Features

1. **Shader Pack Selection**: Dropdown to choose from available packs
2. **Quality Preset**: Low/Medium/High/Ultra presets
3. **Custom Settings**: Sliders/toggles for shader-specific options
4. **Preview**: Real-time preview of shader changes
5. **Reset to Default**: One-click revert to default shaders
6. **Reload Shaders**: Hot-reload for shader developers (F10 key)

### 5.3 Settings Persistence

Store shader settings in `<game_folder_path>/shader_settings.ron`:

```ron
ShaderSettings(
    active_pack: "Enhanced Visuals",
    quality_preset: "high",
    custom_values: {
        "bloom_intensity": 0.7,
        "shadow_quality": 3,
        "water_reflections": true,
    },
)
```

---

## Phase 6: Error Handling & Fallbacks

### 6.1 Hot Reloading (Developer Feature)

Enable shader developers to iterate quickly:

```rust
// client/src/shaders/loader.rs

#[derive(Resource)]
pub struct ShaderWatcher {
    watcher: RecommendedWatcher,
    receiver: Receiver<notify::Result<Event>>,
}

pub fn setup_shader_hot_reload(
    mut commands: Commands,
    paths: Res<GameFolderPaths>,
    active_pack: Res<ActiveShaderPack>,
) {
    let (tx, rx) = std::sync::mpsc::channel();
    
    let mut watcher = recommended_watcher(move |res| {
        tx.send(res).ok();
    }).expect("Failed to create file watcher");
    
    // Watch the active shader pack directory
    watcher.watch(
        &active_pack.path,
        RecursiveMode::Recursive
    ).ok();
    
    commands.insert_resource(ShaderWatcher {
        watcher,
        receiver: rx,
    });
}

pub fn check_shader_changes(
    watcher: Res<ShaderWatcher>,
    mut reload_events: EventWriter<ShaderReloadEvent>,
) {
    while let Ok(Ok(event)) = watcher.receiver.try_recv() {
        if matches!(event.kind, EventKind::Modify(_)) {
            reload_events.send(ShaderReloadEvent);
        }
    }
}
```

Add F10 keybind for manual shader reload:

```rust
// In client/src/input/keyboard.rs
GameAction::ReloadShaders => vec![KeyCode::F10],
```

### 6.2 Shader Compilation Errors

```rust
pub fn handle_shader_error(
    error: &ShaderError,
    shader_path: &Path,
) -> ShaderErrorRecovery {
    error!("Shader compilation failed: {}", shader_path.display());
    error!("Error: {}", error);
    
    // Log to file for user debugging
    let log_path = shader_path.with_extension("error.log");
    fs::write(&log_path, format!("{:#?}", error)).ok();
    
    // Show in-game notification
    ShaderErrorRecovery::FallbackToDefault {
        notification: format!(
            "Shader '{}' failed to compile. See {} for details.",
            shader_path.file_name().unwrap_or_default().to_string_lossy(),
            log_path.display()
        ),
    }
}
```

### 6.3 Graceful Degradation

1. **Invalid Shader**: Fall back to default shader for that category
2. **Missing Shader**: Use built-in Bevy `StandardMaterial`
3. **Pack Load Failure**: Revert to last working pack or default
4. **Performance Issues**: Auto-reduce quality preset

---

## Implementation Timeline

| Phase | Description | Estimated Effort | Dependencies |
|-------|-------------|-----------------|--------------|
| 1 | Directory structure & manifest | 2-3 days | None |
| 2 | Shader loading system (includes hot reload) | 4-5 days | Phase 1 |
| 3a | Chunk shaders | 4-5 days | Phase 2 |
| 3b | Post-processing | 3-4 days | Phase 2 |
| 4 | Default shader pack | 2-3 days | Phase 3a |
| 5 | UI integration | 3-4 days | Phase 2, 4 |
| 6 | Error handling & polish | 2-3 days | All phases |

**Total Estimated Time**: 2.5-3 weeks for full implementation

---

## Technical Considerations

### Performance Impact

- **Shader Compilation**: Cache compiled shaders to avoid startup delays
- **Uniform Updates**: Batch uniform updates, avoid per-frame allocations
- **Draw Calls**: Custom materials may increase draw call count if not batched properly
- **GPU Memory**: Monitor VRAM usage with complex shader packs

### Compatibility

- **GPU Requirements**: Document minimum GPU requirements per shader pack
- **Bevy Version**: Shaders must be updated when Bevy's render pipeline changes
- **Platform Differences**: Test on Windows, macOS, and Linux

### Security

- **No Code Execution**: WGSL cannot execute arbitrary code (GPU-only)
- **Resource Limits**: Implement timeouts for shader compilation
- **Validation**: Validate all shader inputs to prevent malformed data

---

## Future Enhancements

1. **Shader Graph Editor**: Visual node-based shader editing
2. **Compute Shaders**: GPU-based chunk meshing, particle systems
3. **Ray Tracing**: Optional RTX/DXR path tracing mode
4. **Shader Workshop**: In-game browser for downloading shader packs

---

## References

- [Bevy Shader Documentation](https://bevyengine.org/learn/book/shaders/)
- [WGSL Specification](https://www.w3.org/TR/WGSL/)
- [Minecraft Shader Packs (OptiFine)](https://optifine.net/shaders) - Inspiration for features
- [bevy_atmosphere](https://github.com/JonahPlusPlus/bevy_atmosphere) - Sky rendering reference

---

## Appendix A: Shader Pack Development Guide

For shader developers creating custom packs:

### Getting Started

1. Copy `<game_folder>/shaders/packs/default/` to a new folder
2. Rename and edit `shader_pack.ron` with your pack info
3. Modify shader files (`.wgsl`)
4. Select your pack in Settings → Graphics → Shader Pack
5. Press F10 to hot-reload changes during development

### Available Uniforms

| Uniform | Type | Description |
|---------|------|-------------|
| `time` | `f32` | Game time in seconds |
| `world_time` | `f32` | In-game day/night cycle (0.0-1.0) |
| `camera_position` | `vec3<f32>` | Camera world position |
| `view_matrix` | `mat4x4<f32>` | View transformation |
| `projection_matrix` | `mat4x4<f32>` | Projection transformation |
| `fog_color` | `vec4<f32>` | Current fog color |
| `fog_near` | `f32` | Fog start distance |
| `fog_far` | `f32` | Fog end distance |
| `sun_direction` | `vec3<f32>` | Direction to sun |
| `ambient_color` | `vec3<f32>` | Ambient light color |

### Best Practices

1. **Test on Multiple GPUs**: AMD, NVIDIA, Intel, and Apple Silicon
2. **Provide Quality Variants**: Not all players have powerful GPUs
3. **Document Settings**: Explain what each setting does
4. **Include Screenshots**: Help users preview your pack
5. **Version Control**: Use semantic versioning for your packs

# Rendering and UI System Documentation

## Overview

The rendering system transforms voxel world data into visual output using Bevy's rendering pipeline. It handles mesh generation, textures, materials, lighting, and all user interface elements.

## Architecture

```
Rendering System
├── Voxel Rendering
│   ├── Greedy Meshing
│   ├── Texture Atlas
│   ├── Materials
│   └── Culling
│
├── Lighting
│   ├── Directional Light (Sun)
│   ├── Ambient Light
│   └── Day/Night Cycle
│
├── Camera
│   ├── First-Person View
│   ├── Third-Person View
│   └── Controller
│
└── UI
    ├── HUD (In-Game)
    │   ├── Hotbar
    │   ├── Inventory
    │   ├── Chat
    │   ├── Reticle
    │   └── Debug Info
    │
    └── Menus
        ├── Main Menu
        ├── Pause Menu
        ├── Settings
        └── Multiplayer
```

## Voxel Rendering

### Greedy Meshing Algorithm

**Location**: `client/src/world/rendering/meshing.rs`

Greedy meshing combines adjacent identical block faces to reduce polygon count:

```rust
pub fn generate_chunk_mesh(chunk: &ClientChunk) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    
    // Process each direction separately
    for direction in &[
        Direction::Up, Direction::Down,
        Direction::North, Direction::South,
        Direction::East, Direction::West
    ] {
        greedy_mesh_direction(
            chunk,
            direction,
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut indices,
        );
    }
    
    create_mesh(positions, normals, uvs, indices)
}

fn greedy_mesh_direction(
    chunk: &ClientChunk,
    direction: &Direction,
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
) {
    // Create 2D grid for this direction
    let (u_axis, v_axis) = direction.tangent_axes();
    let mut mask = [[None; 16]; 16];
    
    // Scan through chunk in this direction
    for slice in 0..16 {
        // Fill mask for this slice
        for u in 0..16 {
            for v in 0..16 {
                let pos = slice_to_world_pos(slice, u, v, direction);
                mask[u][v] = get_face_at(chunk, pos, direction);
            }
        }
        
        // Greedily merge adjacent faces
        for u in 0..16 {
            for v in 0..16 {
                if let Some(face) = mask[u][v] {
                    // Find maximum rectangle
                    let (width, height) = find_max_rectangle(&mask, u, v, &face);
                    
                    // Create quad for this rectangle
                    add_quad(
                        positions, normals, uvs, indices,
                        slice, u, v, width, height,
                        direction, &face
                    );
                    
                    // Clear merged area
                    clear_mask(&mut mask, u, v, width, height);
                }
            }
        }
    }
}

fn find_max_rectangle(
    mask: &[[Option<FaceData>; 16]; 16],
    start_u: usize,
    start_v: usize,
    face: &FaceData,
) -> (usize, usize) {
    // Find maximum width
    let mut width = 1;
    while start_u + width < 16 && mask[start_u + width][start_v] == Some(*face) {
        width += 1;
    }
    
    // Find maximum height with this width
    let mut height = 1;
    'outer: while start_v + height < 16 {
        for u in start_u..start_u + width {
            if mask[u][start_v + height] != Some(*face) {
                break 'outer;
            }
        }
        height += 1;
    }
    
    (width, height)
}
```

**Performance Benefits**:
- Reduces polygon count by 50-90%
- Fewer draw calls
- Better GPU utilization
- Smoother frame rates

### Face Culling

Only render faces that are visible:

```rust
fn should_render_face(
    chunk: &ClientChunk,
    block_pos: IVec3,
    direction: Direction,
) -> bool {
    let neighbor_pos = block_pos + direction.offset();
    
    // Check neighbor block
    if let Some(neighbor) = get_block(chunk, neighbor_pos) {
        // Don't render if neighbor is opaque
        if neighbor.is_solid() && !neighbor.is_transparent() {
            return false;
        }
    }
    
    // Render if neighbor is air or transparent
    true
}
```

**Optimizations**:
- Interior faces never rendered
- Transparent blocks handled specially
- Neighboring chunk awareness

### Texture Atlas

**Location**: `client/src/world/rendering/materials.rs`

All block textures in a single atlas:

```rust
pub const ATLAS_SIZE: u32 = 16;  // 16x16 grid
pub const TEXTURE_SIZE: f32 = 1.0 / ATLAS_SIZE as f32;

pub fn get_texture_coords(block_id: BlockId, face: BlockFace) -> [Vec2; 4] {
    let (atlas_x, atlas_y) = get_atlas_position(block_id, face);
    
    let u = atlas_x as f32 * TEXTURE_SIZE;
    let v = atlas_y as f32 * TEXTURE_SIZE;
    
    // Return UV coordinates for quad corners
    [
        Vec2::new(u, v),
        Vec2::new(u + TEXTURE_SIZE, v),
        Vec2::new(u + TEXTURE_SIZE, v + TEXTURE_SIZE),
        Vec2::new(u, v + TEXTURE_SIZE),
    ]
}

fn get_atlas_position(block_id: BlockId, face: BlockFace) -> (u32, u32) {
    match (block_id, face) {
        (BlockId::Grass, BlockFace::Top) => (0, 0),
        (BlockId::Grass, BlockFace::Side) => (1, 0),
        (BlockId::Grass, BlockFace::Bottom) => (2, 0),
        (BlockId::Dirt, _) => (2, 0),
        (BlockId::Stone, _) => (3, 0),
        (BlockId::OakLog, BlockFace::Top | BlockFace::Bottom) => (4, 0),
        (BlockId::OakLog, BlockFace::Side) => (5, 0),
        // ... more mappings
        _ => (15, 15),  // Missing texture placeholder
    }
}
```

**Benefits**:
- Single texture bind for all blocks
- Reduces state changes
- Better GPU cache utilization
- Simpler material management

### Materials

```rust
pub fn create_block_material(
    textures: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    // Load texture atlas
    let texture_handle = load_texture_atlas(textures);
    
    materials.add(StandardMaterial {
        base_color_texture: Some(texture_handle),
        perceptual_roughness: 1.0,  // Not shiny
        reflectance: 0.0,  // No reflection
        ..default()
    })
}
```

## Lighting System

### Directional Light (Sun)

**Location**: `client/src/world/celestial.rs`

```rust
#[derive(Component)]
pub struct Sun;

pub fn setup_main_lighting(mut commands: Commands) {
    // Directional light (sun)
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -std::f32::consts::FRAC_PI_4,  // 45 degrees down
            std::f32::consts::FRAC_PI_4,   // 45 degrees rotation
            0.0,
        )),
        Sun,
    ));
    
    // Ambient light
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 0.3,  // Subtle ambient
    });
}
```

### Day/Night Cycle

```rust
pub fn update_sun_position_system(
    time: Res<ClientTime>,
    mut sun_query: Query<&mut Transform, With<Sun>>,
    mut ambient: ResMut<AmbientLight>,
) {
    let time_of_day = time.time_of_day();  // 0.0 to 1.0
    
    for mut transform in sun_query.iter_mut() {
        // Rotate sun around the world
        let angle = time_of_day * 2.0 * PI;
        transform.rotation = Quat::from_rotation_x(angle);
    }
    
    // Adjust ambient light
    let brightness = if time.is_day() {
        0.8  // Bright during day
    } else {
        0.1  // Dark at night
    };
    ambient.brightness = brightness;
}
```

### Atmospheric Rendering

Uses `bevy_atmosphere` for sky:

```rust
pub fn setup_atmosphere(mut commands: Commands) {
    commands.insert_resource(AtmosphereModel::default());
}

pub fn update_atmosphere_system(
    time: Res<ClientTime>,
    mut atmosphere: ResMut<AtmosphereModel>,
) {
    // Update sun position in atmosphere
    let time_of_day = time.time_of_day();
    atmosphere.sun_position = calculate_sun_vector(time_of_day);
}
```

## Camera System

### Camera Controller

**Location**: `client/src/camera/controller.rs`

```rust
#[derive(Component)]
pub struct GameCamera {
    pub sensitivity: f32,
    pub pitch: f32,
    pub yaw: f32,
}

pub fn camera_rotation_system(
    mut mouse_motion: EventReader<MouseMotion>,
    mut camera_query: Query<(&mut GameCamera, &mut Transform)>,
    settings: Res<Settings>,
) {
    let mut total_delta = Vec2::ZERO;
    for event in mouse_motion.read() {
        total_delta += event.delta;
    }
    
    for (mut camera, mut transform) in camera_query.iter_mut() {
        // Update yaw (horizontal rotation)
        camera.yaw -= total_delta.x * camera.sensitivity;
        
        // Update pitch (vertical rotation)
        camera.pitch -= total_delta.y * camera.sensitivity;
        camera.pitch = camera.pitch.clamp(-89.0, 89.0);  // Prevent flipping
        
        // Apply rotation
        transform.rotation = Quat::from_euler(
            EulerRot::YXZ,
            camera.yaw.to_radians(),
            camera.pitch.to_radians(),
            0.0,
        );
    }
}
```

### View Modes

**First-Person**:
```rust
pub fn first_person_camera_system(
    player_query: Query<&Transform, (With<Player>, Without<GameCamera>)>,
    mut camera_query: Query<&mut Transform, With<GameCamera>>,
) {
    for player_transform in player_query.iter() {
        for mut camera_transform in camera_query.iter_mut() {
            // Camera at player eye level
            camera_transform.translation = player_transform.translation 
                + Vec3::new(0.0, 1.6, 0.0);
        }
    }
}
```

**Third-Person**:
```rust
pub fn third_person_camera_system(
    player_query: Query<&Transform, (With<Player>, Without<GameCamera>)>,
    mut camera_query: Query<(&mut Transform, &GameCamera), With<GameCamera>>,
) {
    for player_transform in player_query.iter() {
        for (mut camera_transform, camera) in camera_query.iter_mut() {
            // Calculate camera position behind player
            let offset = camera_transform.rotation * Vec3::new(0.0, 2.0, -5.0);
            camera_transform.translation = player_transform.translation + offset;
        }
    }
}
```

## User Interface

### HUD System

**Location**: `client/src/ui/hud/`

#### Hotbar

```rust
#[derive(Component)]
pub struct Hotbar {
    pub selected_slot: usize,
}

pub fn setup_hotbar(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(60.0),
            position_type: PositionType::Absolute,
            bottom: Val::Px(10.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        Hotbar { selected_slot: 0 },
    ))
    .with_children(|parent| {
        // Create 9 slots
        for i in 0..9 {
            parent.spawn((
                Node {
                    width: Val::Px(50.0),
                    height: Val::Px(50.0),
                    margin: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.2, 0.2, 0.2, 0.8)),
                HotbarSlot(i),
            ));
        }
    });
}

pub fn update_hotbar_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut hotbar: Query<&mut Hotbar>,
) {
    for mut hotbar in hotbar.iter_mut() {
        // Number keys 1-9
        for (i, key) in [
            KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3,
            KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6,
            KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9,
        ].iter().enumerate() {
            if keyboard.just_pressed(*key) {
                hotbar.selected_slot = i;
            }
        }
        
        // Mouse wheel
        // ... scroll handling
    }
}
```

#### Inventory

**Location**: `client/src/ui/hud/inventory/`

```rust
pub fn setup_inventory_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            display: Display::None,  // Hidden by default
            ..default()
        },
        InventoryUI,
    ))
    .with_children(|parent| {
        // Inventory panel
        parent.spawn(Node {
            width: Val::Px(400.0),
            height: Val::Px(300.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|parent| {
            // Create grid of slots (4 rows x 9 columns)
            for row in 0..4 {
                parent.spawn(Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(50.0),
                    flex_direction: FlexDirection::Row,
                    ..default()
                })
                .with_children(|parent| {
                    for col in 0..9 {
                        let slot_index = row * 9 + col;
                        spawn_inventory_slot(parent, slot_index);
                    }
                });
            }
        });
    });
}

pub fn toggle_inventory_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut inventory_ui: Query<&mut Style, With<InventoryUI>>,
) {
    if keyboard.just_pressed(KeyCode::KeyE) {
        for mut style in inventory_ui.iter_mut() {
            style.display = if style.display == Display::None {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}
```

#### Chat

```rust
#[derive(Component)]
pub struct ChatBox {
    pub messages: Vec<ChatMessage>,
    pub max_messages: usize,
}

pub fn render_chat_system(
    mut chat_events: EventReader<ChatMessageEvent>,
    mut chat_query: Query<(&mut ChatBox, &Children)>,
    mut text_query: Query<&mut Text>,
) {
    for event in chat_events.read() {
        for (mut chat_box, children) in chat_query.iter_mut() {
            // Add message
            chat_box.messages.push(event.0.clone());
            
            // Keep only recent messages
            while chat_box.messages.len() > chat_box.max_messages {
                chat_box.messages.remove(0);
            }
            
            // Update display
            for &child in children.iter() {
                if let Ok(mut text) = text_query.get_mut(child) {
                    update_chat_display(&mut text, &chat_box.messages);
                }
            }
        }
    }
}
```

#### Debug HUD

**Location**: `client/src/ui/hud/debug/`

```rust
pub fn setup_debug_hud(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        DebugText,
    ));
}

pub fn update_debug_text_system(
    diagnostics: Res<DiagnosticsStore>,
    player_query: Query<&Transform, With<Player>>,
    world_map: Res<ClientWorldMap>,
    mut debug_text: Query<&mut Text, With<DebugText>>,
    debug_settings: Res<DebugSettings>,
) {
    if !debug_settings.show_fps {
        return;
    }
    
    for mut text in debug_text.iter_mut() {
        let fps = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|d| d.smoothed())
            .unwrap_or(0.0);
        
        for player_transform in player_query.iter() {
            let pos = player_transform.translation;
            let chunk_pos = world_to_chunk_pos(pos);
            
            **text = format!(
                "FPS: {:.0}\n\
                 Position: ({:.1}, {:.1}, {:.1})\n\
                 Chunk: ({}, {})\n\
                 Loaded Chunks: {}\n",
                fps,
                pos.x, pos.y, pos.z,
                chunk_pos.x, chunk_pos.y,
                world_map.chunks.len(),
            );
        }
    }
}
```

### Menu System

**Location**: `client/src/ui/menus/`

#### Main Menu

```rust
pub fn setup_main_menu(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        MainMenu,
    ))
    .with_children(|parent| {
        // Title
        parent.spawn((
            Text::new("Rustcraft"),
            TextFont {
                font_size: 72.0,
                ..default()
            },
        ));
        
        // Buttons
        spawn_button(parent, "Singleplayer", MenuButton::Singleplayer);
        spawn_button(parent, "Multiplayer", MenuButton::Multiplayer);
        spawn_button(parent, "Settings", MenuButton::Settings);
        spawn_button(parent, "Exit", MenuButton::Exit);
    });
}

pub fn handle_menu_buttons(
    mut interaction_query: Query<
        (&Interaction, &MenuButton),
        Changed<Interaction>
    >,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for (interaction, button) in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            match button {
                MenuButton::Singleplayer => {
                    next_state.set(GameState::SoloMenu);
                }
                MenuButton::Multiplayer => {
                    next_state.set(GameState::MultiMenu);
                }
                MenuButton::Settings => {
                    next_state.set(GameState::SettingsMenu);
                }
                MenuButton::Exit => {
                    std::process::exit(0);
                }
            }
        }
    }
}
```

#### Pause Menu

```rust
pub fn toggle_pause_menu(
    keyboard: Res<ButtonInput<KeyCode>>,
    current_state: Res<State<GameState>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        match current_state.get() {
            GameState::Playing => {
                next_state.set(GameState::Paused);
            }
            GameState::Paused => {
                next_state.set(GameState::Playing);
            }
            _ => {}
        }
    }
}
```

## Render Distance Management

**Location**: `client/src/world/rendering/render_distance.rs`

```rust
pub fn adjust_render_distance_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut render_distance: ResMut<RenderDistance>,
) {
    if keyboard.just_pressed(KeyCode::KeyO) {
        render_distance.0 = (render_distance.0 - 1).max(4);
        info!("Render distance: {}", render_distance.0);
    }
    
    if keyboard.just_pressed(KeyCode::KeyP) {
        render_distance.0 = (render_distance.0 + 1).min(32);
        info!("Render distance: {}", render_distance.0);
    }
}

pub fn cull_distant_chunks(
    player_query: Query<&Transform, With<Player>>,
    render_distance: Res<RenderDistance>,
    mut world_map: ResMut<ClientWorldMap>,
    mut commands: Commands,
) {
    for player_transform in player_query.iter() {
        let player_chunk = world_to_chunk_pos(player_transform.translation);
        
        // Remove chunks outside render distance
        world_map.chunks.retain(|chunk_pos, chunk| {
            let distance = chunk_pos.distance_squared(player_chunk);
            let should_keep = distance <= render_distance.0.pow(2);
            
            if !should_keep {
                // Despawn mesh entity
                if let Some(entity) = chunk.entity {
                    commands.entity(entity).despawn_recursive();
                }
            }
            
            should_keep
        });
    }
}
```

## Performance Optimization

### Frustum Culling

Only render chunks in camera view:

```rust
pub fn frustum_culling_system(
    camera_query: Query<&Frustum, With<GameCamera>>,
    mut chunk_query: Query<(&mut Visibility, &Aabb, &GlobalTransform), With<ChunkMesh>>,
) {
    for frustum in camera_query.iter() {
        for (mut visibility, aabb, transform) in chunk_query.iter_mut() {
            let world_aabb = aabb.transformed(transform);
            *visibility = if frustum.intersects_obb(&world_aabb) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
}
```

### Level of Detail (Future)

```rust
// Future enhancement
pub fn lod_system(
    player_query: Query<&Transform, With<Player>>,
    mut chunk_query: Query<(&Transform, &mut ChunkMesh)>,
) {
    for player_transform in player_query.iter() {
        for (chunk_transform, mut mesh) in chunk_query.iter_mut() {
            let distance = player_transform.translation.distance(
                chunk_transform.translation
            );
            
            // Switch to lower detail at distance
            mesh.lod_level = if distance > 100.0 {
                LODLevel::Low
            } else if distance > 50.0 {
                LODLevel::Medium
            } else {
                LODLevel::High
            };
        }
    }
}
```

## Debugging Tools

### Wireframe Mode

```rust
pub fn toggle_wireframe(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut wireframe_config: ResMut<WireframeConfig>,
) {
    if keyboard.just_pressed(KeyCode::F8) {
        wireframe_config.global = !wireframe_config.global;
    }
}
```

### Chunk Boundaries

```rust
pub fn render_chunk_boundaries(
    mut gizmos: Gizmos,
    world_map: Res<ClientWorldMap>,
    debug_settings: Res<DebugSettings>,
) {
    if !debug_settings.show_chunk_boundaries {
        return;
    }
    
    for chunk_pos in world_map.chunks.keys() {
        let world_pos = Vec3::new(
            (chunk_pos.x * 16) as f32,
            0.0,
            (chunk_pos.y * 16) as f32,
        );
        
        // Draw chunk outline
        gizmos.cuboid(
            Transform::from_translation(world_pos + Vec3::new(8.0, 128.0, 8.0))
                .with_scale(Vec3::new(16.0, 256.0, 16.0)),
            Color::srgb(1.0, 0.0, 0.0),
        );
    }
}
```

## Troubleshooting

### Low Frame Rate
- Reduce render distance
- Disable debug overlays
- Check GPU utilization
- Profile rendering pipeline

### Texture Issues
- Verify texture atlas loaded
- Check UV coordinates
- Validate texture path
- Inspect material properties

### UI Not Showing
- Check visibility settings
- Verify game state
- Inspect UI node hierarchy
- Check z-index ordering

### Camera Problems
- Verify camera entity exists
- Check transform updates
- Inspect rotation limits
- Validate input handling

---

The rendering and UI systems work together to present the game world and provide player interaction. Understanding mesh generation, materials, and UI layout is essential for visual improvements.

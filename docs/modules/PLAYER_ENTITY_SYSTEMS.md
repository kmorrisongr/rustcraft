# Player and Entity Systems Documentation

## Overview

The player and entity systems handle all interactive game objects, including player characters, mobs, items, and their physics, AI, and interactions.

## Architecture

```
Player & Entity Systems
├── Player
│   ├── Movement & Physics
│   ├── Collision Detection
│   ├── Inventory Management
│   ├── Block Interactions
│   └── View Modes
│
├── Mobs
│   ├── AI Behavior
│   ├── Spawning
│   ├── Rendering
│   └── Synchronization
│
└── Items
    ├── Stacks
    ├── Pickup/Drop
    └── Usage
```

## Player System

### Player Data Structure

**Location**: `shared/src/players/data.rs`

```rust
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Player {
    pub client_id: ClientId,
    pub username: String,
    pub health: f32,
    pub max_health: f32,
}

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Velocity {
    pub linear: Vec3,
    pub angular: Vec3,
}

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Inventory {
    pub slots: [Option<ItemStack>; 36],
    pub hotbar_selection: usize,
}

impl Inventory {
    pub fn new() -> Self {
        Self {
            slots: [None; 36],
            hotbar_selection: 0,
        }
    }
    
    pub fn get_selected_item(&self) -> Option<&ItemStack> {
        self.slots[self.hotbar_selection].as_ref()
    }
    
    pub fn add_item(&mut self, item: ItemId, count: u32) -> bool {
        // Try to add to existing stacks first
        for slot in &mut self.slots {
            if let Some(stack) = slot {
                if stack.item_id == item && stack.count < stack.max_stack_size() {
                    let space = stack.max_stack_size() - stack.count;
                    let to_add = count.min(space);
                    stack.count += to_add;
                    
                    if to_add == count {
                        return true;  // All items added
                    }
                    count -= to_add;
                }
            }
        }
        
        // Add to empty slot
        for slot in &mut self.slots {
            if slot.is_none() {
                *slot = Some(ItemStack {
                    item_id: item,
                    count: count.min(item.max_stack_size()),
                });
                return true;
            }
        }
        
        false  // Inventory full
    }
    
    pub fn remove_item(&mut self, item: ItemId, count: u32) -> bool {
        let mut remaining = count;
        
        for slot in &mut self.slots {
            if let Some(stack) = slot {
                if stack.item_id == item {
                    let to_remove = remaining.min(stack.count);
                    stack.count -= to_remove;
                    remaining -= to_remove;
                    
                    if stack.count == 0 {
                        *slot = None;
                    }
                    
                    if remaining == 0 {
                        return true;
                    }
                }
            }
        }
        
        remaining == 0
    }
}
```

### Player Movement

**Location**: `shared/src/players/movement.rs`

```rust
pub const PLAYER_SPEED: f32 = 4.5;  // Blocks per second
pub const SPRINT_MULTIPLIER: f32 = 1.3;
pub const JUMP_VELOCITY: f32 = 8.0;
pub const GRAVITY: f32 = -20.0;
pub const FLY_SPEED: f32 = 10.0;

#[derive(Component)]
pub struct PlayerController {
    pub is_flying: bool,
    pub is_sprinting: bool,
    pub is_on_ground: bool,
}

pub fn player_movement_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<(
        &mut Transform,
        &mut Velocity,
        &mut PlayerController,
        &GameCamera,
    )>,
    time: Res<Time>,
) {
    for (mut transform, mut velocity, mut controller, camera) in player_query.iter_mut() {
        // Get movement input
        let mut movement = Vec3::ZERO;
        
        if keyboard.pressed(KeyCode::KeyW) {
            movement.z -= 1.0;
        }
        if keyboard.pressed(KeyCode::KeyS) {
            movement.z += 1.0;
        }
        if keyboard.pressed(KeyCode::KeyA) {
            movement.x -= 1.0;
        }
        if keyboard.pressed(KeyCode::KeyD) {
            movement.x += 1.0;
        }
        
        // Normalize diagonal movement
        if movement.length() > 0.0 {
            movement = movement.normalize();
        }
        
        // Apply camera rotation to movement
        let forward = camera.get_forward();
        let right = camera.get_right();
        let move_direction = forward * movement.z + right * movement.x;
        
        // Calculate speed
        let speed = if controller.is_sprinting {
            PLAYER_SPEED * SPRINT_MULTIPLIER
        } else {
            PLAYER_SPEED
        };
        
        // Apply movement
        if controller.is_flying {
            // Flying mode
            let mut fly_velocity = move_direction * FLY_SPEED;
            
            if keyboard.pressed(KeyCode::Space) {
                fly_velocity.y = FLY_SPEED;
            }
            if keyboard.pressed(KeyCode::ShiftLeft) {
                fly_velocity.y = -FLY_SPEED;
            }
            
            velocity.linear = fly_velocity;
        } else {
            // Walking mode
            velocity.linear.x = move_direction.x * speed;
            velocity.linear.z = move_direction.z * speed;
            
            // Apply gravity
            velocity.linear.y += GRAVITY * time.delta_secs();
            
            // Jump
            if keyboard.just_pressed(KeyCode::Space) && controller.is_on_ground {
                velocity.linear.y = JUMP_VELOCITY;
                controller.is_on_ground = false;
            }
        }
        
        // Apply velocity to position
        transform.translation += velocity.linear * time.delta_secs();
    }
}
```

### Collision Detection

**Location**: `shared/src/players/collision.rs`

```rust
pub const PLAYER_WIDTH: f32 = 0.6;
pub const PLAYER_HEIGHT: f32 = 1.8;
pub const PLAYER_EYE_HEIGHT: f32 = 1.6;

pub struct PlayerBoundingBox {
    pub min: Vec3,
    pub max: Vec3,
}

impl PlayerBoundingBox {
    pub fn from_position(pos: Vec3) -> Self {
        Self {
            min: pos + Vec3::new(-PLAYER_WIDTH / 2.0, 0.0, -PLAYER_WIDTH / 2.0),
            max: pos + Vec3::new(PLAYER_WIDTH / 2.0, PLAYER_HEIGHT, PLAYER_WIDTH / 2.0),
        }
    }
    
    pub fn intersects(&self, block_pos: IVec3) -> bool {
        let block_min = block_pos.as_vec3();
        let block_max = block_min + Vec3::ONE;
        
        self.min.x < block_max.x && self.max.x > block_min.x &&
        self.min.y < block_max.y && self.max.y > block_min.y &&
        self.min.z < block_max.z && self.max.z > block_min.z
    }
}

pub fn player_collision_system(
    world_map: Res<ServerWorldMap>,
    mut player_query: Query<(&mut Transform, &mut Velocity, &mut PlayerController)>,
) {
    for (mut transform, mut velocity, mut controller) in player_query.iter_mut() {
        // Create bounding box at new position
        let new_pos = transform.translation + velocity.linear * 0.016;  // Assume 60 FPS
        let bbox = PlayerBoundingBox::from_position(new_pos);
        
        // Check collision with blocks
        let mut collided = false;
        
        // Check blocks around player
        for y in (bbox.min.y.floor() as i32)..=(bbox.max.y.ceil() as i32) {
            for x in (bbox.min.x.floor() as i32)..=(bbox.max.x.ceil() as i32) {
                for z in (bbox.min.z.floor() as i32)..=(bbox.max.z.ceil() as i32) {
                    let block_pos = IVec3::new(x, y, z);
                    
                    if let Some(block) = get_block(&world_map, block_pos) {
                        if block.is_solid() && bbox.intersects(block_pos) {
                            resolve_collision(
                                &mut transform,
                                &mut velocity,
                                &mut controller,
                                block_pos,
                            );
                            collided = true;
                        }
                    }
                }
            }
        }
        
        // Check if on ground
        let ground_check = transform.translation + Vec3::new(0.0, -0.1, 0.0);
        controller.is_on_ground = is_on_solid_ground(&world_map, ground_check);
    }
}

fn resolve_collision(
    transform: &mut Transform,
    velocity: &mut Velocity,
    controller: &mut PlayerController,
    block_pos: IVec3,
) {
    let block_center = block_pos.as_vec3() + Vec3::splat(0.5);
    let player_center = transform.translation + Vec3::new(0.0, PLAYER_HEIGHT / 2.0, 0.0);
    let delta = player_center - block_center;
    
    // Find axis with smallest penetration
    let abs_delta = delta.abs();
    
    if abs_delta.y < abs_delta.x && abs_delta.y < abs_delta.z {
        // Vertical collision
        if delta.y > 0.0 {
            // Hit ceiling
            transform.translation.y = block_pos.y as f32 + 1.0;
            velocity.linear.y = 0.0;
        } else {
            // Hit floor
            transform.translation.y = block_pos.y as f32 + 1.0;
            velocity.linear.y = 0.0;
            controller.is_on_ground = true;
        }
    } else if abs_delta.x < abs_delta.z {
        // X-axis collision
        if delta.x > 0.0 {
            transform.translation.x = block_pos.x as f32 + 1.0 + PLAYER_WIDTH / 2.0;
        } else {
            transform.translation.x = block_pos.x as f32 - PLAYER_WIDTH / 2.0;
        }
        velocity.linear.x = 0.0;
    } else {
        // Z-axis collision
        if delta.z > 0.0 {
            transform.translation.z = block_pos.z as f32 + 1.0 + PLAYER_WIDTH / 2.0;
        } else {
            transform.translation.z = block_pos.z as f32 - PLAYER_WIDTH / 2.0;
        }
        velocity.linear.z = 0.0;
    }
}
```

### Player Interactions

**Location**: `client/src/player/interactions.rs`

```rust
pub fn block_interaction_system(
    mouse: Res<ButtonInput<MouseButton>>,
    camera_query: Query<(&Transform, &GameCamera)>,
    mut player_query: Query<&mut Inventory, With<LocalPlayer>>,
    world_map: Res<ClientWorldMap>,
    mut client: ResMut<RenetClient>,
) {
    for (camera_transform, _) in camera_query.iter() {
        // Raycast to find target block
        let ray_origin = camera_transform.translation;
        let ray_direction = camera_transform.forward();
        
        if let Some(raycast_result) = raycast(
            &world_map,
            ray_origin,
            ray_direction,
            5.0,  // Max reach distance
        ) {
            // Left click - break block
            if mouse.just_pressed(MouseButton::Left) {
                send_break_block(&mut client, raycast_result.position);
            }
            
            // Right click - place block
            if mouse.just_pressed(MouseButton::Right) {
                for mut inventory in player_query.iter_mut() {
                    if let Some(item_stack) = inventory.get_selected_item() {
                        // Calculate placement position (face normal)
                        let place_pos = raycast_result.position + raycast_result.normal;
                        
                        send_place_block(
                            &mut client,
                            place_pos,
                            item_stack.item_id.to_block_id(),
                        );
                    }
                }
            }
        }
    }
}

fn send_break_block(client: &mut RenetClient, pos: IVec3) {
    let message = ClientToServerMessage::BlockBreak(pos);
    client.send_message(
        CTS_STANDARD_CHANNEL,
        game_message_to_payload(&message),
    );
}

fn send_place_block(client: &mut RenetClient, pos: IVec3, block_id: BlockId) {
    let message = ClientToServerMessage::BlockPlace(BlockPlaceMessage {
        position: pos,
        block_id,
        direction: BlockDirection::Front,
    });
    client.send_message(
        CTS_STANDARD_CHANNEL,
        game_message_to_payload(&message),
    );
}
```

## Mob System

### Mob Types

**Location**: `shared/src/world/mobs.rs`

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MobType {
    Fox,
    // Future: Cow, Pig, Zombie, etc.
}

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Mob {
    pub mob_type: MobType,
    pub health: f32,
    pub max_health: f32,
    pub target: Option<Entity>,
}

impl Mob {
    pub fn new(mob_type: MobType) -> Self {
        let (health, max_health) = match mob_type {
            MobType::Fox => (10.0, 10.0),
        };
        
        Self {
            mob_type,
            health,
            max_health,
            target: None,
        }
    }
}
```

### Mob AI Behavior

**Location**: `server/src/mob/behavior.rs`

```rust
pub fn fox_ai_system(
    mut fox_query: Query<(&mut Transform, &mut Velocity, &Mob)>,
    player_query: Query<&Transform, (With<Player>, Without<Mob>)>,
    time: Res<Time>,
) {
    for (mut transform, mut velocity, mob) in fox_query.iter_mut() {
        if mob.mob_type != MobType::Fox {
            continue;
        }
        
        // Find nearest player
        let mut nearest_player = None;
        let mut nearest_distance = f32::MAX;
        
        for player_transform in player_query.iter() {
            let distance = transform.translation.distance(player_transform.translation);
            if distance < nearest_distance {
                nearest_distance = distance;
                nearest_player = Some(player_transform);
            }
        }
        
        // AI behavior states
        if let Some(player_transform) = nearest_player {
            if nearest_distance < 3.0 {
                // Run away from player (foxes are shy)
                let flee_direction = (transform.translation - player_transform.translation)
                    .normalize_or_zero();
                velocity.linear = flee_direction * 3.0;
            } else if nearest_distance > 20.0 {
                // Wander randomly when far from player
                wander_behavior(&mut velocity, &time);
            } else {
                // Idle/watch player
                velocity.linear = Vec3::ZERO;
                
                // Face player
                let look_direction = (player_transform.translation - transform.translation)
                    .normalize_or_zero();
                if look_direction.length() > 0.01 {
                    transform.rotation = Quat::from_rotation_y(
                        f32::atan2(look_direction.x, look_direction.z)
                    );
                }
            }
        } else {
            // No players nearby, wander
            wander_behavior(&mut velocity, &time);
        }
    }
}

fn wander_behavior(velocity: &mut Velocity, time: &Time) {
    // Simple random walk
    let noise = (time.elapsed_secs() * 0.5).sin();
    velocity.linear = Vec3::new(
        noise * 2.0,
        0.0,
        (noise * 1.7).cos() * 2.0,
    );
}
```

### Mob Spawning

**Location**: `client/src/mob/spawn.rs`

```rust
pub fn spawn_mob_client_system(
    mut commands: Commands,
    mut events: EventReader<MobUpdateEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mob_query: Query<&Mob>,
) {
    for event in events.read() {
        // Check if mob already exists
        if mob_query.get(event.entity).is_ok() {
            continue;
        }
        
        // Spawn mob visuals
        match event.mob_type {
            MobType::Fox => {
                spawn_fox(
                    &mut commands,
                    event.entity,
                    event.position,
                    &mut meshes,
                    &mut materials,
                );
            }
        }
    }
}

fn spawn_fox(
    commands: &mut Commands,
    entity: Entity,
    position: Vec3,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert((
        Mesh3d(meshes.add(Cuboid::new(0.6, 0.6, 0.9))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.8, 0.4, 0.1),  // Orange
            ..default()
        })),
        Transform::from_translation(position),
        Visibility::default(),
    ));
}
```

## Item Stack System

**Location**: `shared/src/world/items.rs`

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ItemId {
    // Blocks (placeable)
    Dirt,
    Grass,
    Stone,
    OakLog,
    // ... more blocks
    
    // Tools
    WoodenPickaxe,
    StonePickaxe,
    // ... more tools
    
    // Food
    Apple,
    Bread,
    // ... more food
}

impl ItemId {
    pub fn max_stack_size(&self) -> u32 {
        match self {
            // Tools don't stack
            ItemId::WoodenPickaxe | ItemId::StonePickaxe => 1,
            
            // Food stacks to 64
            ItemId::Apple | ItemId::Bread => 64,
            
            // Blocks stack to 64
            _ => 64,
        }
    }
    
    pub fn to_block_id(&self) -> Option<BlockId> {
        match self {
            ItemId::Dirt => Some(BlockId::Dirt),
            ItemId::Grass => Some(BlockId::Grass),
            ItemId::Stone => Some(BlockId::Stone),
            // ... more mappings
            _ => None,
        }
    }
}

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct ItemStack {
    pub item_id: ItemId,
    pub count: u32,
}

impl ItemStack {
    pub fn new(item_id: ItemId, count: u32) -> Self {
        Self {
            item_id,
            count: count.min(item_id.max_stack_size()),
        }
    }
    
    pub fn can_merge(&self, other: &ItemStack) -> bool {
        self.item_id == other.item_id && 
        self.count < self.max_stack_size()
    }
    
    pub fn max_stack_size(&self) -> u32 {
        self.item_id.max_stack_size()
    }
    
    pub fn split(&mut self, amount: u32) -> Option<ItemStack> {
        if amount >= self.count {
            return None;
        }
        
        self.count -= amount;
        Some(ItemStack::new(self.item_id, amount))
    }
}
```

## Player Synchronization

### Server to Client

```rust
pub fn broadcast_player_states(
    player_query: Query<(&Transform, &Velocity, &Player), Changed<Transform>>,
    mut server: ResMut<RenetServer>,
) {
    let mut updates = Vec::new();
    
    for (transform, velocity, player) in player_query.iter() {
        updates.push(PlayerUpdateEvent {
            client_id: player.client_id,
            position: transform.translation,
            rotation: transform.rotation,
            velocity: velocity.linear,
        });
    }
    
    if !updates.is_empty() {
        let message = ServerToClientMessage::PlayerUpdates(updates);
        server.broadcast_message(
            STC_STANDARD_CHANNEL,
            game_message_to_payload(&message),
        );
    }
}
```

### Client Reception

```rust
pub fn receive_player_updates(
    mut events: EventReader<PlayerUpdateEvent>,
    mut commands: Commands,
    mut player_query: Query<(&mut Transform, &mut Velocity, &Player)>,
) {
    for event in events.read() {
        let mut found = false;
        
        // Update existing player
        for (mut transform, mut velocity, player) in player_query.iter_mut() {
            if player.client_id == event.client_id {
                transform.translation = event.position;
                transform.rotation = event.rotation;
                velocity.linear = event.velocity;
                found = true;
                break;
            }
        }
        
        // Spawn new player if not found
        if !found {
            commands.spawn((
                Player {
                    client_id: event.client_id,
                    username: "Player".to_string(),
                    health: 20.0,
                    max_health: 20.0,
                },
                Transform::from_translation(event.position),
                Velocity { linear: event.velocity, angular: Vec3::ZERO },
            ));
        }
    }
}
```

## View Modes

**Location**: `shared/src/players/mod.rs`

```rust
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ViewMode {
    FirstPerson,
    ThirdPerson,
}

pub fn toggle_view_mode_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<&mut ViewMode, With<LocalPlayer>>,
) {
    if keyboard.just_pressed(KeyCode::F5) {
        for mut view_mode in player_query.iter_mut() {
            *view_mode = match *view_mode {
                ViewMode::FirstPerson => ViewMode::ThirdPerson,
                ViewMode::ThirdPerson => ViewMode::FirstPerson,
            };
        }
    }
}
```

## Performance Considerations

### Entity Culling

```rust
pub fn cull_distant_entities(
    player_query: Query<&Transform, With<LocalPlayer>>,
    mut entity_query: Query<(&Transform, &mut Visibility), (Without<LocalPlayer>, With<Mob>)>,
) {
    const MAX_DISTANCE: f32 = 100.0;
    
    for player_transform in player_query.iter() {
        for (entity_transform, mut visibility) in entity_query.iter_mut() {
            let distance = player_transform.translation.distance(
                entity_transform.translation
            );
            
            *visibility = if distance < MAX_DISTANCE {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
}
```

## Future Enhancements

- [ ] Player animations (walking, jumping, mining)
- [ ] More mob types with varied behaviors
- [ ] Mob pathfinding with A*
- [ ] Player stats (hunger, experience)
- [ ] Crafting system
- [ ] Tool durability
- [ ] Armor and equipment
- [ ] Particle effects
- [ ] Sound effects

## Troubleshooting

### Player Falls Through World
- Check collision detection is enabled
- Verify chunks are loaded before spawning
- Check block solidity properties

### Inventory Issues
- Verify network synchronization
- Check stack merging logic
- Validate item IDs

### Mob AI Problems
- Enable debug logging
- Check pathfinding constraints
- Verify behavior state machine

---

Understanding player mechanics, physics, and entity management is crucial for extending gameplay features and ensuring a responsive, interactive experience.

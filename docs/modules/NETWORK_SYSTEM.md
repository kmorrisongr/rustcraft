# Network System Documentation

## Overview

The network system enables multiplayer functionality in Rustcraft, using a client-server architecture with authoritative server design. It handles player connections, world synchronization, chat, and all multiplayer interactions.

## Architecture

```
Network System
├── Transport Layer (bevy_renet)
│   ├── UDP-based communication
│   ├── Reliable ordered channels
│   └── Packet compression (LZ4)
│
├── Server
│   ├── Connection management
│   ├── Message dispatching
│   ├── State broadcasting
│   └── Authentication
│
├── Client
│   ├── Connection setup
│   ├── Input buffering
│   ├── State reception
│   └── Prediction/reconciliation
│
└── Shared
    ├── Message definitions
    ├── Channel configuration
    └── Serialization utilities
```

## Network Protocol

### Transport Configuration

**Location**: `shared/src/lib.rs`

#### Channels

The network uses multiple channels for different message types:

```rust
// Client → Server Channels
pub const CTS_STANDARD_CHANNEL: u8 = 0;  // General messages
pub const CTS_AUTH_CHANNEL: u8 = 1;       // Authentication

// Server → Client Channels
pub const STC_STANDARD_CHANNEL: u8 = 0;   // General messages
pub const STC_CHUNK_DATA_CHANNEL: u8 = 1; // World data
pub const STC_AUTH_CHANNEL: u8 = 2;       // Authentication responses
```

**Why Multiple Channels?**
- Prevents large chunk data from blocking small messages
- Prioritizes critical messages (auth, player movement)
- Allows different reliability guarantees

#### Channel Configuration

```rust
pub fn get_customized_server_to_client_channels() -> Vec<ChannelConfig> {
    vec![
        ChannelConfig {
            channel_id: STC_STANDARD_CHANNEL,
            max_memory_usage_bytes: 128 * 1024 * 1024,  // 128 MB
            send_type: SendType::ReliableOrdered {
                resend_time: Duration::from_millis(300),
            },
        },
        // ... other channels
    ]
}
```

**Key Parameters**:
- **max_memory_usage_bytes**: Maximum buffer per channel
- **resend_time**: How long to wait before resending lost packets
- **available_bytes_per_tick**: Bandwidth throttling (5 MB/tick)

### Message Definitions

**Location**: `shared/src/messages/`

#### Client to Server Messages

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ClientToServerMessage {
    // Authentication
    AuthRegisterRequest(AuthRegisterRequest),
    
    // Player actions
    PlayerInput(PlayerInputMessage),
    BlockBreak(BlockPosition),
    BlockPlace(BlockPlaceMessage),
    
    // Chat
    ChatMessage(String),
    
    // World requests
    RequestChunk(IVec2),
    
    // Inventory
    InventoryUpdate(InventoryUpdateMessage),
}
```

#### Server to Client Messages

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerToClientMessage {
    // Authentication
    AuthRegisterResponse(AuthRegisterResponse),
    
    // World updates
    WorldUpdate(WorldUpdateMessage),
    BlockUpdate(BlockUpdateMessage),
    
    // Player updates
    PlayerSpawn(PlayerSpawnEvent),
    PlayerUpdate(PlayerUpdateEvent),
    PlayerDespawn(ClientId),
    
    // Mob updates
    MobUpdate(MobUpdateEvent),
    
    // Chat
    ChatBroadcast(ChatMessage),
    
    // Inventory
    ItemStackUpdate(ItemStackUpdateEvent),
}
```

### Message Serialization

Uses `bincode` with LZ4 compression:

```rust
pub fn game_message_to_payload<T: Serialize>(message: T) -> Vec<u8> {
    // Serialize with bincode
    let payload = bincode::options().serialize(&message).unwrap();
    
    // Compress with LZ4
    let compressed = lz4::block::compress(&payload, None, true).unwrap();
    
    // Log large messages
    if payload.len() > 1024 {
        debug!("Original: {}, Compressed: {}", 
               format_bytes(payload.len()),
               format_bytes(compressed.len()));
    }
    
    compressed
}

pub fn payload_to_game_message<T: DeserializeOwned>(
    payload: &[u8]
) -> Result<T, bincode::Error> {
    // Decompress
    let decompressed = lz4::block::decompress(payload, None)?;
    
    // Deserialize
    bincode::options().deserialize(&decompressed)
}
```

**Compression Benefits**:
- Typical chunk data: 80-90% compression ratio
- Reduces bandwidth significantly
- Minimal CPU overhead

## Server-Side Networking

### Server Setup

**Location**: `server/src/init.rs`

```rust
pub fn init(
    socket: SocketAddr,
    config: GameServerConfig,
    paths: GameFolderPaths,
) {
    let mut app = App::new();
    
    // Add networking resources
    let server = RenetServer::new(ConnectionConfig::default());
    app.insert_resource(server);
    
    // Add systems
    app.add_systems(Update, (
        handle_client_connections,
        handle_client_disconnections,
        process_client_messages,
        broadcast_state_updates,
    ));
    
    app.run();
}
```

### Connection Management

```rust
pub fn handle_client_connections(
    mut server: ResMut<RenetServer>,
    mut commands: Commands,
) {
    // Check for new connections
    for client_id in server.clients_id() {
        if !is_connected(client_id) {
            info!("Client {} connected", client_id);
            
            // Spawn player entity
            commands.spawn((
                Player::new(client_id),
                Transform::from_xyz(0.0, 64.0, 0.0),
                Velocity::default(),
                Inventory::new(),
            ));
        }
    }
}

pub fn handle_client_disconnections(
    mut server: ResMut<RenetServer>,
    mut commands: Commands,
    player_query: Query<(Entity, &Player)>,
) {
    for (entity, player) in player_query.iter() {
        if !server.is_connected(player.client_id) {
            info!("Client {} disconnected", player.client_id);
            
            // Despawn player
            commands.entity(entity).despawn_recursive();
            
            // Notify other clients
            broadcast_player_despawn(player.client_id);
        }
    }
}
```

### Message Dispatching

**Location**: `server/src/network/dispatcher.rs`

```rust
pub fn process_client_messages(
    mut server: ResMut<RenetServer>,
    mut world_map: ResMut<ServerWorldMap>,
    mut player_query: Query<(&mut Player, &mut Transform, &mut Inventory)>,
) {
    for client_id in server.clients_id() {
        // Process all channels
        for channel in 0..3 {
            while let Some(message) = server.receive_message(client_id, channel) {
                match payload_to_game_message(&message) {
                    Ok(msg) => handle_message(
                        msg,
                        client_id,
                        &mut world_map,
                        &mut player_query,
                    ),
                    Err(e) => error!("Failed to deserialize message: {}", e),
                }
            }
        }
    }
}

fn handle_message(
    message: ClientToServerMessage,
    client_id: ClientId,
    world_map: &mut ServerWorldMap,
    player_query: &mut Query<(&mut Player, &mut Transform, &mut Inventory)>,
) {
    match message {
        ClientToServerMessage::PlayerInput(input) => {
            handle_player_input(client_id, input, player_query);
        }
        
        ClientToServerMessage::BlockBreak(pos) => {
            handle_block_break(world_map, pos, client_id);
        }
        
        ClientToServerMessage::BlockPlace(msg) => {
            handle_block_place(world_map, msg, client_id);
        }
        
        ClientToServerMessage::ChatMessage(text) => {
            handle_chat_message(client_id, text);
        }
        
        // ... other message types
    }
}
```

### State Broadcasting

**Location**: `server/src/network/broadcast_world.rs`, `server/src/network/broadcast_chat.rs`

#### World Updates

```rust
pub fn broadcast_world_updates(
    world_map: Res<ServerWorldMap>,
    mut server: ResMut<RenetServer>,
    player_query: Query<(&Transform, &Player)>,
) {
    for (player_transform, player) in player_query.iter() {
        let player_chunk = world_to_chunk_pos(player_transform.translation);
        
        // Find chunks to send
        let chunks_to_send = get_chunks_in_radius(
            &world_map,
            player_chunk,
            VIEW_DISTANCE
        );
        
        for (chunk_pos, chunk) in chunks_to_send {
            let message = ServerToClientMessage::WorldUpdate(
                WorldUpdateMessage {
                    chunk_pos,
                    blocks: chunk.map.clone(),
                }
            );
            
            send_to_client(
                &mut server,
                player.client_id,
                message,
                STC_CHUNK_DATA_CHANNEL
            );
        }
    }
}
```

#### Player Updates

```rust
pub fn broadcast_player_updates(
    player_query: Query<(&Transform, &Velocity, &Player), Changed<Transform>>,
    mut server: ResMut<RenetServer>,
) {
    // Collect updates
    let updates: Vec<_> = player_query.iter()
        .map(|(transform, velocity, player)| PlayerUpdateEvent {
            client_id: player.client_id,
            position: transform.translation,
            rotation: transform.rotation,
            velocity: velocity.0,
        })
        .collect();
    
    // Broadcast to all clients
    if !updates.is_empty() {
        let message = ServerToClientMessage::PlayerUpdates(updates);
        server.broadcast_message(
            STC_STANDARD_CHANNEL,
            game_message_to_payload(&message)
        );
    }
}
```

#### Chat Broadcasting

```rust
pub fn broadcast_chat_system(
    mut chat_events: EventReader<ChatEvent>,
    mut server: ResMut<RenetServer>,
) {
    for event in chat_events.read() {
        let message = ServerToClientMessage::ChatBroadcast(
            ChatMessage {
                sender: event.sender_name.clone(),
                text: event.text.clone(),
                timestamp: event.timestamp,
            }
        );
        
        server.broadcast_message(
            STC_STANDARD_CHANNEL,
            game_message_to_payload(&message)
        );
    }
}
```

## Client-Side Networking

### Client Setup

**Location**: `client/src/network/setup.rs`

```rust
pub fn setup_network_client(
    server_address: SocketAddr,
    username: String,
) -> RenetClient {
    let client = RenetClient::new(ConnectionConfig::default());
    
    client.connect(server_address)?;
    
    Ok(client)
}
```

### Connection Flow

```rust
pub enum TargetServerState {
    NotConnected,
    Connecting,
    Authenticating,
    Connected,
    Failed(String),
}

pub fn establish_connection_system(
    mut client: ResMut<RenetClient>,
    mut state: ResMut<TargetServerState>,
    profile: Res<CurrentPlayerProfile>,
) {
    match *state {
        TargetServerState::NotConnected => {
            // Start connection
            *state = TargetServerState::Connecting;
        }
        
        TargetServerState::Connecting => {
            if client.is_connected() {
                // Send auth request
                let auth = ClientToServerMessage::AuthRegisterRequest(
                    AuthRegisterRequest {
                        username: profile.username.clone(),
                    }
                );
                send_message(&mut client, auth);
                *state = TargetServerState::Authenticating;
            }
        }
        
        TargetServerState::Authenticating => {
            // Wait for auth response
            // Handled in message processing
        }
        
        TargetServerState::Connected => {
            // Normal operation
        }
        
        TargetServerState::Failed(_) => {
            // Show error to user
        }
    }
}
```

### Input Buffering

**Location**: `client/src/network/buffered_client.rs`

Client prediction requires buffering inputs:

```rust
#[derive(Resource)]
pub struct PlayerTickInputsBuffer {
    pub inputs: VecDeque<TickInput>,
    pub next_tick: u64,
}

#[derive(Clone, Debug)]
pub struct TickInput {
    pub tick: u64,
    pub movement: Vec3,
    pub rotation: Quat,
    pub actions: PlayerActions,
}

pub fn buffer_player_inputs_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut buffer: ResMut<PlayerTickInputsBuffer>,
    time: Res<SyncTime>,
) {
    let input = TickInput {
        tick: buffer.next_tick,
        movement: get_movement_from_input(&keyboard),
        rotation: get_rotation_from_mouse(&mouse),
        actions: get_actions_from_input(&keyboard, &mouse),
    };
    
    buffer.inputs.push_back(input);
    buffer.next_tick += 1;
    
    // Keep only recent inputs (for reconciliation)
    while buffer.inputs.len() > 60 {
        buffer.inputs.pop_front();
    }
}
```

### Sending Inputs

```rust
pub fn upload_player_inputs_system(
    mut client: ResMut<RenetClient>,
    mut buffer: ResMut<PlayerTickInputsBuffer>,
    mut last_sent: Local<u64>,
) {
    // Send all new inputs
    for input in &buffer.inputs {
        if input.tick > *last_sent {
            let message = ClientToServerMessage::PlayerInput(
                PlayerInputMessage {
                    tick: input.tick,
                    movement: input.movement,
                    rotation: input.rotation,
                    actions: input.actions,
                }
            );
            
            send_message(&mut client, message);
            *last_sent = input.tick;
        }
    }
}
```

### Receiving Updates

**Location**: `client/src/network/world.rs`, `client/src/network/chat.rs`

```rust
pub fn poll_network_messages(
    mut client: ResMut<RenetClient>,
    mut world_events: EventWriter<WorldUpdateEvent>,
    mut player_events: EventWriter<PlayerUpdateEvent>,
    mut chat_events: EventWriter<ChatMessageEvent>,
) {
    for channel in 0..3 {
        while let Some(message) = client.receive_message(channel) {
            match payload_to_game_message(&message) {
                Ok(ServerToClientMessage::WorldUpdate(update)) => {
                    world_events.send(WorldUpdateEvent(update));
                }
                
                Ok(ServerToClientMessage::PlayerUpdate(update)) => {
                    player_events.send(PlayerUpdateEvent(update));
                }
                
                Ok(ServerToClientMessage::ChatBroadcast(msg)) => {
                    chat_events.send(ChatMessageEvent(msg));
                }
                
                Ok(msg) => {
                    handle_other_message(msg);
                }
                
                Err(e) => {
                    error!("Failed to deserialize: {}", e);
                }
            }
        }
    }
}
```

### Prediction and Reconciliation

Client predicts movement locally, then reconciles with server:

```rust
pub fn client_prediction_system(
    mut player: Query<(&mut Transform, &mut Velocity), With<LocalPlayer>>,
    buffer: Res<PlayerTickInputsBuffer>,
    physics: Res<PhysicsConfig>,
) {
    // Apply most recent input locally
    if let Some(input) = buffer.inputs.back() {
        for (mut transform, mut velocity) in player.iter_mut() {
            // Predict movement
            apply_movement(&mut transform, &mut velocity, input, &physics);
        }
    }
}

pub fn server_reconciliation_system(
    mut player: Query<(&mut Transform, &mut Velocity), With<LocalPlayer>>,
    mut buffer: ResMut<PlayerTickInputsBuffer>,
    mut server_updates: EventReader<PlayerUpdateEvent>,
) {
    for update in server_updates.read() {
        // Server sent authoritative position
        for (mut transform, mut velocity) in player.iter_mut() {
            // Update to server position
            transform.translation = update.position;
            velocity.0 = update.velocity;
            
            // Replay inputs after server tick
            for input in &buffer.inputs {
                if input.tick > update.tick {
                    apply_movement(&mut transform, &mut velocity, input, &physics);
                }
            }
        }
    }
}
```

## Authentication

### Client Request

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuthRegisterRequest {
    pub username: String,
}
```

### Server Response

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuthRegisterResponse {
    pub success: bool,
    pub client_id: ClientId,
    pub message: String,
}
```

### Authentication Flow

```rust
// Server side
pub fn handle_auth_request(
    request: AuthRegisterRequest,
    client_id: ClientId,
    mut players: Query<&mut Player>,
) -> AuthRegisterResponse {
    // Validate username
    if request.username.is_empty() {
        return AuthRegisterResponse {
            success: false,
            client_id,
            message: "Username cannot be empty".to_string(),
        };
    }
    
    // Check for duplicates
    for player in players.iter() {
        if player.username == request.username {
            return AuthRegisterResponse {
                success: false,
                client_id,
                message: "Username already taken".to_string(),
            };
        }
    }
    
    // Accept
    AuthRegisterResponse {
        success: true,
        client_id,
        message: format!("Welcome, {}!", request.username),
    }
}
```

## Network Cleanup

**Location**: `server/src/network/cleanup.rs`, `client/src/network/cleanup.rs`

### Server Cleanup

```rust
pub fn cleanup_disconnected_clients(
    mut commands: Commands,
    mut server: ResMut<RenetServer>,
    player_query: Query<(Entity, &Player)>,
) {
    // Remove entities for disconnected clients
    for (entity, player) in player_query.iter() {
        if !server.is_connected(player.client_id) {
            commands.entity(entity).despawn_recursive();
        }
    }
}
```

### Client Cleanup

```rust
pub fn cleanup_on_disconnect(
    client: Res<RenetClient>,
    mut commands: Commands,
    entity_query: Query<Entity, Without<LocalPlayer>>,
) {
    if !client.is_connected() {
        // Despawn all remote entities
        for entity in entity_query.iter() {
            commands.entity(entity).despawn_recursive();
        }
    }
}
```

## Error Handling

### Network Failure Handler

```rust
pub fn network_failure_handler(
    client: Res<RenetClient>,
    mut state: ResMut<TargetServerState>,
) {
    if let Some(error) = client.disconnection_reason() {
        error!("Network error: {:?}", error);
        *state = TargetServerState::Failed(
            format!("Connection lost: {:?}", error)
        );
    }
}
```

### Timeout Handling

```rust
const TIMEOUT_DURATION: Duration = Duration::from_secs(10);

pub fn connection_timeout_system(
    time: Res<Time>,
    mut timeout_timer: Local<Option<Instant>>,
    client: Res<RenetClient>,
    mut state: ResMut<TargetServerState>,
) {
    match *state {
        TargetServerState::Connecting => {
            if timeout_timer.is_none() {
                *timeout_timer = Some(Instant::now());
            }
            
            if timeout_timer.unwrap().elapsed() > TIMEOUT_DURATION {
                *state = TargetServerState::Failed(
                    "Connection timeout".to_string()
                );
            }
        }
        
        TargetServerState::Connected => {
            *timeout_timer = None;
        }
        
        _ => {}
    }
}
```

## Performance Optimization

### Bandwidth Management

```rust
// Throttle updates per tick
const MAX_BYTES_PER_TICK: usize = 5 * 1024 * 1024;  // 5 MB

pub fn throttle_world_updates(
    chunks: &[ChunkUpdate],
    max_bytes: usize,
) -> Vec<ChunkUpdate> {
    let mut total_bytes = 0;
    let mut result = Vec::new();
    
    for chunk in chunks {
        let chunk_bytes = estimate_chunk_size(chunk);
        if total_bytes + chunk_bytes <= max_bytes {
            result.push(chunk.clone());
            total_bytes += chunk_bytes;
        } else {
            break;  // Hit limit, send rest next tick
        }
    }
    
    result
}
```

### Delta Compression

Only send changed data:

```rust
pub struct DeltaWorldUpdate {
    pub chunk_pos: IVec2,
    pub added_blocks: HashMap<IVec3, BlockData>,
    pub removed_blocks: Vec<IVec3>,
}

// vs. full update
pub struct FullWorldUpdate {
    pub chunk_pos: IVec2,
    pub all_blocks: HashMap<IVec3, BlockData>,  // Much larger
}
```

### Priority Queuing

```rust
pub fn prioritize_messages(
    messages: &mut Vec<Message>,
    player_pos: Vec3,
) {
    messages.sort_by_key(|msg| {
        match msg {
            // High priority
            Message::PlayerUpdate(_) => 0,
            Message::ChatMessage(_) => 1,
            
            // Medium priority (by distance)
            Message::WorldUpdate(update) => {
                let distance = chunk_distance(update.chunk_pos, player_pos);
                100 + distance as i32
            }
            
            // Low priority
            Message::MobUpdate(_) => 1000,
        }
    });
}
```

## Testing Multiplayer

### Local Testing

```bash
# Terminal 1: Server
cargo run --bin server -- --port 8000 --world testworld

# Terminal 2: Client 1
cargo run --bin client

# Terminal 3: Client 2
./run2.sh
```

### Network Simulation

For testing latency and packet loss:

```rust
// Add artificial delay
std::thread::sleep(Duration::from_millis(50));

// Simulate packet loss
if rand::random::<f32>() < 0.05 {
    continue;  // Drop 5% of packets
}
```

## Troubleshooting

### Connection Issues

**Can't connect to server**:
- Verify server is running: `netstat -an | grep 8000`
- Check firewall settings
- Ensure correct IP and port
- Verify network reachability

### Desync Issues

**Client and server state differs**:
- Check message serialization
- Verify tick synchronization
- Enable debug logging
- Check for dropped messages

### Performance Problems

**High latency, lag**:
- Reduce send rate
- Optimize message size
- Use delta compression
- Prioritize critical messages

### Debugging Tools

```rust
// Enable verbose network logging
pub fn log_network_stats_system(
    client: Res<RenetClient>,
) {
    debug!("RTT: {:?}", client.rtt());
    debug!("Packet loss: {:.2}%", client.packet_loss() * 100.0);
    debug!("Bytes sent: {}", client.bytes_sent());
    debug!("Bytes received: {}", client.bytes_received());
}
```

---

The network system is critical for multiplayer functionality. Understanding message flow, prediction, and synchronization is essential for a smooth multiplayer experience.

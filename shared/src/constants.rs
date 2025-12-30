use bevy::prelude::*;

pub const PROTOCOL_ID: u64 = 0;
pub const TICKS_PER_SECOND: u64 = 20;
pub const CHUNK_SIZE: i32 = 16;
pub const SEA_LEVEL: i32 = 62;
pub const MAX_INVENTORY_SLOTS: u32 = 4 * 9;
pub const DEFAULT_RENDER_DISTANCE: i32 = 8;
pub const UNIX_EPOCH_TIME_ERROR: &str = "System time is before UNIX_EPOCH";
pub const SOCKET_LOCAL_ADDR_ERROR: &str = "Failed to retrieve local address for UDP socket";
pub const SOCKET_BIND_ERROR: &str = "Failed to bind UDP socket";
pub const TARGET_SERVER_ADDR_ERROR: &str =
    "Target server address missing when initializing connection";
pub const NETCODE_CLIENT_TRANSPORT_ERROR: &str = "Failed to create Netcode client transport";
pub const NETCODE_SERVER_TRANSPORT_ERROR: &str = "Failed to create Netcode server transport";
pub const USERNAME_MISSING_AUTHENTICATED_ERROR: &str =
    "Username missing while handling authenticated session token";
pub const USERNAME_MISSING_CONNECTION_ERROR: &str =
    "Username missing while establishing connection";
pub const HALF_BLOCK: Vec3 = Vec3 {
    x: 0.5,
    y: 0.5,
    z: 0.5,
};

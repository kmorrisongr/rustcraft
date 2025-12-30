use crate::{
    network::{
        cleanup::cleanup_all_players_from_world,
        dispatcher::{self, setup_resources_and_events},
    },
    world::{data::SAVE_PATH, load_from_file::load_world_data},
};
use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
};
use bevy_app::ScheduleRunnerPlugin;
use bevy_renet::{netcode::NetcodeServerTransport, RenetServerPlugin};
use bevy_renet::{
    netcode::{NetcodeServerPlugin, ServerAuthentication, ServerConfig},
    renet::RenetServer,
};
use serde::{Deserialize, Serialize};
use shared::{
    constants::{
        NETCODE_SERVER_TRANSPORT_ERROR, SOCKET_LOCAL_ADDR_ERROR, UNIX_EPOCH_TIME_ERROR,
    },
    get_shared_renet_config, messages::PlayerId,
    world::{ServerChunkWorldMap, ServerWorldMap},
    GameFolderPaths, GameServerConfig, TICKS_PER_SECOND,
};
use std::fmt::{Debug, Display, Formatter};
use std::time::{Duration, SystemTime, SystemTimeError};
use std::{collections::HashMap, net::IpAddr};

use std::net::{SocketAddr, UdpSocket};

#[derive(Resource, Serialize, Deserialize, Debug, Clone)]
pub struct ServerTime(pub u64);

#[derive(Debug)]
pub struct LobbyPlayer {
    pub name: String,
}

impl LobbyPlayer {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

// ServerLobby represents the list of players connected to the server
// (who may or may not be in the game world yet)
#[derive(Debug, Default, Resource)]
pub struct ServerLobby {
    pub players: HashMap<PlayerId, LobbyPlayer>,
}

#[derive(Debug)]
pub(crate) enum NetcodeSetupError {
    SocketAddr(std::io::Error),
    Time(SystemTimeError),
    Transport(String),
}

impl Display for NetcodeSetupError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NetcodeSetupError::SocketAddr(err) => write!(f, "{}: {err}", SOCKET_LOCAL_ADDR_ERROR),
            NetcodeSetupError::Time(err) => write!(f, "{}: {err}", UNIX_EPOCH_TIME_ERROR),
            NetcodeSetupError::Transport(err) => {
                write!(f, "{}: {err}", NETCODE_SERVER_TRANSPORT_ERROR)
            }
        }
    }
}

#[allow(dead_code)]
pub fn acquire_local_ephemeral_udp_socket(ip: IpAddr) -> std::io::Result<UdpSocket> {
    acquire_socket_by_port(ip, 0)
}

pub fn acquire_socket_by_port(ip: IpAddr, port: u16) -> std::io::Result<UdpSocket> {
    let addr = SocketAddr::new(ip, port);
    UdpSocket::bind(addr)
}

pub fn add_netcode_network(
    socket: UdpSocket,
) -> Result<(RenetServer, NetcodeServerTransport, SocketAddr), NetcodeSetupError> {
    let granted_addr: SocketAddr = socket
        .local_addr()
        .map_err(NetcodeSetupError::SocketAddr)?;

    let current_time: Duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(NetcodeSetupError::Time)?;
    let server_config = ServerConfig {
        current_time,
        max_clients: 64,
        protocol_id: shared::PROTOCOL_ID,
        public_addresses: vec![granted_addr],
        authentication: ServerAuthentication::Unsecure,
    };

    let transport: NetcodeServerTransport = NetcodeServerTransport::new(server_config, socket)
        .map_err(|err| NetcodeSetupError::Transport(err.to_string()))?;

    let server = RenetServer::new(get_shared_renet_config());

    Ok((server, transport, granted_addr))
}

pub fn init(socket: UdpSocket, config: GameServerConfig, game_folder_paths: GameFolderPaths) {
    let (server, transport, addr) = match add_netcode_network(socket) {
        Ok(data) => data,
        Err(err) => {
            error!("{err}");
            return;
        }
    };

    let mut app = App::new();
    app.add_plugins(
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
            1.0 / TICKS_PER_SECOND as f64,
        ))),
    );

    app.add_plugins(RenetServerPlugin);
    app.add_plugins(FrameTimeDiagnosticsPlugin::default());
    app.add_plugins(LogDiagnosticsPlugin::default());
    app.add_plugins(bevy::log::LogPlugin::default());

    app.insert_resource(ServerLobby::default());
    app.insert_resource(game_folder_paths.clone());

    let world_name = &config.world_name.clone();

    app.insert_resource(config);

    info!("Starting server on {}", addr);

    app.add_plugins(NetcodeServerPlugin);
    app.insert_resource(server);
    app.insert_resource(transport);

    setup_resources_and_events(&mut app);

    // Load world from files
    let world_data = match load_world_data(world_name, &game_folder_paths) {
        Ok(data) => data,
        Err(err) => {
            error!(
                "Failed to load world {} & failed to create a default world : {}",
                world_name, err
            );
            panic!()
        }
    };

    let mut world_map = ServerWorldMap {
        name: world_data.name,
        chunks: ServerChunkWorldMap {
            map: world_data.map,
            chunks_to_update: Vec::new(),
            generation_requests: HashMap::new(),
        },
        players: HashMap::new(),
        mobs: world_data.mobs,
        item_stacks: world_data.item_stacks,
        time: world_data.time,
    };

    cleanup_all_players_from_world(&mut world_map);

    // Insert world_map and seed into ressources
    app.insert_resource(world_map);
    app.insert_resource(world_data.seed);
    app.insert_resource(ServerTime(world_data.time));

    // Create save folder if does not already exist
    let save_folder = format!(
        "{}{}/players/",
        game_folder_paths.game_folder_path.join(SAVE_PATH).display(),
        world_name
    );

    if let Err(err) = std::fs::create_dir_all(save_folder) {
        error!(
            "Could not create save directory for map {} : {}",
            world_name, err
        );
        panic!();
    }

    dispatcher::register_systems(&mut app);

    app.run();
}

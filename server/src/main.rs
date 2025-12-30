use std::net::Ipv4Addr;

use crate::init::acquire_socket_by_port;
use clap::Parser;
use shared::constants::{DEFAULT_RENDER_DISTANCE, SOCKET_BIND_ERROR};
use shared::{get_game_folder_paths, GameServerConfig};

mod init;
mod mob;
mod network;
mod world;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = 8000)]
    port: u16,

    #[arg(short, long, default_value = "default")]
    world: String,

    #[arg(short, long)]
    game_folder_path: Option<String>,

    #[arg(short, long, default_value_t = DEFAULT_RENDER_DISTANCE)]
    render_distance: i32,
}

fn main() {
    let args = Args::parse();

    // Validate render_distance is within reasonable range
    if args.render_distance < 1 || args.render_distance > 32 {
        eprintln!("Error: render_distance must be between 1 and 32 (inclusive).");
        eprintln!("Got: {}", args.render_distance);
        eprintln!("Negative values would cause unexpected chunk ranges, and very large values would cause performance issues.");
        std::process::exit(1);
    }

    let socket = match acquire_socket_by_port(std::net::IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), args.port) {
        Ok(socket) => socket,
        Err(err) => {
            eprintln!("{}: {err}", SOCKET_BIND_ERROR);
            std::process::exit(1);
        }
    };

    init::init(
        socket,
        GameServerConfig {
            world_name: args.world,
            is_solo: false,
            broadcast_render_distance: args.render_distance,
        },
        get_game_folder_paths(args.game_folder_path, None),
    );
}

mod camera;
mod constants;
mod effects;
mod entities;
mod game;
mod input;
mod mob;
mod network;
mod player;
mod shaders;
mod ui;
mod world;

use crate::ui::menus::MenusPlugin;
use bevy::{
    prelude::*,
    render::{
        settings::{RenderCreation, WgpuFeatures, WgpuSettings},
        RenderPlugin,
    },
    window::PresentMode,
};
use bevy_inspector_egui::{bevy_egui::EguiPlugin, DefaultInspectorConfigPlugin};
use clap::Parser;
use constants::{TEXTURE_PATH_BASE, TEXTURE_PATH_CUSTOM};
use input::{data::GameAction, keyboard::get_bindings, spawn_global_input_manager};
use leafwing_input_manager::prelude::*;
use menus::solo::SelectedWorld;
use shared::{game_state::GameState, get_game_folder_paths, SpecialFlag};
use std::panic;
use ui::{
    hud::debug::inspector::inspector_ui,
    menus::{self, splash},
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Flag to use custom textures
    #[arg(long, help = "Use custom textures instead of base textures")]
    use_custom_textures: bool,

    #[arg(short, long)]
    game_folder_path: Option<String>,

    #[arg(
        short,
        long,
        help = "Allows overriding of the asset folder path, defaults to <game_folder_path>/data"
    )]
    assets_folder_path: Option<String>,

    #[arg(long)]
    special_flag: bool,

    #[arg(short, long, help = "Player name to use for the game")]
    player_name: Option<String>,
}

#[derive(Component)]
pub struct MenuCamera;

pub const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);

#[derive(Message)]
pub struct LoadWorldEvent {
    pub world_name: String,
}

/// Resource to pass the loaded InputMap to the GlobalInputManager spawning system.
#[derive(Resource)]
pub struct LoadedInputMap(pub InputMap<GameAction>);

#[derive(Resource, Debug)]
pub struct TexturePath {
    pub path: String,
}

#[derive(Resource, Debug)]
pub struct PlayerNameSupplied {
    pub name: String,
}

fn main() {
    // Set up a custom panic hook to handle application exit gracefully.
    // During shutdown (whether from CMD+Q on macOS or clicking the Quit button),
    // Bevy's ECS resources may be in an inconsistent state, causing panics when
    // systems try to access resources that are being cleaned up. This hook suppresses
    // the scary stack trace for these shutdown-related panics while still allowing
    // normal unwinding so that destructors run and resources are properly released.
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let panic_message = panic_info.to_string();

        // Check if this is an app-exit panic we want to suppress
        // These occur when Bevy resources are accessed during shutdown
        if panic_message.contains("Resource requested by")
            && panic_message.contains("does not exist")
        {
            eprintln!("Application exiting...");
            // TODO: This will not allow further unwinding, but it prevents annoying popups :shrug:
            // Replace in the future.
            std::process::exit(0);
        }

        // For all other panics, use the default behavior
        original_hook(panic_info);
    }));

    // Parse command-line arguments
    let args = Args::parse();

    // Determine which texture path to use
    let texture_path = if args.use_custom_textures {
        TEXTURE_PATH_CUSTOM
    } else {
        TEXTURE_PATH_BASE
    };

    let special_flag = args.special_flag;

    println!(
        "Using {} for textures",
        if args.use_custom_textures {
            "custom textures"
        } else {
            "base textures"
        }
    );

    let game_folder_paths = get_game_folder_paths(args.game_folder_path, args.assets_folder_path);

    println!(
        "Starting application with game folder: {}",
        game_folder_paths.game_folder_path.display()
    );

    let special_flag = SpecialFlag { special_flag };

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            // Ensures that pixel-art textures will remain pixelated, and not become a blurry mess
            .set(ImagePlugin::default_nearest())
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(WgpuSettings {
                    // WARNING: This is a native-only feature. It will not work with WebGL or WebGPU
                    features: WgpuFeatures::POLYGON_MODE_LINE,
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                file_path: "../data".to_string(),
                // TODO: Remove unapproved_path_mode once the asset loading system has been improved
                unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
                ..Default::default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Rustcraft".to_string(),
                    present_mode: PresentMode::AutoVsync,
                    ..default()
                }),
                ..default()
            }),
    );

    app.add_plugins(EguiPlugin::default())
        .add_plugins(DefaultInspectorConfigPlugin)
        .add_systems(Update, inspector_ui);

    // Add leafwing-input-manager for action-based input handling
    app.add_plugins(InputManagerPlugin::<GameAction>::default())
        .add_systems(Startup, spawn_global_input_manager);

    app.add_message::<LoadWorldEvent>();
    network::add_base_netcode(&mut app);
    app.insert_resource(LoadedInputMap(get_bindings(&game_folder_paths)))
        .insert_resource(SelectedWorld::default())
        .insert_resource(TexturePath {
            path: texture_path.to_string(),
        })
        .insert_resource(game_folder_paths)
        .insert_resource(special_flag)
        .insert_resource(PlayerNameSupplied {
            name: args.player_name.unwrap_or_else(|| "Player".to_string()),
        })
        .init_state::<GameState>()
        // Adds the plugins for each state
        .add_plugins((splash::splash_plugin, MenusPlugin, game::game_plugin))
        .run();
}

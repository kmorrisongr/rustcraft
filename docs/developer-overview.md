# Developer Overview

This repository is a Rust workspace composed of three crates plus a data folder:

- **shared/** crate:
  - Cross-crate types, constants, world data structures, and the serializable game protocol messages exchanged between client and server.
  - Network payloads are compressed/serialized via `bincode` and `lz4` (`game_message_to_payload` / `payload_to_game_message`) to keep bandwidth usage low while retaining fast `encode`/`decode` times.
  - Channel configuration lives in `shared/src/lib.rs`, while world sizing and tick rate defaults are in `shared/src/constants.rs`.
- **client/**: Bevy-based game client. The entry point (`client/src/main.rs`) wires CLI options, sets up render/window configuration, declares the global `GameState` (Splash → Menu → PreGameLoading → Game), and installs UI and gameplay plugins.
- **server/**: headless Bevy schedule runner that hosts world state and networking. The entry point (`server/src/main.rs`) parses CLI flags (port/world) and calls `init::init` to configure networking, load saves, and register systems.
- **data/**: game assets (textures, models, sounds) loaded by the client via `AssetPlugin`.

## Client runtime flow
- `game::game_plugin` is the core gameplay plugin. It seeds render/time resources, spawns lighting and HUD, and installs systems for player input, camera control, block interactions, debug overlays, and chunk rendering.
- Networking systems (`network::...`) connect to either a locally launched server or a remote host and keep `ClientWorldMap` synchronized.
- UI and menus live under `ui/` (`splash`, `menus`, HUD widgets). Input bindings are defined in `input/` and persisted through the game folder.
- World rendering and time helpers are under `world/` (celestial bodies, per-chunk rendering, client-side clocks).

## Server runtime flow
- `init::init` builds a minimal Bevy app that ticks at `TICKS_PER_SECOND` (defined in `shared/src/constants.rs`, currently 20) and sets up `bevy_renet` transport.
- It loads a world from disk (`world::load_from_file`), registers events/resources via `network::dispatcher`, and persists saves under the provided game folder.
- World logic lives under `world/`: generation (`generation.rs`), background chunk work, simulation, block interactions (`handle_block_interactions`), item stacks, and saving. Chat and player cleanup utilities are under `network/`.

## Shared foundations and configuration
- Message enums under `shared/src/messages/` define the protocol for players, mobs, chat, world updates, and authentication.
- `ChannelResolvableExt` maps each message to a Renet channel so reliable delivery semantics stay consistent.
- Common world definitions are under `shared/src/world/` (chunk storage, block and item types, seeds). Cross-crate resources such as `GameFolderPaths`, `SpecialFlag`, and `GameServerConfig` are declared in `shared/src/lib.rs`.

## Assets, saves, and CLI knobs
- Default save/asset paths vary by platform (see `default_game_folder_paths`); both client and server accept `--game-folder-path`, and the client accepts `--assets-folder-path` to override locations.
- Additional client toggles: `--use-custom-textures`, `--player-name`, and a `--special-flag` hook.
- Saved data (worlds, player inventories, item stacks) is stored inside the configured game folder, with server saves placed under `saves/<world>` relative to that path. The `data/` directory is copied into release builds.

## Working on the codebase
- Toolchain: a nightly compiler is required (the build uses `-Zshare-generics`); install `rustup` if needed, then run `rustup install nightly` and `rustup default nightly` before running `cargo test` or `cargo run`.
- Extension points:
  - World generation/biomes: `server/world/generation.rs` and supporting modules.
  - Block/item definitions and chunk constants: `shared/src/world/` and `shared/src/constants.rs`.
  - Client visuals and rendering: `client/world/rendering/`, camera controls in `client/camera/`, particles/animations in `client/mob/`.
  - UI & input: `client/ui/` for menus/HUD, `client/input/` for key bindings, `client/player/` for movement.
  - Networking: message shapes in `shared/src/messages/` and client/server plumbing in `client/src/network/` and `server/src/network/`.

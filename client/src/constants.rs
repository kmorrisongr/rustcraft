pub const INTERACTION_DISTANCE: f32 = 7.;
pub const BASE_ROUGHNESS: f32 = 0.6;
pub const BASE_SPECULAR_HIGHLIGHT: f32 = 0.;

// 20 ticks * 60 seconds * 20 minutes
// pub const DAY_DURATION_IN_TICKS: u64 = 20 * 60 * 20;
// Debug/testing: 1 minute
pub const DAY_DURATION_IN_TICKS: u64 = 20 * 60;

pub const MAX_HOTBAR_SLOTS: u32 = 9;

pub const HOTBAR_CELL_SIZE: f32 = 50.;
pub const HOTBAR_PADDING: f32 = 5.;
pub const HOTBAR_BORDER: f32 = 5.;

pub const SAVE_PATH: &str = "saves/";
pub const SERVER_LIST_SAVE_NAME: &str = "servers.ron";
pub const BINDS_PATH: &str = "keybindings.ron";

pub const GRASS_COLOR: [f32; 4] = [0.1, 1.0, 0.3, 1.0];

pub const TEXTURE_PATH_BASE: &str = "graphics/base_textures/";
pub const TEXTURE_PATH_CUSTOM: &str = "graphics/custom_textures/";

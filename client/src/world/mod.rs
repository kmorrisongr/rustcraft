pub mod celestial;
pub mod data;
pub mod fluid_sync;
pub mod rendering;
pub mod time;

pub use data::*;
pub use rendering::*;
pub use fluid_sync::*;

use bevy::prelude::Resource;

#[derive(Resource)]
pub struct FirstChunkReceived(pub bool);

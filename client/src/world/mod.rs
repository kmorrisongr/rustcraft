pub mod celestial;
pub mod data;
pub mod fluid_sync;
pub mod rendering;
pub mod time;

pub use data::*;
pub use fluid_sync::*;
pub use rendering::*;

use bevy::prelude::Resource;

#[derive(Resource)]
pub struct FirstChunkReceived(pub bool);

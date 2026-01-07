//! Water shader integration with bevy_water crate
//!
//! This module re-exports the bevy_water types and provides a marker component
//! for water meshes in chunk rendering.

use bevy::prelude::*;

// Re-export bevy_water types for use throughout the codebase
pub use bevy_water::material::{StandardWaterMaterial, WaterMaterial};
pub use bevy_water::{WaterPlugin, WaterSettings};

/// Component marker for entities using water material
///
/// This is used to identify water mesh entities in the chunk rendering system.
#[derive(Component)]
pub struct WaterMesh;

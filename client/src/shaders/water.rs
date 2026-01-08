//! Water shader integration with bevy_water crate
//!
//! This module re-exports the bevy_water types.
//!
//! NOTE: The old mesh-based water rendering has been replaced with particle-based
//! fluid simulation. These re-exports are kept for the WaterPlugin which is still
//! used in the game setup, though the actual water rendering is now handled by
//! the fluid particle system.

// Re-export bevy_water types for use throughout the codebase
pub use bevy_water::{WaterPlugin, WaterSettings};

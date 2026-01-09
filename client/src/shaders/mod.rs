//! Custom shader system for Rustcraft
//!
//! This module provides water rendering integration using the bevy_water crate.

pub mod water;

// Re-export water types for convenience
pub use water::{WaterPlugin, WaterSettings};

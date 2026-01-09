//! Custom shader system for Rustcraft
//!
//! This module provides custom water rendering using our Gerstner wave shader.

pub mod water;

// Re-export water types for convenience
pub use water::{StandardWaterMaterial, WaterMaterial, WaterMesh, WaterPlugin, WaterTime};

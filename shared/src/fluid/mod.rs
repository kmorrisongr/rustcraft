//! Fluid simulation module using Salva3D.
//!
//! This module provides a fluid particle system to replace the water mesh rendering.
//! Water blocks indicate where fluid particles should spawn, and Salva handles the
//! physics simulation.

pub mod config;
pub mod plugin;
pub mod spawning;

pub use config::*;
pub use plugin::*;
pub use spawning::*;

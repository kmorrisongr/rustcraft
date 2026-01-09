//! Unified water system with shared CPU/GPU wave parameters.
//!
//! This module provides a single source of truth for wave configuration
//! that's used by both:
//! - CPU physics (buoyancy, swimming, boat dynamics)
//! - GPU rendering (vertex displacement, normals)
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    WaveConfig (shared)                       │
//! │  - Wave parameters (direction, steepness, wavelength, speed) │
//! │  - Serializable for network sync                             │
//! └─────────────────────────┬───────────────────────────────────┘
//!                           │
//!           ┌───────────────┴───────────────┐
//!           ▼                               ▼
//!   ┌───────────────┐               ┌───────────────┐
//!   │   CPU Physics │               │  GPU Shader   │
//!   │ (water_sim.rs)│               │ (water.wgsl)  │
//!   │               │               │               │
//!   │ - Buoyancy    │               │ - Vertex anim │
//!   │ - Swimming    │               │ - Normals     │
//!   │ - Boat physics│               │ - Reflections │
//!   └───────────────┘               └───────────────┘
//! ```
//!
//! ## Performance Features
//!
//! - **SIMD Optimization**: Wave calculations use SIMD where available
//! - **Spatial Hashing**: Fast water body lookups for physics queries
//! - **Batch Queries**: Sample multiple points efficiently for boats
//! - **LOD System**: Reduce wave complexity for distant objects

pub mod config;
pub mod physics;
pub mod simd;

pub use config::{WaveConfig, WavePreset};
pub use physics::WaterPhysicsWorld;

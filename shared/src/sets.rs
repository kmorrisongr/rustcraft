use bevy::prelude::*;

/// A unified system set for organizing game systems by their logical purpose.
/// This same enum is used across all schedules (Update, FixedUpdate, OnEnter, etc.)
/// with ordering configured per-schedule where needed.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameSet {
    /// System initialization, spawning core entities
    Initialize,
    /// Loading resources (textures, materials, etc.)
    Resources,
    /// Processing player input
    PlayerInput,
    /// Player physics and movement
    PlayerPhysics,
    /// World/entity input processing
    WorldInput,
    /// World physics simulation
    WorldPhysics,
    /// Network message sending/receiving
    Networking,
    /// Rendering and mesh updates
    Rendering,
    /// UI updates and rendering
    Ui,
    /// Cleanup and teardown
    Cleanup,
}

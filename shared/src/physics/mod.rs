pub mod movement;
pub mod rapier;

// Re-export Rapier integration
pub use movement::{
    rapier_movement_system, simulate_player_movement_rapier, RapierMovementController,
};
pub use rapier::*;

use bevy::prelude::*;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameSet {
    PlayerInput,
    PlayerPhysics,
    WorldInput,
    WorldPhysics,
    Networking,
    Rendering,
    Ui,
}

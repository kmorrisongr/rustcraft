use bevy::prelude::*;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameUpdateSet {
    PlayerInput,
    PlayerPhysics,
    WorldInput,
    WorldPhysics,
    Networking,
    Rendering,
    Ui,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameFixedPreUpdateSet {
    Networking,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameFixedUpdateSet {
    Networking,
}

use bevy::prelude::*;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameOnEnterSet {
    Initialize,
    Ui,
    Rest,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GamePreUpdateSet {
    PlayerInput,
    Rest,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameUpdateSet {
    PlayerInput,
    PlayerPhysics,
    WorldInput,
    WorldPhysics,
    Networking,
    Rendering,
    Ui,
    Rest,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameFixedPreUpdateSet {
    Networking,
    Rest,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameFixedUpdateSet {
    Networking,
    Rest,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GamePostUpdateSet {
    Rendering,
    Rest,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameOnExitSet {
    World,
    Networking,
    Rest,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum PreGameLoadingUpdateSet {
    Initialize,
    Networking,
    Rest,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum PreGameLoadingOnEnterSet {
    Initialize,
    Networking,
    Resources,
    Ui,
}

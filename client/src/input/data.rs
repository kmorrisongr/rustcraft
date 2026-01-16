use bevy::prelude::{Component, Reflect};
use leafwing_input_manager::Actionlike;
use serde::{Deserialize, Serialize};

#[derive(
    Actionlike,
    Eq,
    Hash,
    PartialEq,
    Component,
    Debug,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    PartialOrd,
    Ord,
    Reflect,
)]
pub enum GameAction {
    MoveForward,
    MoveBackward,
    MoveLeft,
    MoveRight,
    Jump,
    Escape,
    ToggleFps,
    ToggleViewMode,
    ToggleChunkDebugMode,
    ToggleFlyMode,
    FlyUp,
    FlyDown,
    ToggleBlockWireframeDebugMode,
    ToggleRaycastDebugMode,
    ToggleInventory,
    OpenChat,
    RenderDistanceMinus,
    RenderDistancePlus,
    ReloadChunks,
    ToggleFlashlight,
}

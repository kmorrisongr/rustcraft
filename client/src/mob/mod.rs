use bevy::prelude::*;
use shared::world::{MobId, MobKind};

mod fox;
mod spawn;

pub use fox::*;
use shared::sets::GameSets;
pub use spawn::*;

use crate::effects::{simulate_particles, ParticlePlugin};

/// Unified mob component. Replaces the previous MobRoot/MobMarker split.
#[derive(Debug, Component, Clone)]
pub struct Mob {
    pub kind: MobKind,
    pub id: MobId,
}

#[derive(Debug, Clone)]
pub struct TargetedMobData {
    pub kind: MobKind,
    pub id: MobId,
}

#[derive(Debug, Resource, Clone, Default)]
pub struct TargetedMob {
    pub target: Option<TargetedMobData>,
}

pub struct MobPlugin;

impl Plugin for MobPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ParticlePlugin)
            .init_resource::<FoxFeetTargets>()
            .init_resource::<TargetedMob>()
            .add_systems(
                Update,
                (spawn_mobs_system, setup_fox_once_loaded).in_set(GameSets::Update::WorldInput),
            )
            .add_systems(
                Update,
                (simulate_particles,).in_set(GameSets::Update::WorldPhysics),
            )
            .add_observer(observe_on_step);
    }
}

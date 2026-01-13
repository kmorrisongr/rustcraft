use bevy::prelude::*;

mod fox;
mod spawn;

pub use fox::*;
use shared::sets::GameUpdateSet;
pub use spawn::*;

#[derive(Debug, Component, Clone)]
pub struct MobRoot {
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub id: u128,
}

#[derive(Debug, Component, Clone)]
pub struct MobMarker {
    #[allow(dead_code)]
    pub name: String,
    pub id: u128,
}

#[derive(Debug, Clone)]
pub struct TargetedMobData {
    #[allow(dead_code)]
    pub name: String,
    pub id: u128,
    //pub entity: Entity,
}

#[derive(Debug, Resource, Clone, Default)]
pub struct TargetedMob {
    pub target: Option<TargetedMobData>,
}

pub struct MobPlugin;
impl Plugin for MobPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ParticleAssets>()
            .init_resource::<FoxFeetTargets>()
            .init_resource::<Animations>()
            .init_resource::<TargetedMob>()
            .add_systems(
                Update,
                (
                    spawn_mobs_system,
                    setup_fox_once_loaded,
                    update_targetted_mob_color,
                )
                    .in_set(GameUpdateSet::WorldInput),
            )
            .add_systems(
                Update,
                (simulate_particles,).in_set(GameUpdateSet::WorldPhysics),
            )
            .add_observer(observe_on_step);
    }
}

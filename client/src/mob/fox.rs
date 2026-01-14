//! Fox mob implementation with animations.

use std::time::Duration;

use bevy::{
    animation::{AnimationEvent, AnimationTargetId},
    prelude::*,
};
use rand::{rng, Rng};
use shared::world::MobKind;

use crate::effects::{spawn_particle, ParticleAssets};

use super::Mob;

const FOX_PATH: &str = "models/animated/Fox.glb";

/// Per-entity animation data. Each mob instance stores its own animation state.
#[derive(Component)]
pub struct MobAnimations {
    pub animations: Vec<AnimationNodeIndex>,
    pub graph: Handle<AnimationGraph>,
}

#[derive(AnimationEvent, Reflect, Clone)]
pub struct OnStep {
    pub target: Entity,
}

impl EntityEvent for OnStep {
    fn event_target(&self) -> Entity {
        self.target
    }

    fn event_target_mut(&mut self) -> &mut Entity {
        &mut self.target
    }
}

pub fn observe_on_step(
    trigger: On<OnStep>,
    particle: Res<ParticleAssets>,
    mut commands: Commands,
    transforms: Query<&GlobalTransform>,
) {
    let translation = transforms
        .get(trigger.event_target())
        .unwrap()
        .translation();
    let mut rng = rng();
    // Spawn a bunch of particles.
    for _ in 0..14 {
        let horizontal = rng.random::<Dir2>() * rng.random_range(8.0..12.0);
        let vertical = rng.random_range(0.0..4.0);
        let size = rng.random_range(0.2..1.0);
        commands.queue(spawn_particle(
            particle.mesh.clone(),
            particle.material.clone(),
            translation.reject_from_normalized(Vec3::Y),
            rng.random_range(0.2..0.6),
            size,
            Vec3::new(horizontal.x, vertical, horizontal.y) * 10.0,
        ));
    }
}

pub fn setup_fox(
    id: u128,
    spawn_pos: Vec3,
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    graphs: &mut ResMut<Assets<AnimationGraph>>,
) {
    // Build the animation graph
    let (graph, node_indices) = AnimationGraph::from_clips([
        asset_server.load(GltfAssetLabel::Animation(2).from_asset(FOX_PATH)), // Run
        asset_server.load(GltfAssetLabel::Animation(1).from_asset(FOX_PATH)), // Walk
        asset_server.load(GltfAssetLabel::Animation(0).from_asset(FOX_PATH)), // Survey
    ]);

    let graph_handle = graphs.add(graph);

    // Store animations on the entity itself, not as a global resource
    let mob_animations = MobAnimations {
        animations: node_indices,
        graph: graph_handle,
    };

    commands.spawn((
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(FOX_PATH))),
        Transform::from_translation(spawn_pos).with_scale(Vec3::splat(0.01)),
        Mob {
            kind: MobKind::Fox,
            id,
        },
        mob_animations,
    ));
}

/// Sets up animation player once the glTF scene is loaded.
/// Finds the parent entity with MobAnimations to configure transitions.
pub fn setup_fox_once_loaded(
    mut commands: Commands,
    feet: Res<FoxFeetTargets>,
    graphs: Res<Assets<AnimationGraph>>,
    mut clips: ResMut<Assets<AnimationClip>>,
    mut players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
    mob_query: Query<(&Mob, &MobAnimations)>,
    parents: Query<&ChildOf>,
) {
    fn get_clip<'a>(
        node: AnimationNodeIndex,
        graph: &AnimationGraph,
        clips: &'a mut Assets<AnimationClip>,
    ) -> &'a mut AnimationClip {
        let node = graph.get(node).unwrap();
        let clip = match &node.node_type {
            AnimationNodeType::Clip(handle) => clips.get_mut(handle),
            _ => unreachable!(),
        };
        clip.unwrap()
    }

    for (entity, mut player) in &mut players {
        // Walk up the hierarchy to find the mob entity with animations
        let mut current = entity;
        let mob_animations = loop {
            if let Ok((mob, anims)) = mob_query.get(current) {
                if mob.kind == MobKind::Fox {
                    break Some(anims);
                }
            }
            if let Ok(child_of) = parents.get(current) {
                current = child_of.parent();
            } else {
                break None;
            }
        };

        let Some(animations) = mob_animations else {
            continue;
        };

        let Some(graph) = graphs.get(&animations.graph) else {
            continue;
        };

        // Add step events to running animation for particle effects
        let running_animation = get_clip(animations.animations[0], graph, &mut clips);
        running_animation.add_event_to_target(feet.front_left, 0.625, OnStep { target: entity });
        running_animation.add_event_to_target(feet.front_right, 0.5, OnStep { target: entity });
        running_animation.add_event_to_target(feet.back_left, 0.0, OnStep { target: entity });
        running_animation.add_event_to_target(feet.back_right, 0.125, OnStep { target: entity });

        let mut transitions = AnimationTransitions::new();
        transitions
            .play(&mut player, animations.animations[0], Duration::ZERO)
            .repeat();

        commands
            .entity(entity)
            .insert(AnimationGraphHandle(animations.graph.clone()))
            .insert(transitions);
    }
}

#[derive(Resource)]
pub struct FoxFeetTargets {
    front_right: AnimationTargetId,
    front_left: AnimationTargetId,
    back_left: AnimationTargetId,
    back_right: AnimationTargetId,
}

impl Default for FoxFeetTargets {
    fn default() -> Self {
        let hip_node = ["root", "_rootJoint", "b_Root_00", "b_Hip_01"];
        let front_left_foot = hip_node.iter().chain(
            [
                "b_Spine01_02",
                "b_Spine02_03",
                "b_LeftUpperArm_09",
                "b_LeftForeArm_010",
                "b_LeftHand_011",
            ]
            .iter(),
        );
        let front_right_foot = hip_node.iter().chain(
            [
                "b_Spine01_02",
                "b_Spine02_03",
                "b_RightUpperArm_06",
                "b_RightForeArm_07",
                "b_RightHand_08",
            ]
            .iter(),
        );
        let back_left_foot = hip_node.iter().chain(
            [
                "b_LeftLeg01_015",
                "b_LeftLeg02_016",
                "b_LeftFoot01_017",
                "b_LeftFoot02_018",
            ]
            .iter(),
        );
        let back_right_foot = hip_node.iter().chain(
            [
                "b_RightLeg01_019",
                "b_RightLeg02_020",
                "b_RightFoot01_021",
                "b_RightFoot02_022",
            ]
            .iter(),
        );
        Self {
            front_left: AnimationTargetId::from_iter(front_left_foot),
            front_right: AnimationTargetId::from_iter(front_right_foot),
            back_left: AnimationTargetId::from_iter(back_left_foot),
            back_right: AnimationTargetId::from_iter(back_right_foot),
        }
    }
}

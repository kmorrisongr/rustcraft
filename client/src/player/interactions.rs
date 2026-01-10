use crate::mob::{MobMarker, TargetedMob, TargetedMobData};
use crate::network::buffered_client::CurrentFrameInputs;
use crate::ui::hud::UIMode;
use crate::world::{ClientWorldMap, WorldRenderRequestUpdateEvent};
use bevy::color::palettes::css::WHITE;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use shared::messages::NetworkAction;
use shared::players::blocks::{simulate_player_block_interactions, CallerType};
use shared::players::{Player, ViewMode};
use shared::world::{global_to_chunk_local, raycast};

use super::CurrentPlayerMarker;

#[derive(SystemParam)]
pub struct PlayerInteractionQueries<'w, 's> {
    player_query: Query<'w, 's, &'static mut Player, With<CurrentPlayerMarker>>,
    p_transform: Query<'w, 's, &'static mut Transform, With<CurrentPlayerMarker>>,
    camera_query: Query<'w, 's, &'static Transform, (With<Camera>, Without<CurrentPlayerMarker>)>,
    mob_query: Query<'w, 's, &'static MobMarker>,
}

#[derive(SystemParam)]
pub struct PlayerInteractionResources<'w> {
    world_map: ResMut<'w, ClientWorldMap>,
    mouse_input: Res<'w, ButtonInput<MouseButton>>,
    ui_mode: Res<'w, UIMode>,
    view_mode: Res<'w, ViewMode>,
    targeted_mob: ResMut<'w, TargetedMob>,
    frame_inputs: ResMut<'w, CurrentFrameInputs>,
    ev_render: EventWriter<'w, WorldRenderRequestUpdateEvent>,
}

// Function to handle block placement and breaking
pub fn handle_block_interactions(
    queries: PlayerInteractionQueries,
    resources: PlayerInteractionResources,
    mut ray_cast: MeshRayCast,
    mut gizmos: Gizmos,
) {
    let PlayerInteractionQueries {
        mut player_query,
        p_transform,
        camera_query,
        mob_query,
    } = queries;
    let PlayerInteractionResources {
        world_map,
        mouse_input,
        ui_mode,
        view_mode,
        mut targeted_mob,
        mut frame_inputs,
        mut ev_render,
    } = resources;

    let mut player = player_query.single_mut().unwrap();

    if *ui_mode == UIMode::Opened {
        return;
    }

    let camera_transform = camera_query.single().unwrap();
    let player_transform = p_transform.single().unwrap();
    let player_translation = &player_transform.translation;

    let ray = Ray3d::new(camera_transform.translation, camera_transform.forward());

    let world_map = world_map.into_inner();

    let maybe_block = raycast::raycast(world_map, camera_transform, player_translation, *view_mode);

    if let Some((entity, _)) = ray_cast
        .cast_ray(ray, &MeshRayCastSettings::default())
        .first()
    {
        let mob = mob_query.get(*entity);
        if let Ok(mob) = mob {
            targeted_mob.target = Some(TargetedMobData {
                // entity: *entity,
                id: mob.id,
                name: mob.name.clone(),
            });
        } else {
            targeted_mob.target = None;
        }
    } else {
        targeted_mob.target = None;
    }

    if mouse_input.just_pressed(MouseButton::Left) && targeted_mob.target.is_some() {
        // TODO: Attack the targeted

        targeted_mob.target = None;

        return;
    }

    if let Some(res) = maybe_block {
        // Draw gizmos for the bounding box
        let center = (res.bbox.max + res.bbox.min) / 2.0;
        let hsize = res.bbox.max - res.bbox.min;
        gizmos.cuboid(
            Transform::from_translation(center.into()).with_scale(hsize.into()),
            WHITE,
        );

        // Handle left-click for breaking blocks
        if mouse_input.pressed(MouseButton::Left) {
            frame_inputs.0.inputs.insert(NetworkAction::LeftClick);
        }

        // Handle right-click for placing blocks
        if mouse_input.pressed(MouseButton::Right) {
            frame_inputs.0.inputs.insert(NetworkAction::RightClick);
        }

        simulate_player_block_interactions(
            &mut player,
            world_map,
            &frame_inputs.0,
            CallerType::Client,
        );

        // Trigger chunk re-render if breaking/placing blocks
        // This ensures the breaking animation is visible
        if frame_inputs.0.inputs.contains(&NetworkAction::LeftClick)
            || frame_inputs.0.inputs.contains(&NetworkAction::RightClick)
        {
            let (chunk_pos, _) = global_to_chunk_local(&res.position);
            ev_render.write(WorldRenderRequestUpdateEvent::ChunkToReload(chunk_pos));
        }
    }
}

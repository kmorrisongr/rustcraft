pub mod background_generation;
pub mod broadcast_world;
pub(crate) mod data;
pub mod generation;
pub mod load_from_file;
pub mod save;
pub mod simulation;
pub mod stacks;
pub mod terrain_mutation;
pub mod water_boundary;
pub mod water_flow;
pub mod water_simulation;
pub mod water_sleep;

use bevy::prelude::Event;
use bevy::prelude::EventReader;
use bevy::prelude::IVec3;
use bevy::prelude::ResMut;
use bevy::prelude::*;
use shared::world::{BlockData, ItemStack, ServerItemStack, ServerWorldMap, WorldMap};
use ulid::Ulid;
use water_flow::LateralFlowQueue;
use water_simulation::{WaterSimulationQueue, WaterSurfaceUpdateQueue};

#[derive(Event, Debug)]
pub struct BlockInteractionEvent {
    pub position: IVec3,
    pub block_type: Option<BlockData>, // None = delete, Some = add
}

pub fn handle_block_interactions(
    mut world_map: ResMut<ServerWorldMap>,
    mut events: EventReader<BlockInteractionEvent>,
    mut water_queue: ResMut<WaterSimulationQueue>,
    mut surface_queue: ResMut<WaterSurfaceUpdateQueue>,
    mut lateral_queue: ResMut<LateralFlowQueue>,
) {
    for event in events.read() {
        match &event.block_type {
            Some(block) => {
                // Block placement - trigger water displacement using the new terrain mutation system
                let result = terrain_mutation::handle_block_placement(
                    &mut world_map,
                    event.position,
                    &mut water_queue,
                    &mut surface_queue,
                    &mut lateral_queue,
                );

                if result.displaced > 0.0 {
                    debug!(
                        "Block placement at {:?}: displaced {:.3} water, overflow {:.3}",
                        event.position, result.displaced, result.overflow
                    );
                }

                world_map.chunks.set_block(&event.position, *block);
                debug!("Block added at {:?}: {:?}", event.position, block);
            }
            None => {
                debug!("Getting block by coordinates at {:?}", event.position);
                for (id, nb) in world_map
                    .chunks
                    .get_block_by_coordinates(&event.position)
                    .unwrap()
                    .id
                    .get_drops(1)
                {
                    world_map.item_stacks.push(ServerItemStack {
                        id: Ulid::new().0,
                        despawned: false,
                        stack: ItemStack {
                            item_id: id,
                            item_type: id.get_default_type(),
                            nb,
                        },
                        pos: Vec3::new(
                            event.position.x as f32,
                            event.position.y as f32,
                            event.position.z as f32,
                        ),
                        timestamp: 0,
                    });
                }

                world_map
                    .chunks
                    .remove_block_by_coordinates(&event.position);

                // Trigger water flow check after block removal using the new terrain mutation system
                terrain_mutation::handle_block_removal(
                    &mut world_map,
                    event.position,
                    &mut water_queue,
                    &mut surface_queue,
                    &mut lateral_queue,
                );

                info!("Block removed at {:?}", event.position);
            }
        }
    }
}

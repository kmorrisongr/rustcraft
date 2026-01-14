use bevy::{prelude::*, render::mesh::VertexAttributeValues};
use shared::{
    messages::ItemStackUpdateEvent,
    world::{ItemStack, ItemType},
    CHUNK_SIZE,
};

use crate::{
    player::CurrentPlayerMarker,
    world::{MaterialResource, RenderDistance, TextureAtlases},
};

/// Size of a dropped block item in the world
const BLOCK_STACK_SIZE: Vec3 = Vec3::new(0.2, 0.2, 0.2);
/// Size of a dropped non-block item in the world (flatter, like a card)
const ITEM_STACK_SIZE: Vec3 = Vec3::new(0.2, 0.2, 0.05);
/// Rotation speed for dropped items (radians per second)
const STACK_ROTATION_SPEED: f32 = 1.0;

#[derive(Debug, Component)]
pub struct StackMarker {
    pub id: u128,
    pub stack: ItemStack,
}

/// Creates a textured mesh for a dropped item stack
fn create_stack_mesh(stack: &ItemStack, texture_atlases: &TextureAtlases) -> Mesh {
    let size = match stack.item_type {
        ItemType::Block(_) => BLOCK_STACK_SIZE,
        _ => ITEM_STACK_SIZE,
    };

    let mut mesh = Cuboid::from_size(size).mesh().build();

    let uv_attribute = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0).unwrap();
    let VertexAttributeValues::Float32x2(uv_attribute) = uv_attribute else {
        panic!("Unexpected vertex format, expected Float32x2.");
    };

    if let Some(uv_coords) = texture_atlases
        .items
        .uvs
        .get(&format!("{:?}", stack.item_id))
    {
        for uv in uv_attribute.iter_mut() {
            uv[0] = uv[0].clamp(uv_coords.u0, uv_coords.u1);
            uv[1] = uv[1].clamp(uv_coords.v0, uv_coords.v1);
        }
    }

    mesh
}

pub fn stack_update_system(
    mut events: EventReader<ItemStackUpdateEvent>,
    mut commands: Commands,
    mut stacks: Query<(Entity, &mut StackMarker, &mut Transform), Without<CurrentPlayerMarker>>,
    mut meshes: ResMut<Assets<Mesh>>,
    time: Res<Time>,
    material_resource: Res<MaterialResource>,
    texture_atlases: Option<Res<TextureAtlases>>,
    distance: Res<RenderDistance>,
    player_pos: Query<&Transform, With<CurrentPlayerMarker>>,
) {
    let Some(texture_atlases) = texture_atlases else {
        return;
    };

    for ev in events.read() {
        match ev.data {
            Some((stack, pos)) => {
                // Try to update an existing stack with this ID
                if let Some((_, mut marker, mut transform)) =
                    stacks.iter_mut().find(|(_, m, _)| m.id == ev.id)
                {
                    transform.translation = pos;
                    marker.stack = stack;
                    continue;
                }

                // No existing stack found - spawn a new one
                let mesh = create_stack_mesh(&stack, &texture_atlases);
                commands.spawn((
                    StackMarker { id: ev.id, stack },
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(
                        material_resource
                            .global_materials
                            .get(&crate::world::GlobalMaterial::Items)
                            .unwrap()
                            .clone_weak(),
                    ),
                    Transform::from_translation(pos),
                ));
            }
            None => {
                // Despawn the stack with this ID
                if let Some((entity, _, _)) = stacks.iter().find(|(_, m, _)| m.id == ev.id) {
                    commands.entity(entity).despawn();
                }
            }
        }
    }

    // Rotate visible stacks and despawn those outside render distance
    let Ok(player_transform) = player_pos.single() else {
        return;
    };
    let max_distance = distance.distance as f32 * CHUNK_SIZE as f32;

    for (entity, _, mut transform) in stacks.iter_mut() {
        if player_transform.translation.distance(transform.translation) > max_distance {
            commands.entity(entity).despawn();
        } else {
            transform.rotate_local_y(STACK_ROTATION_SPEED * time.delta_secs());
        }
    }
}

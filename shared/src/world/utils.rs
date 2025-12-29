use bevy::math::{IVec3, Vec3};

use crate::CHUNK_SIZE;

pub fn block_to_chunk_coord(x: i32) -> i32 {
    if x >= 0 {
        x / CHUNK_SIZE
    } else {
        (x - (CHUNK_SIZE - 1)) / CHUNK_SIZE
    }
}

pub fn block_vec3_to_chunk_v3_coord(v: Vec3) -> Vec3 {
    Vec3::new(
        block_to_chunk_coord(v.x as i32) as f32,
        block_to_chunk_coord(v.y as i32) as f32,
        block_to_chunk_coord(v.z as i32) as f32,
    )
}

pub fn world_position_to_chunk_position(v: Vec3) -> IVec3 {
    IVec3::new(
        block_to_chunk_coord(v.x as i32),
        block_to_chunk_coord(v.y as i32),
        block_to_chunk_coord(v.z as i32),
    )
}

pub fn to_global_pos(chunk_pos: &IVec3, local_block_pos: &IVec3) -> IVec3 {
    *chunk_pos * CHUNK_SIZE + *local_block_pos
}

pub fn to_local_pos(global_block_pos: &IVec3) -> IVec3 {
    IVec3 {
        x: ((global_block_pos.x % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE,
        y: ((global_block_pos.y % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE,
        z: ((global_block_pos.z % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE,
    }
}

pub fn global_block_to_chunk_pos(global_block_pos: &IVec3) -> IVec3 {
    IVec3::new(
        block_to_chunk_coord(global_block_pos.x),
        block_to_chunk_coord(global_block_pos.y),
        block_to_chunk_coord(global_block_pos.z),
    )
}

/// Converts a global block position to its containing chunk position and the
/// block's local coordinates inside that chunk.
/// Returns `(chunk_pos, local_pos)`.
pub fn global_to_chunk_local(position: &IVec3) -> (IVec3, IVec3) {
    let chunk_pos = global_block_to_chunk_pos(position);
    let local_pos = to_local_pos(position);
    (chunk_pos, local_pos)
}

pub const SIX_OFFSETS: [IVec3; 6] = [
    IVec3::new(1, 0, 0),
    IVec3::new(-1, 0, 0),
    IVec3::new(0, 1, 0),
    IVec3::new(0, -1, 0),
    IVec3::new(0, 0, 1),
    IVec3::new(0, 0, -1),
];

pub fn chunk_in_radius(player_pos: &IVec3, chunk_pos: &IVec3, radius: i32) -> bool {
    (player_pos.x - chunk_pos.x).abs() <= radius && (player_pos.z - chunk_pos.z).abs() <= radius
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_to_chunk_local_handles_positive_and_negative_coords() {
        let position = IVec3::new(16, 0, -1);
        let (chunk_pos, local_pos) = global_to_chunk_local(&position);

        assert_eq!(chunk_pos, IVec3::new(1, 0, -1));
        assert_eq!(local_pos, IVec3::new(0, 0, 15));
    }
}

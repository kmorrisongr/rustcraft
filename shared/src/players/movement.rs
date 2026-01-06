use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    physics::{apply_gravity, resolve_vertical_movement, try_move, PhysicsBody},
    players::constants::{FLY_SPEED_MULTIPLIER, GRAVITY, JUMP_VELOCITY, SPEED},
    world::{world_position_to_chunk_position, WorldMap},
};
use bevy::prelude::*;
use bevy_log::warn;

use super::Player;

/// Recompute gravity_enabled based on whether required chunks are loaded.
fn compute_gravity_enabled(player: &Player, world_map: &impl WorldMap) -> bool {
    let current_chunk = world_position_to_chunk_position(player.position);
    let chunk_below = current_chunk - IVec3::Y;
    let chunk_above = current_chunk + IVec3::Y;
    world_map.has_chunk(&current_chunk)
        && world_map.has_chunk(&chunk_below)
        && world_map.has_chunk(&chunk_above)
}

/// Check if gravity state needs to be updated and update it if so.
/// Only runs the chunk lookup when:
/// - Player entered a new chunk, OR
/// - Gravity is currently disabled (waiting for chunks to load)
fn maybe_update_gravity_state(player: &mut Player, world_map: &impl WorldMap) {
    let current_chunk = world_position_to_chunk_position(player.position);
    let chunk_changed = player.last_gravity_check_chunk != Some(current_chunk);

    if chunk_changed {
        player.last_gravity_check_chunk = Some(current_chunk);
        player.gravity_enabled = compute_gravity_enabled(player, world_map);
    } else if !player.gravity_enabled {
        // Keep checking while gravity is disabled (chunks may have loaded)
        player.gravity_enabled = compute_gravity_enabled(player, world_map);
    }
    // If gravity is enabled and chunk hasn't changed, do nothing
}

pub fn simulate_player_movement(
    player: &mut Player,
    world_map: &impl WorldMap,
    action: &PlayerFrameInput,
) {
    // let's check if the 9 chunks around the player are loaded
    let chunks = world_map.get_surrounding_chunks(player.position, 1);
    if chunks.len() < 9 {
        log::debug!("Not enough chunks loaded, skipping movement simulation");
        return;
    }

    let delta = action.delta_ms as f32 / 1000.0;

    let mut direction = Vec3::ZERO;

    if action.is_pressed(NetworkAction::ToggleFlyMode) {
        player.is_flying = !player.is_flying;
    }

    player.camera_transform = action.camera;

    let is_jumping = action.is_pressed(NetworkAction::JumpOrFlyUp);

    // Calculate movement directions relative to the camera
    let forward = player
        .camera_transform
        .forward()
        .xyz()
        .with_y(0.0)
        .normalize();

    let right = player
        .camera_transform
        .right()
        .xyz()
        .with_y(0.0)
        .normalize();

    // Adjust direction based on key presses
    if action.is_pressed(NetworkAction::MoveBackward) {
        direction -= forward;
    }
    if action.is_pressed(NetworkAction::MoveForward) {
        direction += forward;
    }
    if action.is_pressed(NetworkAction::MoveLeft) {
        direction -= right;
    }
    if action.is_pressed(NetworkAction::MoveRight) {
        direction += right;
    }

    // Normalize direction to prevent faster movement with diagonals
    if direction != Vec3::ZERO {
        direction = direction.normalize();
    }

    if action.is_pressed(NetworkAction::JumpOrFlyUp) {
        direction += Vec3::Y;
    }
    if action.is_pressed(NetworkAction::SneakOrFlyDown) {
        direction -= Vec3::Y;
    }

    let mut body = PhysicsBody::new(
        player.position,
        player.velocity,
        player.on_ground,
        Vec3::new(player.width, player.height, player.width),
    );

    // Update gravity state on chunk enter, or keep checking while disabled
    maybe_update_gravity_state(player, world_map);

    // Handle jumping (if on the ground) and gravity, only if not flying
    if !player.is_flying {
        if body.on_ground && is_jumping {
            // Player can jump only when grounded
            body.velocity.y = JUMP_VELOCITY * delta;
            body.on_ground = false;
        } else if player.gravity_enabled {
            apply_gravity(&mut body, GRAVITY, delta);
        }
    } else {
        body.velocity.y = 0.0;
        body.on_ground = false;
    }

    let max_velocity = 0.9;

    resolve_vertical_movement(&mut body, world_map, max_velocity, !player.is_flying);

    let speed = if player.is_flying {
        SPEED * FLY_SPEED_MULTIPLIER
    } else {
        SPEED
    };
    let speed = speed * delta;
    let displacement = Vec3::new(direction.x * speed, 0.0, direction.z * speed);

    if player.is_flying {
        try_move(
            &mut body,
            world_map,
            displacement + Vec3::new(0.0, direction.y * speed, 0.0),
            false,
        );
    } else {
        try_move(
            &mut body,
            world_map,
            Vec3::new(displacement.x, 0.0, 0.0),
            true,
        );
        try_move(
            &mut body,
            world_map,
            Vec3::new(0.0, 0.0, displacement.z),
            true,
        );
    }

    // Safety net: prevent players from falling indefinitely if they slip through the world.
    const FALL_RESET_Y: f32 = -100.0;
    if body.position.y < FALL_RESET_Y {
        warn!(
            "Player {:?} fell below safety threshold (y = {}). Resetting position.",
            player.id,
            body.position.y
        );
        body.position.y = 0.0;
        body.velocity = Vec3::ZERO;
    }
    player.position = body.position;
    player.velocity = body.velocity;
    player.on_ground = body.on_ground;
}

trait IsPressed {
    fn is_pressed(&self, action: NetworkAction) -> bool;
}

impl IsPressed for PlayerFrameInput {
    fn is_pressed(&self, action: NetworkAction) -> bool {
        self.inputs.contains(&action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::BlockData;
    use std::collections::HashSet;

    /// Mock WorldMap implementation for testing
    struct MockWorldMap {
        loaded_chunks: HashSet<IVec3>,
    }

    impl MockWorldMap {
        fn new() -> Self {
            Self {
                loaded_chunks: HashSet::new(),
            }
        }

        fn with_chunks(chunks: Vec<IVec3>) -> Self {
            Self {
                loaded_chunks: chunks.into_iter().collect(),
            }
        }
    }

    impl WorldMap for MockWorldMap {
        fn get_block_mut_by_coordinates(&mut self, _position: &IVec3) -> Option<&mut BlockData> {
            None
        }

        fn get_block_by_coordinates(&self, _position: &IVec3) -> Option<&BlockData> {
            None
        }

        fn remove_block_by_coordinates(&mut self, _global_block_pos: &IVec3) -> Option<BlockData> {
            None
        }

        fn set_block(&mut self, _position: &IVec3, _block: BlockData) {}

        fn has_chunk(&self, chunk_pos: &IVec3) -> bool {
            self.loaded_chunks.contains(chunk_pos)
        }

        fn mark_block_for_update(&mut self, _position: &IVec3) {}
    }

    /// Helper function to create a test player at a given position
    fn create_test_player(position: Vec3) -> Player {
        Player {
            id: 1,
            name: "TestPlayer".to_string(),
            position,
            camera_transform: Transform::default(),
            velocity: Vec3::ZERO,
            on_ground: false,
            is_flying: false,
            inventory: crate::players::Inventory::new(),
            height: 1.8,
            width: 0.8,
            last_input_processed: 0,
            gravity_enabled: false,
            last_gravity_check_chunk: None,
        }
    }

    #[test]
    fn compute_gravity_enabled_all_chunks_loaded() {
        let player = create_test_player(Vec3::new(8.0, 8.0, 8.0));
        let current_chunk = IVec3::new(0, 0, 0);
        let chunk_below = IVec3::new(0, -1, 0);
        let chunk_above = IVec3::new(0, 1, 0);
        
        let world_map = MockWorldMap::with_chunks(vec![current_chunk, chunk_below, chunk_above]);
        
        assert!(compute_gravity_enabled(&player, &world_map));
    }

    #[test]
    fn compute_gravity_enabled_missing_current_chunk() {
        let player = create_test_player(Vec3::new(8.0, 8.0, 8.0));
        let chunk_below = IVec3::new(0, -1, 0);
        let chunk_above = IVec3::new(0, 1, 0);
        
        let world_map = MockWorldMap::with_chunks(vec![chunk_below, chunk_above]);
        
        assert!(!compute_gravity_enabled(&player, &world_map));
    }

    #[test]
    fn compute_gravity_enabled_missing_chunk_below() {
        let player = create_test_player(Vec3::new(8.0, 8.0, 8.0));
        let current_chunk = IVec3::new(0, 0, 0);
        let chunk_above = IVec3::new(0, 1, 0);
        
        let world_map = MockWorldMap::with_chunks(vec![current_chunk, chunk_above]);
        
        assert!(!compute_gravity_enabled(&player, &world_map));
    }

    #[test]
    fn compute_gravity_enabled_missing_chunk_above() {
        let player = create_test_player(Vec3::new(8.0, 8.0, 8.0));
        let current_chunk = IVec3::new(0, 0, 0);
        let chunk_below = IVec3::new(0, -1, 0);
        
        let world_map = MockWorldMap::with_chunks(vec![current_chunk, chunk_below]);
        
        assert!(!compute_gravity_enabled(&player, &world_map));
    }

    #[test]
    fn compute_gravity_enabled_at_chunk_boundary_positive() {
        // Player at x=16, which is the boundary between chunk 0 and chunk 1
        let player = create_test_player(Vec3::new(16.0, 8.0, 8.0));
        let current_chunk = IVec3::new(1, 0, 0);
        let chunk_below = IVec3::new(1, -1, 0);
        let chunk_above = IVec3::new(1, 1, 0);
        
        let world_map = MockWorldMap::with_chunks(vec![current_chunk, chunk_below, chunk_above]);
        
        assert!(compute_gravity_enabled(&player, &world_map));
    }

    #[test]
    fn compute_gravity_enabled_at_chunk_boundary_negative() {
        // Player at x=-1, which is in chunk -1
        let player = create_test_player(Vec3::new(-1.0, 8.0, 8.0));
        let current_chunk = IVec3::new(-1, 0, 0);
        let chunk_below = IVec3::new(-1, -1, 0);
        let chunk_above = IVec3::new(-1, 1, 0);
        
        let world_map = MockWorldMap::with_chunks(vec![current_chunk, chunk_below, chunk_above]);
        
        assert!(compute_gravity_enabled(&player, &world_map));
    }

    #[test]
    fn compute_gravity_enabled_at_negative_y() {
        // Player at negative Y coordinate
        let player = create_test_player(Vec3::new(8.0, -24.0, 8.0));
        let current_chunk = IVec3::new(0, -2, 0);
        let chunk_below = IVec3::new(0, -3, 0);
        let chunk_above = IVec3::new(0, -1, 0);
        
        let world_map = MockWorldMap::with_chunks(vec![current_chunk, chunk_below, chunk_above]);
        
        assert!(compute_gravity_enabled(&player, &world_map));
    }

    #[test]
    fn compute_gravity_enabled_at_negative_coords_missing_chunk() {
        // Player at negative coordinates with missing chunk below
        let player = create_test_player(Vec3::new(-8.0, -24.0, -8.0));
        let current_chunk = IVec3::new(-1, -2, -1);
        let chunk_above = IVec3::new(-1, -1, -1);
        
        let world_map = MockWorldMap::with_chunks(vec![current_chunk, chunk_above]);
        
        assert!(!compute_gravity_enabled(&player, &world_map));
    }

    #[test]
    fn maybe_update_gravity_state_chunk_changed_chunks_loaded() {
        let mut player = create_test_player(Vec3::new(8.0, 8.0, 8.0));
        player.last_gravity_check_chunk = Some(IVec3::new(-1, 0, 0)); // Different chunk
        player.gravity_enabled = false;
        
        let current_chunk = IVec3::new(0, 0, 0);
        let chunk_below = IVec3::new(0, -1, 0);
        let chunk_above = IVec3::new(0, 1, 0);
        let world_map = MockWorldMap::with_chunks(vec![current_chunk, chunk_below, chunk_above]);
        
        maybe_update_gravity_state(&mut player, &world_map);
        
        assert!(player.gravity_enabled);
        assert_eq!(player.last_gravity_check_chunk, Some(current_chunk));
    }

    #[test]
    fn maybe_update_gravity_state_chunk_changed_chunks_not_loaded() {
        let mut player = create_test_player(Vec3::new(8.0, 8.0, 8.0));
        player.last_gravity_check_chunk = Some(IVec3::new(-1, 0, 0)); // Different chunk
        player.gravity_enabled = true; // Was enabled before
        
        let world_map = MockWorldMap::new(); // No chunks loaded
        
        maybe_update_gravity_state(&mut player, &world_map);
        
        assert!(!player.gravity_enabled);
        assert_eq!(player.last_gravity_check_chunk, Some(IVec3::new(0, 0, 0)));
    }

    #[test]
    fn maybe_update_gravity_state_same_chunk_gravity_enabled() {
        let mut player = create_test_player(Vec3::new(8.0, 8.0, 8.0));
        let current_chunk = IVec3::new(0, 0, 0);
        player.last_gravity_check_chunk = Some(current_chunk);
        player.gravity_enabled = true;
        
        let world_map = MockWorldMap::new(); // Even with no chunks, shouldn't check
        
        maybe_update_gravity_state(&mut player, &world_map);
        
        // Should remain enabled, no check performed
        assert!(player.gravity_enabled);
        assert_eq!(player.last_gravity_check_chunk, Some(current_chunk));
    }

    #[test]
    fn maybe_update_gravity_state_same_chunk_gravity_disabled_waiting_for_chunks() {
        let mut player = create_test_player(Vec3::new(8.0, 8.0, 8.0));
        let current_chunk = IVec3::new(0, 0, 0);
        player.last_gravity_check_chunk = Some(current_chunk);
        player.gravity_enabled = false;
        
        // First check: chunks still not loaded
        let world_map = MockWorldMap::new();
        maybe_update_gravity_state(&mut player, &world_map);
        assert!(!player.gravity_enabled);
        
        // Second check: chunks now loaded
        let chunk_below = IVec3::new(0, -1, 0);
        let chunk_above = IVec3::new(0, 1, 0);
        let world_map = MockWorldMap::with_chunks(vec![current_chunk, chunk_below, chunk_above]);
        maybe_update_gravity_state(&mut player, &world_map);
        assert!(player.gravity_enabled);
    }

    #[test]
    fn maybe_update_gravity_state_first_check_no_previous_chunk() {
        let mut player = create_test_player(Vec3::new(8.0, 8.0, 8.0));
        player.last_gravity_check_chunk = None; // First check
        player.gravity_enabled = false;
        
        let current_chunk = IVec3::new(0, 0, 0);
        let chunk_below = IVec3::new(0, -1, 0);
        let chunk_above = IVec3::new(0, 1, 0);
        let world_map = MockWorldMap::with_chunks(vec![current_chunk, chunk_below, chunk_above]);
        
        maybe_update_gravity_state(&mut player, &world_map);
        
        assert!(player.gravity_enabled);
        assert_eq!(player.last_gravity_check_chunk, Some(current_chunk));
    }

    #[test]
    fn maybe_update_gravity_state_player_moving_between_chunks() {
        let mut player = create_test_player(Vec3::new(8.0, 8.0, 8.0));
        player.last_gravity_check_chunk = Some(IVec3::new(0, 0, 0));
        player.gravity_enabled = true;
        
        // Move player to adjacent chunk
        player.position = Vec3::new(24.0, 8.0, 8.0); // Chunk (1, 0, 0)
        let new_chunk = IVec3::new(1, 0, 0);
        let chunk_below = IVec3::new(1, -1, 0);
        let chunk_above = IVec3::new(1, 1, 0);
        let world_map = MockWorldMap::with_chunks(vec![new_chunk, chunk_below, chunk_above]);
        
        maybe_update_gravity_state(&mut player, &world_map);
        
        assert!(player.gravity_enabled);
        assert_eq!(player.last_gravity_check_chunk, Some(new_chunk));
    }

    #[test]
    fn maybe_update_gravity_state_chunks_unload_while_gravity_disabled() {
        let mut player = create_test_player(Vec3::new(8.0, 8.0, 8.0));
        let current_chunk = IVec3::new(0, 0, 0);
        player.last_gravity_check_chunk = Some(current_chunk);
        player.gravity_enabled = false;
        
        // Chunks remain unloaded, gravity should stay disabled
        let world_map = MockWorldMap::new();
        
        for _ in 0..5 {
            maybe_update_gravity_state(&mut player, &world_map);
            assert!(!player.gravity_enabled);
            assert_eq!(player.last_gravity_check_chunk, Some(current_chunk));
        }
    }

    #[test]
    fn compute_gravity_enabled_chunk_boundary_edge_cases() {
        // Test at exact chunk boundary on multiple axes
        let chunk_size = crate::CHUNK_SIZE as f32;
        let test_cases = vec![
            (Vec3::new(0.0, 0.0, 0.0), IVec3::new(0, 0, 0)),
            (Vec3::new(chunk_size, 0.0, 0.0), IVec3::new(1, 0, 0)),
            (Vec3::new(-chunk_size, 0.0, 0.0), IVec3::new(-1, 0, 0)),
            (Vec3::new(0.0, chunk_size, 0.0), IVec3::new(0, 1, 0)),
            (Vec3::new(0.0, -chunk_size, 0.0), IVec3::new(0, -1, 0)),
        ];
        
        for (pos, expected_chunk) in test_cases {
            let player = create_test_player(pos);
            let chunk_below = expected_chunk - IVec3::Y;
            let chunk_above = expected_chunk + IVec3::Y;
            let world_map = MockWorldMap::with_chunks(vec![expected_chunk, chunk_below, chunk_above]);
            
            assert!(
                compute_gravity_enabled(&player, &world_map),
                "Failed for position {:?} in chunk {:?}",
                pos,
                expected_chunk
            );
        }
    }
}

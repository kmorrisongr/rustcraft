//! Rapier physics integration for Rustcraft.
//!
//! This module provides Rapier-based physics for players and mobs,
//! integrating with the voxel world collision system.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

/// Physics constants used throughout the game.
pub mod constants {
    /// Gravity acceleration (m/s²) - negative for downward
    pub const GRAVITY: f32 = -20.0;
    /// Maximum fall speed (terminal velocity)
    pub const TERMINAL_VELOCITY: f32 = 50.0;
    /// Jump impulse velocity
    pub const JUMP_VELOCITY: f32 = 8.0;
    /// Default player movement speed
    pub const PLAYER_SPEED: f32 = 5.0;
    /// Fly mode speed multiplier
    pub const FLY_SPEED_MULTIPLIER: f32 = 4.0;
}

/// Component marking an entity as using Rustcraft physics.
/// This is used to identify entities that should be processed by our physics systems.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct RustcraftPhysicsBody {
    /// Whether gravity should be applied to this entity
    pub gravity_enabled: bool,
    /// Whether this entity is currently flying (bypasses gravity)
    pub is_flying: bool,
    /// Whether this entity is on the ground
    pub on_ground: bool,
}

impl RustcraftPhysicsBody {
    pub fn new() -> Self {
        Self {
            gravity_enabled: false, // Disabled until chunks load
            is_flying: false,
            on_ground: false,
        }
    }

    pub fn with_gravity(mut self, enabled: bool) -> Self {
        self.gravity_enabled = enabled;
        self
    }

    pub fn with_flying(mut self, flying: bool) -> Self {
        self.is_flying = flying;
        self
    }
}

/// Bundle for creating a physics-enabled player entity.
#[derive(Bundle)]
pub struct PlayerPhysicsBundle {
    pub body: RigidBody,
    pub collider: Collider,
    pub velocity: Velocity,
    pub gravity_scale: GravityScale,
    pub locked_axes: LockedAxes,
    pub ccd: Ccd,
    pub rustcraft_body: RustcraftPhysicsBody,
    pub friction: Friction,
    pub restitution: Restitution,
    pub collision_groups: CollisionGroups,
    pub damping: Damping,
}

impl PlayerPhysicsBundle {
    /// Create a new player physics bundle with standard player dimensions.
    ///
    /// # Arguments
    /// * `width` - Player hitbox width (x and z)
    /// * `height` - Player hitbox height (y)
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            // KinematicPositionBased gives us full control over movement
            // while still allowing collision detection
            body: RigidBody::KinematicPositionBased,
            // Capsule collider for smooth movement over terrain
            collider: Collider::capsule_y(height / 2.0 - width / 2.0, width / 2.0),
            velocity: Velocity::zero(),
            // We handle gravity ourselves based on game state
            gravity_scale: GravityScale(0.0),
            // Lock rotation - players don't rotate from physics
            locked_axes: LockedAxes::ROTATION_LOCKED,
            // Enable continuous collision detection for fast-moving objects
            ccd: Ccd::enabled(),
            rustcraft_body: RustcraftPhysicsBody::new(),
            // No friction - we handle movement ourselves
            friction: Friction::coefficient(0.0),
            // No bounce
            restitution: Restitution::coefficient(0.0),
            // Player collision group
            collision_groups: CollisionGroups::new(
                Group::GROUP_1, // Player group
                Group::GROUP_2, // Collides with world
            ),
            // Some linear damping for smoother stopping
            damping: Damping {
                linear_damping: 0.0,
                angular_damping: 0.0,
            },
        }
    }

    /// Create with default player dimensions (0.8 width, 1.8 height)
    pub fn default_player() -> Self {
        Self::new(0.8, 1.8)
    }
}

/// Bundle for creating a physics-enabled mob entity.
#[derive(Bundle)]
pub struct MobPhysicsBundle {
    pub body: RigidBody,
    pub collider: Collider,
    pub velocity: Velocity,
    pub gravity_scale: GravityScale,
    pub locked_axes: LockedAxes,
    pub rustcraft_body: RustcraftPhysicsBody,
    pub friction: Friction,
    pub restitution: Restitution,
    pub collision_groups: CollisionGroups,
}

impl MobPhysicsBundle {
    /// Create a new mob physics bundle.
    ///
    /// # Arguments
    /// * `width` - Mob hitbox width (x)
    /// * `height` - Mob hitbox height (y)
    /// * `depth` - Mob hitbox depth (z)
    pub fn new(width: f32, height: f32, depth: f32) -> Self {
        Self {
            body: RigidBody::KinematicPositionBased,
            // Box collider for mobs
            collider: Collider::cuboid(width / 2.0, height / 2.0, depth / 2.0),
            velocity: Velocity::zero(),
            gravity_scale: GravityScale(0.0),
            locked_axes: LockedAxes::ROTATION_LOCKED,
            rustcraft_body: RustcraftPhysicsBody::new().with_gravity(true),
            friction: Friction::coefficient(0.0),
            restitution: Restitution::coefficient(0.0),
            collision_groups: CollisionGroups::new(
                Group::GROUP_3, // Mob group
                Group::GROUP_2, // Collides with world
            ),
        }
    }
}

/// Collision groups used in Rustcraft physics.
pub mod collision_groups {
    use bevy_rapier3d::prelude::Group;

    /// Player entities
    pub const PLAYER: Group = Group::GROUP_1;
    /// World/terrain colliders
    pub const WORLD: Group = Group::GROUP_2;
    /// Mob entities
    pub const MOB: Group = Group::GROUP_3;
    /// Projectiles
    pub const PROJECTILE: Group = Group::GROUP_4;
}

/// Result of a ground check operation.
#[derive(Debug, Clone, Copy)]
pub struct GroundCheckResult {
    /// Whether the entity is on the ground
    pub on_ground: bool,
    /// The normal of the ground surface (if on ground)
    pub ground_normal: Option<Vec3>,
    /// Distance to ground (if detected)
    pub ground_distance: Option<f32>,
}

/// Configuration for the Rustcraft physics plugin.
#[derive(Resource, Clone, Debug)]
pub struct RustcraftPhysicsConfig {
    /// Gravity acceleration
    pub gravity: f32,
    /// Maximum fall speed
    pub terminal_velocity: f32,
    /// Ground detection distance threshold
    pub ground_check_distance: f32,
}

impl Default for RustcraftPhysicsConfig {
    fn default() -> Self {
        Self {
            gravity: constants::GRAVITY,
            terminal_velocity: constants::TERMINAL_VELOCITY,
            ground_check_distance: 0.1,
        }
    }
}

/// Plugin that sets up Rapier physics for Rustcraft.
///
/// This plugin should be added to both client and server apps.
/// It configures Rapier with settings appropriate for a voxel game.
pub struct RustcraftPhysicsPlugin;

impl Plugin for RustcraftPhysicsPlugin {
    fn build(&self, app: &mut App) {
        // Add Rapier plugin with custom configuration
        // In bevy_rapier3d 0.30, RapierConfiguration is a Component attached to the
        // RapierContext entity, not a Resource. We use RapierContextInitialization
        // to configure it at startup.
        app.add_plugins(
            RapierPhysicsPlugin::<NoUserData>::default().with_default_system_setup(true),
        );

        // Add our custom physics config
        app.insert_resource(RustcraftPhysicsConfig::default());

        // System to configure Rapier once the context is spawned
        app.add_systems(Startup, configure_rapier_context);

        // Debug rendering (only in debug builds)
        #[cfg(debug_assertions)]
        {
            app.add_plugins(RapierDebugRenderPlugin::default());
        }
    }
}

/// System to configure the Rapier context after it's spawned.
fn configure_rapier_context(mut query: Query<&mut RapierConfiguration>) {
    for mut config in query.iter_mut() {
        config.gravity = Vec3::new(0.0, constants::GRAVITY, 0.0);
        config.physics_pipeline_active = true;
        config.query_pipeline_active = true;
    }
}

/// Helper function to create a block collider at a specific position.
/// Used when generating collision geometry for the voxel world.
pub fn create_block_collider(position: IVec3) -> (Collider, Transform) {
    let collider = Collider::cuboid(0.5, 0.5, 0.5);
    let transform = Transform::from_translation(Vec3::new(
        position.x as f32 + 0.5,
        position.y as f32 + 0.5,
        position.z as f32 + 0.5,
    ));
    (collider, transform)
}

/// Component for world chunk collision entities.
#[derive(Component)]
pub struct ChunkCollider {
    pub chunk_position: IVec3,
}

/// Bundle for spawning a chunk's collision geometry.
#[derive(Bundle)]
pub struct ChunkColliderBundle {
    pub collider: Collider,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub chunk_marker: ChunkCollider,
    pub collision_groups: CollisionGroups,
}

impl ChunkColliderBundle {
    /// Create a compound collider for a chunk from a list of solid block positions.
    ///
    /// # Arguments
    /// * `chunk_pos` - The chunk's position in chunk coordinates
    /// * `solid_blocks` - List of local block positions within the chunk that are solid
    pub fn from_solid_blocks(chunk_pos: IVec3, solid_blocks: &[(IVec3, bool)]) -> Option<Self> {
        if solid_blocks.is_empty() {
            return None;
        }

        // Create compound collider from individual block colliders
        let shapes: Vec<(Vec3, Quat, Collider)> = solid_blocks
            .iter()
            .filter(|(_, is_solid)| *is_solid)
            .map(|(local_pos, _)| {
                let offset = Vec3::new(
                    local_pos.x as f32 + 0.5,
                    local_pos.y as f32 + 0.5,
                    local_pos.z as f32 + 0.5,
                );
                (offset, Quat::IDENTITY, Collider::cuboid(0.5, 0.5, 0.5))
            })
            .collect();

        if shapes.is_empty() {
            return None;
        }

        let chunk_world_pos = Vec3::new(
            (chunk_pos.x * crate::CHUNK_SIZE) as f32,
            (chunk_pos.y * crate::CHUNK_SIZE) as f32,
            (chunk_pos.z * crate::CHUNK_SIZE) as f32,
        );

        Some(Self {
            collider: Collider::compound(shapes),
            transform: Transform::from_translation(chunk_world_pos),
            global_transform: GlobalTransform::default(),
            chunk_marker: ChunkCollider {
                chunk_position: chunk_pos,
            },
            collision_groups: CollisionGroups::new(
                collision_groups::WORLD,
                Group::ALL, // World collides with everything
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_physics_bundle_creation() {
        let bundle = PlayerPhysicsBundle::default_player();
        assert!(matches!(bundle.body, RigidBody::KinematicPositionBased));
        assert_eq!(bundle.gravity_scale.0, 0.0);
    }

    #[test]
    fn test_mob_physics_bundle_creation() {
        let bundle = MobPhysicsBundle::new(1.0, 1.0, 1.5);
        assert!(matches!(bundle.body, RigidBody::KinematicPositionBased));
    }

    #[test]
    fn test_rustcraft_physics_body() {
        let body = RustcraftPhysicsBody::new()
            .with_gravity(true)
            .with_flying(false);
        assert!(body.gravity_enabled);
        assert!(!body.is_flying);
        assert!(!body.on_ground);
    }
}

//! Generic particle system for visual effects.

use bevy::{color::palettes::css::WHITE, prelude::*};

#[derive(Component)]
pub struct Particle {
    pub lifetime_timer: Timer,
    pub size: f32,
    pub velocity: Vec3,
}

#[derive(Resource)]
pub struct ParticleAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

impl FromWorld for ParticleAssets {
    fn from_world(world: &mut World) -> Self {
        Self {
            mesh: world.resource_mut::<Assets<Mesh>>().add(Sphere::new(10.0)),
            material: world
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial {
                    base_color: WHITE.into(),
                    ..Default::default()
                }),
        }
    }
}

pub fn simulate_particles(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform, &mut Particle)>,
    time: Res<Time>,
) {
    for (entity, mut transform, mut particle) in &mut query {
        if particle.lifetime_timer.tick(time.delta()).just_finished() {
            commands.entity(entity).despawn();
        } else {
            transform.translation += particle.velocity * time.delta_secs();
            transform.scale =
                Vec3::splat(particle.size.lerp(0.0, particle.lifetime_timer.fraction()));
            particle
                .velocity
                .smooth_nudge(&Vec3::ZERO, 4.0, time.delta_secs());
        }
    }
}

pub fn spawn_particle<M: Material>(
    mesh: Handle<Mesh>,
    material: Handle<M>,
    translation: Vec3,
    lifetime: f32,
    size: f32,
    velocity: Vec3,
) -> impl Command {
    move |world: &mut World| {
        world.spawn((
            Particle {
                lifetime_timer: Timer::from_seconds(lifetime, TimerMode::Once),
                size,
                velocity,
            },
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform {
                translation,
                scale: Vec3::splat(size),
                ..Default::default()
            },
        ));
    }
}

pub struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ParticleAssets>();
    }
}

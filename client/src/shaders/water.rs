//! Water shader material with Misty Lake effect
//!
//! This module provides a custom material for rendering water blocks using
//! the Misty Lake shader (ported from ShaderToy), featuring FBM-based wave
//! animation, fresnel reflections, and atmospheric fog.

use bevy::{
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderRef},
};

use super::water_uniforms::WaterUniforms;

/// Plugin for the water shader system
pub struct WaterShaderPlugin;

impl Plugin for WaterShaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<WaterMaterial>::default())
            .init_resource::<WaterShaderTime>()
            .add_systems(Update, (update_water_shader_time, update_water_materials));
    }
}

/// Resource tracking time for water shader animation
#[derive(Resource, Default)]
pub struct WaterShaderTime {
    pub elapsed: f32,
}

/// System to update the water shader time
fn update_water_shader_time(time: Res<Time>, mut water_time: ResMut<WaterShaderTime>) {
    water_time.elapsed = time.elapsed_secs();
}

/// Custom material for water rendering with animated waves
///
/// This material uses a custom WGSL shader that displaces vertices
/// to create standing wave patterns on water surfaces.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct WaterMaterial {
    /// All water shader uniforms grouped together
    #[uniform(0)]
    pub uniforms: WaterUniforms,

    /// Texture atlas for water texture
    #[texture(1)]
    #[sampler(2)]
    pub texture: Option<Handle<Image>>,
}

impl Default for WaterMaterial {
    fn default() -> Self {
        Self {
            uniforms: WaterUniforms {
                time: 0.0,
                wave_scale: 0.5, // Smaller scale = larger wave patterns visible at world scale
                bump_strength: 1.0, // Strong normal perturbation for visible ripples
                water_color: Vec4::new(0.15, 0.35, 0.45, 0.75), // Slightly more opaque teal
                deep_color: Vec4::new(0.05, 0.15, 0.25, 0.9), // Deeper blue-green
                fog_density: 0.00001, // Visible distance fog
                _padding: Vec3::ZERO,
            },
            texture: None,
        }
    }
}

impl Material for WaterMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/misty_lake.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/misty_lake.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

/// Component marker for entities using water material
#[derive(Component)]
pub struct WaterMesh;

/// System to update water material time uniforms
fn update_water_materials(
    water_time: Res<WaterShaderTime>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    for (_, material) in materials.iter_mut() {
        material.uniforms.time = water_time.elapsed;
    }
}

//! Custom Gerstner wave water shader for Rustcraft.
//!
//! This module provides a custom water material that uses our own WGSL shader
//! for realistic Gerstner wave animation. This gives us full control over the
//! water appearance and effects.
//!
//! The shader implements:
//! - Gerstner wave vertex displacement for realistic wave motion
//! - Fresnel-based reflections
//! - Configurable wave parameters (direction, steepness, wavelength, speed)
//! - Up to 4 simultaneous waves

use bevy::{
    asset::embedded_asset,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{
        AsBindGroup, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError,
    },
};

/// Plugin that registers the custom water material and shader.
pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        // Embed the shader at compile time
        embedded_asset!(app, "../../../data/shaders/gerstner_water.wgsl");

        app.add_plugins(MaterialPlugin::<StandardWaterMaterial>::default())
            .init_resource::<WaterTime>()
            .add_systems(Update, update_water_time);
    }
}

/// Resource tracking time for water animation.
/// This is used to update the shader uniforms.
#[derive(Resource, Default)]
pub struct WaterTime {
    pub elapsed: f32,
}

/// System to update water time each frame.
fn update_water_time(time: Res<Time>, mut water_time: ResMut<WaterTime>) {
    water_time.elapsed = time.elapsed_secs();
}

/// Standard water material type alias for convenience.
pub type StandardWaterMaterial = ExtendedMaterial<StandardMaterial, WaterMaterial>;

/// Water material extension for the standard PBR pipeline.
///
/// This material extends Bevy's StandardMaterial with custom water properties
/// and uses our Gerstner wave shader for vertex animation.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct WaterMaterial {
    /// Wave amplitude multiplier (affects wave height)
    #[uniform(100)]
    pub amplitude: f32,

    /// Water clarity (0.0 = murky, 1.0 = crystal clear)
    #[uniform(100)]
    pub clarity: f32,

    /// Deep water color
    #[uniform(100)]
    pub deep_color: LinearRgba,

    /// Shallow water color
    #[uniform(100)]
    pub shallow_color: LinearRgba,

    /// Foam/edge color
    #[uniform(100)]
    pub edge_color: LinearRgba,

    /// Edge foam scale
    #[uniform(100)]
    pub edge_scale: f32,

    /// UV coordinate scale for texturing
    #[uniform(100)]
    pub coord_scale: Vec2,

    /// UV coordinate offset for texturing
    #[uniform(100)]
    pub coord_offset: Vec2,
}

impl Default for WaterMaterial {
    fn default() -> Self {
        Self {
            amplitude: 0.5,
            clarity: 0.3,
            deep_color: LinearRgba::new(0.05, 0.15, 0.25, 0.9),
            shallow_color: LinearRgba::new(0.15, 0.35, 0.45, 0.75),
            edge_color: LinearRgba::new(0.8, 0.9, 1.0, 0.5),
            edge_scale: 0.1,
            coord_scale: Vec2::new(1.0, 1.0),
            coord_offset: Vec2::ZERO,
        }
    }
}

impl MaterialExtension for WaterMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://client/data/shaders/gerstner_water.wgsl".into()
    }

    fn vertex_shader() -> ShaderRef {
        "embedded://client/data/shaders/gerstner_water.wgsl".into()
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &bevy::render::mesh::MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Enable alpha blending for water transparency
        if let Some(fragment) = &mut descriptor.fragment {
            if let Some(target) = fragment.targets.first_mut() {
                if let Some(target_state) = target {
                    target_state.blend =
                        Some(bevy::render::render_resource::BlendState::ALPHA_BLENDING);
                }
            }
        }
        Ok(())
    }
}

/// Component marker for entities using water material.
///
/// This is used to identify water mesh entities in the chunk rendering system.
#[derive(Component)]
pub struct WaterMesh;

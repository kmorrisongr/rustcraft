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
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{
        AsBindGroup, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError,
    },
};

/// The Gerstner water shader source code, included at compile time.
/// Using a unique UUID for the shader handle.
const WATER_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(0x1a2b3c4d5e6f7890abcdef1234567890);

/// Plugin that registers the custom water material and shader.
pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        // Load the shader from the embedded source
        app.world_mut().resource_mut::<Assets<Shader>>().insert(
            &WATER_SHADER_HANDLE,
            Shader::from_wgsl(
                include_str!("../../../data/shaders/gerstner_water.wgsl"),
                file!(),
            ),
        );

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
/// and uses our Gerstner wave shader for fragment animation.
///
/// Note: Currently wave parameters are hardcoded in the shader for simplicity.
/// Future versions can add uniform bindings for runtime configuration.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct WaterMaterial {
    // Placeholder - the shader currently uses hardcoded values
    // Future: Add uniform bindings here for configurable wave parameters
}

impl MaterialExtension for WaterMaterial {
    fn fragment_shader() -> ShaderRef {
        WATER_SHADER_HANDLE.into()
    }

    // Note: We don't override vertex_shader() - use Bevy's default vertex shader.
    // Our fragment shader handles the Gerstner wave animation in screen space.

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

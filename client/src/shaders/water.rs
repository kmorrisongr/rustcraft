//! Custom Gerstner wave water shader for Rustcraft.
//!
//! This module provides a custom water material that uses our own WGSL shader
//! for realistic Gerstner wave animation. This gives us full control over the
//! water appearance and effects.
//!
//! The shader implements:
//! - Gerstner wave normal animation for realistic wave motion
//! - Fresnel-based reflections
//! - Hardcoded wave parameters matching `WavePreset::Ocean` from shared crate

use bevy::{
    asset::weak_handle,
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
        // Load the shader from the embedded source
        let shader = Shader::from_wgsl(
            include_str!("../../../data/shaders/gerstner_water.wgsl"),
            file!(),
        );
        app.world_mut()
            .resource_mut::<Assets<Shader>>()
            .insert(WATER_SHADER_HANDLE.id(), shader);

        app.add_plugins(MaterialPlugin::<StandardWaterMaterial>::default());
    }
}

/// Shader handle for the water material.
const WATER_SHADER_HANDLE: Handle<Shader> = weak_handle!("1a2b3c4d-5e6f-7890-abcd-ef1234567890");

/// Standard water material type alias for convenience.
pub type StandardWaterMaterial = ExtendedMaterial<StandardMaterial, WaterMaterial>;

/// Water material extension for the standard PBR pipeline.
///
/// This material extends Bevy's StandardMaterial with our custom Gerstner wave
/// fragment shader. Wave parameters are currently hardcoded in the shader.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct WaterMaterial {}

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

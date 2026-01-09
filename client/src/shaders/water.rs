//! Custom Gerstner wave water shader for Rustcraft.
//!
//! This module provides a custom water material that uses our own WGSL shader
//! for realistic Gerstner wave animation. This gives us full control over the
//! water appearance and effects.
//!
//! The shader implements:
//! - Gerstner wave normal animation for realistic wave motion
//! - Fresnel-based reflections
//! - Configurable water colors via uniforms

use bevy::{
    asset::weak_handle,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{
        AsBindGroup, RenderPipelineDescriptor, ShaderRef, ShaderType, SpecializedMeshPipelineError,
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

/// Water color configuration passed to the shader as uniforms.
///
/// Uses `Vec4` for each color to ensure proper 16-byte alignment in GPU memory.
/// The alpha channel of each vec4 is used to pack additional parameters:
/// - `shallow_color.w` = water alpha/transparency
/// - `deep_color.w` = wave amplitude scale
/// - `sky_color.w` = unused (padding for alignment)
#[derive(Clone, Copy, Debug, Reflect, ShaderType)]
pub struct WaterColors {
    /// Shallow water color (xyz) and water alpha (w)
    pub shallow_color: Vec4,
    /// Deep water color (xyz) and amplitude scale (w)
    pub deep_color: Vec4,
    /// Sky reflection color (xyz), w is padding
    pub sky_color: Vec4,
}

impl Default for WaterColors {
    fn default() -> Self {
        Self {
            // Shallow water: teal-ish, alpha = 0.8
            shallow_color: Vec4::new(0.15, 0.35, 0.45, 0.8),
            // Deep water: darker blue, amplitude_scale = 0.5
            deep_color: Vec4::new(0.05, 0.15, 0.25, 0.5),
            // Sky reflection: light blue, w unused
            sky_color: Vec4::new(0.5, 0.7, 0.9, 1.0),
        }
    }
}

impl WaterColors {
    /// Create water colors with custom transparency.
    pub fn with_alpha(mut self, alpha: f32) -> Self {
        self.shallow_color.w = alpha;
        self
    }

    /// Create water colors with custom wave amplitude scale.
    pub fn with_amplitude_scale(mut self, scale: f32) -> Self {
        self.deep_color.w = scale;
        self
    }
}

/// Water material extension for the standard PBR pipeline.
///
/// This material extends Bevy's StandardMaterial with our custom Gerstner wave
/// fragment shader. Water colors and parameters are configurable via the
/// `colors` uniform.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct WaterMaterial {
    /// Water color configuration passed to the shader
    #[uniform(100)]
    pub colors: WaterColors,
}

impl Default for WaterMaterial {
    fn default() -> Self {
        Self {
            colors: WaterColors::default(),
        }
    }
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

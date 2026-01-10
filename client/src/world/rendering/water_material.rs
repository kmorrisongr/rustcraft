//! Water material for Gerstner wave rendering.
//!
//! This module provides a custom material that extends Bevy's StandardMaterial
//! with additional uniforms for water wave simulation parameters.

use bevy::{
    asset::Asset,
    pbr::{ExtendedMaterial, MaterialExtension, StandardMaterial},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderRef, ShaderType},
};

use shared::world::WaveScaleConfig;

/// Uniform data for the water shader.
/// This is passed to the GPU and controls wave appearance.
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct WaterMaterialUniform {
    /// Deep water color (RGBA)
    pub deep_color: Vec4,
    /// Shallow water color (RGBA)
    pub shallow_color: Vec4,
    /// Time offset for desynchronizing wave animation between chunks
    pub time_offset: f32,
    /// Wave amplitude multiplier (default: 0.08)
    pub wave_amplitude: f32,
    /// Wave speed multiplier (default: 1.0)
    pub wave_speed: f32,
    /// Number of wave layers to compute (1-4, default: 3)
    pub wave_layers: u32,
}

impl Default for WaterMaterialUniform {
    fn default() -> Self {
        Self {
            deep_color: Vec4::new(0.1, 0.3, 0.5, 0.8),
            shallow_color: Vec4::new(0.3, 0.6, 0.8, 0.6),
            time_offset: 0.0,
            wave_amplitude: 0.08,
            wave_speed: 1.0,
            wave_layers: 3,
        }
    }
}

/// Water material extension that adds Gerstner wave parameters.
///
/// This is used with `ExtendedMaterial<StandardMaterial, WaterMaterialExtension>`
/// to create a complete water material with animated waves.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct WaterMaterialExtension {
    #[uniform(100)]
    pub uniform: WaterMaterialUniform,
}

impl Default for WaterMaterialExtension {
    fn default() -> Self {
        Self {
            uniform: WaterMaterialUniform::default(),
        }
    }
}

impl MaterialExtension for WaterMaterialExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/water.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/water.wgsl".into()
    }

    fn deferred_vertex_shader() -> ShaderRef {
        "shaders/water.wgsl".into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        "shaders/water.wgsl".into()
    }
}

/// Type alias for the complete water material.
pub type WaterMaterial = ExtendedMaterial<StandardMaterial, WaterMaterialExtension>;

/// Creates a new water material with default settings.
///
/// # Arguments
/// * `time_offset` - Optional time offset to desynchronize waves between chunks
///
/// # Returns
/// A `WaterMaterial` ready to be added to the asset store.
pub fn create_water_material(time_offset: Option<f32>) -> WaterMaterial {
    let mut uniform = WaterMaterialUniform::default();
    if let Some(offset) = time_offset {
        uniform.time_offset = offset;
    }

    ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::srgba(0.2, 0.5, 0.8, 0.7),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.1,
            reflectance: 0.5,
            cull_mode: None, // Render both sides for underwater visibility
            double_sided: true,
            ..default()
        },
        extension: WaterMaterialExtension { uniform },
    }
}

/// Resource holding the shared water material handle.
///
/// Water meshes share a single material to reduce draw calls.
/// The material is created once and reused across all water surfaces.
#[derive(Resource)]
pub struct WaterMaterialResource {
    /// Handle to the water material asset
    pub handle: Handle<WaterMaterial>,
}

/// System to initialize the water material resource.
///
/// This runs during game setup and creates the shared water material.
pub fn setup_water_material(
    mut commands: Commands,
    mut materials: ResMut<Assets<WaterMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // Pre-load the shader
    let _shader_handle: Handle<Shader> = asset_server.load("shaders/water.wgsl");

    // Create the water material
    let water_material = create_water_material(None);
    let handle = materials.add(water_material);

    commands.insert_resource(WaterMaterialResource { handle });

    info!("Water material initialized with Gerstner wave shader");
}

/// Settings to control water rendering quality and appearance.
#[derive(Resource, Debug, Clone)]
pub struct WaterRenderSettings {
    /// Whether water rendering is enabled
    pub enabled: bool,
    /// Maximum distance at which water waves are animated (squared for comparison)
    pub wave_distance_sq: f32,
    /// Number of wave layers at close range (1-4)
    pub wave_layers_near: u32,
    /// Number of wave layers at far range (1-4)
    pub wave_layers_far: u32,
    /// Wave amplitude multiplier
    pub wave_amplitude: f32,
    /// Wave animation speed
    pub wave_speed: f32,
    /// Configuration for wave scale based on local water volume
    pub wave_scale_config: WaveScaleConfig,
    /// Number of subdivisions per water cell for smooth wave displacement.
    /// Higher values = smoother waves but more vertices.
    /// - 1 = 4 vertices per cell (original, no tessellation)
    /// - 2 = 9 vertices per cell (2x2 grid) - default
    /// - 4 = 25 vertices per cell (4x4 grid) - higher quality, more expensive
    /// - 8 = 81 vertices per cell (high quality)
    pub tessellation: u32,
    /// LOD tessellation level (fewer subdivisions for distant water)
    pub tessellation_lod: u32,
}

impl Default for WaterRenderSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            wave_distance_sq: 64.0 * 64.0, // 64 blocks
            wave_layers_near: 4,
            wave_layers_far: 2,
            wave_amplitude: 0.08,
            wave_speed: 1.0,
            wave_scale_config: WaveScaleConfig::default(),
            tessellation: 2,
            tessellation_lod: 2,
        }
    }
}

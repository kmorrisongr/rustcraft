//! Gerstner wave water shader for Rustcraft
//!
//! This shader implements realistic water rendering using:
//! - Multiple Gerstner wave layers for surface displacement
//! - Fresnel-based reflectivity
//! - Depth-based color and transparency
//! - Animated flow with configurable wave parameters

#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::globals,
}

// Water material parameters (bound via ExtendedMaterial)
struct WaterMaterialUniform {
    // Base water color (deep water)
    deep_color: vec4<f32>,
    // Shallow water color
    shallow_color: vec4<f32>,
    // Time offset for animation
    time_offset: f32,
    // Wave amplitude multiplier
    wave_amplitude: f32,
    // Wave speed multiplier
    wave_speed: f32,
    // Number of wave layers (1-4)
    wave_layers: u32,
};

@group(2) @binding(100)
var<uniform> water_material: WaterMaterialUniform;

// Gerstner wave parameters for each layer
// Direction (x,z), Steepness (Q), Wavelength (L)
const WAVE_PARAMS: array<vec4<f32>, 4> = array<vec4<f32>, 4>(
    vec4<f32>(1.0, 0.0, 0.5, 8.0),     // Primary wave - long, gentle
    vec4<f32>(0.7, 0.7, 0.35, 4.0),    // Secondary wave - medium
    vec4<f32>(-0.3, 0.9, 0.25, 2.5),   // Tertiary wave - shorter
    vec4<f32>(0.9, -0.4, 0.15, 1.5),   // Detail wave - small ripples
);

// Calculate a single Gerstner wave contribution
// Returns: xyz = position offset, w = not used here
fn gerstner_wave(
    position: vec2<f32>,
    direction: vec2<f32>,
    steepness: f32,
    wavelength: f32,
    time: f32,
    amplitude: f32,
) -> vec3<f32> {
    let k = 2.0 * 3.14159265 / wavelength;
    let c = sqrt(9.8 / k); // Phase speed from dispersion relation
    let d = normalize(direction);
    let f = k * (dot(d, position) - c * time);
    let a = amplitude * steepness / k;
    
    return vec3<f32>(
        d.x * a * cos(f),
        amplitude * sin(f),
        d.y * a * cos(f)
    );
}

// Calculate the normal for a Gerstner wave at a point
fn gerstner_wave_normal(
    position: vec2<f32>,
    direction: vec2<f32>,
    steepness: f32,
    wavelength: f32,
    time: f32,
    amplitude: f32,
) -> vec3<f32> {
    let k = 2.0 * 3.14159265 / wavelength;
    let c = sqrt(9.8 / k);
    let d = normalize(direction);
    let f = k * (dot(d, position) - c * time);
    let a = amplitude * steepness;
    
    return vec3<f32>(
        -d.x * a * cos(f),
        1.0,
        -d.y * a * cos(f)
    );
}

// Vertex shader - applies Gerstner wave displacement
@vertex
fn vertex(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    
    // Get world position from mesh transform
    let world_from_local = mesh_functions::get_world_from_local(vertex_index);
    var world_position = (world_from_local * vec4<f32>(position, 1.0)).xyz;
    
    // Accumulate wave displacement
    var displacement = vec3<f32>(0.0, 0.0, 0.0);
    var accumulated_normal = vec3<f32>(0.0, 1.0, 0.0);
    
    let time = globals.time * water_material.wave_speed + water_material.time_offset;
    let base_amplitude = water_material.wave_amplitude;
    let pos_xz = world_position.xz;
    
    // Apply each wave layer
    let num_layers = min(water_material.wave_layers, 4u);
    for (var i = 0u; i < num_layers; i = i + 1u) {
        let params = WAVE_PARAMS[i];
        let dir = vec2<f32>(params.x, params.y);
        let steepness = params.z;
        let wavelength = params.w;
        
        // Reduce amplitude for each successive layer
        let layer_amplitude = base_amplitude * pow(0.7, f32(i));
        
        displacement += gerstner_wave(pos_xz, dir, steepness, wavelength, time, layer_amplitude);
        accumulated_normal += gerstner_wave_normal(pos_xz, dir, steepness, wavelength, time, layer_amplitude);
    }
    
    // Apply displacement
    world_position += displacement;
    
    // Normalize the accumulated normal
    let final_normal = normalize(accumulated_normal);
    
    // Transform to clip space
    out.position = position_world_to_clip(world_position);
    out.world_position = vec4<f32>(world_position, 1.0);
    out.world_normal = final_normal;
    out.uv = uv;
    
    // Pass instance index for mesh data lookup
    #ifdef VERTEX_OUTPUT_INSTANCE_INDEX
        out.instance_index = vertex_index;
    #endif
    
    return out;
}

// Fresnel effect calculation
fn fresnel_schlick(cos_theta: f32, f0: f32) -> f32 {
    return f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0);
}

// Fragment shader - water surface shading
@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    
    // Get view direction
    let view_dir = normalize(in.world_position.xyz);
    let normal = normalize(in.world_normal);
    
    // Fresnel calculation for water (IOR ~1.33)
    let cos_theta = max(dot(-view_dir, normal), 0.0);
    let fresnel = fresnel_schlick(cos_theta, 0.02);
    
    // Mix between deep and shallow water colors based on view angle
    // More grazing angles show more reflection (lighter color)
    let base_color = mix(water_material.deep_color.rgb, water_material.shallow_color.rgb, fresnel);
    
    // Add specular highlight (simple approximation)
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3)); // Approximate sun direction
    let half_vec = normalize(light_dir - view_dir);
    let spec = pow(max(dot(normal, half_vec), 0.0), 64.0);
    let specular = vec3<f32>(1.0, 1.0, 0.95) * spec * 0.5;
    
    // Final color with transparency
    // Transparency decreases at grazing angles (fresnel effect)
    let alpha = mix(0.6, 0.85, fresnel);
    
    out.color = vec4<f32>(base_color + specular, alpha);
    
    return out;
}

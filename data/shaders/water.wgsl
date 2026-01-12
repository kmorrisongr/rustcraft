//! Gerstner wave water shader for Rustcraft
//!
//! This shader implements realistic water rendering using:
//! - Vertex displacement via multiple Gerstner wave layers
//! - Fresnel-based reflectivity
//! - Depth-based color and transparency
//! - Animated flow with configurable wave parameters
//!
//! This shader extends Bevy's StandardMaterial via MaterialExtension,
//! providing both vertex displacement and custom fragment coloring.

#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
    mesh_view_bindings::globals,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{Vertex, VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{Vertex, VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

// Water material parameters (bound via ExtendedMaterial at binding 100)
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
    // Wave layer parameters: vec4(dir_x, dir_z, steepness, wavelength)
    wave_params_0: vec4<f32>,
    wave_params_1: vec4<f32>,
    wave_params_2: vec4<f32>,
    wave_params_3: vec4<f32>,
};

@group(2) @binding(100)
var<uniform> water_material: WaterMaterialUniform;

// Helper function to get wave parameters for a given layer index
// Returns vec4<f32>(dir_x, dir_z, steepness, wavelength)
fn get_wave_params(layer_index: u32) -> vec4<f32> {
    switch layer_index {
        case 0u: { return water_material.wave_params_0; }
        case 1u: { return water_material.wave_params_1; }
        case 2u: { return water_material.wave_params_2; }
        case 3u: { return water_material.wave_params_3; }
        default: { return vec4<f32>(1.0, 0.0, 0.5, 8.0); }
    }
}

const PI: f32 = 3.14159265;
const GRAVITY: f32 = 9.8;

// Calculate a single Gerstner wave displacement
// Returns vec3: (x_offset, y_offset, z_offset)
fn gerstner_wave_displacement(
    position: vec2<f32>,
    direction: vec2<f32>,
    steepness: f32,
    wavelength: f32,
    time: f32,
    amplitude: f32,
) -> vec3<f32> {
    let k = 2.0 * PI / wavelength;
    let c = sqrt(GRAVITY / k); // Phase speed from dispersion relation
    let d = normalize(direction);
    let f = k * (dot(d, position) - c * time);
    let a = amplitude * steepness / k;
    
    return vec3<f32>(
        d.x * a * cos(f),
        amplitude * sin(f),
        d.y * a * cos(f)
    );
}

// Calculate the normal from Gerstner wave derivatives
fn gerstner_wave_normal(
    position: vec2<f32>,
    direction: vec2<f32>,
    steepness: f32,
    wavelength: f32,
    time: f32,
    amplitude: f32,
) -> vec3<f32> {
    let k = 2.0 * PI / wavelength;
    let c = sqrt(GRAVITY / k);
    let d = normalize(direction);
    let f = k * (dot(d, position) - c * time);
    let a = amplitude * steepness;
    
    // Partial derivatives
    let dx = -d.x * a * cos(f);
    let dz = -d.y * a * cos(f);
    
    return vec3<f32>(dx, 1.0, dz);
}

// Compute total displacement from all wave layers
fn compute_wave_displacement(world_pos: vec2<f32>, time: f32, num_layers: u32, base_amplitude: f32) -> vec3<f32> {
    var displacement = vec3<f32>(0.0, 0.0, 0.0);
    
    for (var i = 0u; i < num_layers; i = i + 1u) {
        let params = get_wave_params(i);
        let dir = vec2<f32>(params.x, params.y);
        let steepness = params.z;
        let wavelength = params.w;
        let layer_amplitude = base_amplitude * pow(0.7, f32(i));
        
        displacement += gerstner_wave_displacement(world_pos, dir, steepness, wavelength, time, layer_amplitude);
    }
    
    return displacement;
}

// Compute combined normal from all wave layers
fn compute_wave_normal(world_pos: vec2<f32>, time: f32, num_layers: u32, base_amplitude: f32) -> vec3<f32> {
    var accumulated_normal = vec3<f32>(0.0, 1.0, 0.0);
    
    for (var i = 0u; i < num_layers; i = i + 1u) {
        let params = get_wave_params(i);
        let dir = vec2<f32>(params.x, params.y);
        let steepness = params.z;
        let wavelength = params.w;
        let layer_amplitude = base_amplitude * pow(0.7, f32(i));
        
        accumulated_normal += gerstner_wave_normal(world_pos, dir, steepness, wavelength, time, layer_amplitude);
    }
    
    return normalize(accumulated_normal);
}

// Fresnel effect calculation using Schlick's approximation
fn fresnel_schlick(cos_theta: f32, f0: f32) -> f32 {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

// ============================================================================
// VERTEX SHADER
// ============================================================================

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    
    // Get world position from mesh instance
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    var world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    
    // Calculate time for wave animation
    let time = globals.time * water_material.wave_speed + water_material.time_offset;
    let num_layers = min(water_material.wave_layers, 4u);
    
    // Get wave scale from vertex color red channel (0.0 = flat/ripples, 1.0 = full waves)
    // This is calculated per-cell based on local water volume
#ifdef VERTEX_COLORS
    let wave_scale = clamp(vertex.color.r, 0.0, 1.0);
#else
    let wave_scale = 1.0;
#endif
    
    // Apply wave scale to base amplitude
    let base_amplitude = water_material.wave_amplitude * wave_scale;
    
    // Apply Gerstner wave displacement
    let displacement = compute_wave_displacement(world_position.xz, time, num_layers, base_amplitude);
    world_position.x += displacement.x;
    world_position.y += displacement.y;
    world_position.z += displacement.z;
    
    // Calculate wave normal at displaced position
    let wave_normal = compute_wave_normal(world_position.xz, time, num_layers, base_amplitude);
    
    // Transform normal to world space (for a flat water surface, we mainly use the wave normal)
    let world_normal = normalize(wave_normal);
    
    // Standard vertex output setup
    out.position = position_world_to_clip(world_position.xyz);
    out.world_position = world_position;
    out.world_normal = world_normal;
    
#ifdef VERTEX_UVS
    out.uv = vertex.uv;
#endif

#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif

#ifdef VERTEX_TANGENTS
    // Compute tangent from wave direction (primary wave direction)
    let tangent_dir = normalize(vec3<f32>(1.0, 0.0, 0.0));
    out.world_tangent = vec4<f32>(tangent_dir, 1.0);
#endif

#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif

#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif

    return out;
}

// ============================================================================
// FRAGMENT SHADER
// ============================================================================

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    // Generate PBR input from standard material
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    
    // Calculate time for any additional fragment effects
    let time = globals.time * water_material.wave_speed + water_material.time_offset;
    let num_layers = min(water_material.wave_layers, 4u);
    
    // Get wave scale from vertex color red channel (interpolated across triangle)
#ifdef VERTEX_COLORS
    let wave_scale = clamp(in.color.r, 0.0, 1.0);
#else
    let wave_scale = 1.0;
#endif
    
    let base_amplitude = water_material.wave_amplitude * wave_scale;
    
    // Recalculate normal at fragment level for smoother shading
    let wave_normal = compute_wave_normal(in.world_position.xz, time, num_layers, base_amplitude);
    
    // Use the wave normal for lighting
    pbr_input.N = wave_normal;
    pbr_input.world_normal = wave_normal;
    
    // Calculate view direction for fresnel
    let V = pbr_input.V;
    let cos_theta = max(dot(V, pbr_input.N), 0.0);
    let fresnel = fresnel_schlick(cos_theta, 0.02);
    
    // Mix between deep and shallow water colors based on fresnel
    let water_color = mix(
        water_material.deep_color.rgb,
        water_material.shallow_color.rgb,
        fresnel
    );
    
    // Apply water color to PBR input
    pbr_input.material.base_color = vec4<f32>(water_color, pbr_input.material.base_color.a);
    
    // Adjust alpha based on fresnel (more opaque at grazing angles)
    pbr_input.material.base_color.a = mix(0.6, 0.85, fresnel);
    
    // Apply alpha discard if needed
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}

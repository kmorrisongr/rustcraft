// Gerstner Waves Water Shader for Rustcraft
// Implements Gerstner wave vertex animation and PBR water rendering
//
// Based on GPU Gems Chapter 1: Effective Water Simulation from Physical Models
// https://developer.nvidia.com/gpugems/gpugems/part-i-natural-effects/chapter-1-effective-water-simulation-physical-models

#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#import bevy_core_pipeline::tonemapping::tone_mapping
#import bevy_render::view::View

@group(0) @binding(0) var<uniform> view: View;

// ============================================================================
// Gerstner Wave Parameters
// ============================================================================

struct GerstnerWave {
    direction: vec2<f32>,    // Wave direction (normalized)
    steepness: f32,          // 0.0 = sine wave, 1.0 = sharp crests
    wavelength: f32,         // Wavelength in world units
    speed: f32,              // Wave speed multiplier
    _padding: f32,
}

struct WaterUniforms {
    time: f32,
    base_level: f32,         // Base water level (Y coordinate)
    num_waves: u32,
    _padding: f32,
    waves: array<GerstnerWave, 4>,  // Support up to 4 waves
}

@group(2) @binding(100) var<uniform> water_uniforms: WaterUniforms;

// ============================================================================
// Constants
// ============================================================================

const PI: f32 = 3.14159265359;

// ============================================================================
// Gerstner Wave Functions
// ============================================================================

/// Calculate wave number (k = 2π / wavelength)
fn wave_number(wavelength: f32) -> f32 {
    return 2.0 * PI / wavelength;
}

/// Calculate angular frequency (ω = k * speed)
fn frequency(k: f32, speed: f32) -> f32 {
    return k * speed;
}

/// Calculate Gerstner wave displacement for a single wave
/// Returns (horizontal_displacement, vertical_displacement)
fn gerstner_wave(wave: GerstnerWave, position: vec2<f32>, time: f32) -> vec3<f32> {
    let k = wave_number(wave.wavelength);
    let omega = frequency(k, wave.speed);
    let phase = k * dot(wave.direction, position) - omega * time;
    
    let cos_phase = cos(phase);
    let sin_phase = sin(phase);
    
    // Amplitude based on steepness
    let amplitude = wave.steepness / k;
    
    // Horizontal displacement (X, Z)
    let horizontal = wave.direction * amplitude * sin_phase;
    
    // Vertical displacement (Y)
    let vertical = amplitude * cos_phase;
    
    return vec3<f32>(horizontal.x, vertical, horizontal.y);
}

/// Calculate combined displacement from all waves
fn calculate_total_displacement(position: vec2<f32>, time: f32) -> vec3<f32> {
    var total_displacement = vec3<f32>(0.0);
    
    for (var i = 0u; i < water_uniforms.num_waves; i++) {
        total_displacement += gerstner_wave(water_uniforms.waves[i], position, time);
    }
    
    return total_displacement;
}

/// Calculate Gerstner wave normal for a single wave
fn gerstner_wave_normal(wave: GerstnerWave, position: vec2<f32>, time: f32) -> vec3<f32> {
    let k = wave_number(wave.wavelength);
    let omega = frequency(k, wave.speed);
    let phase = k * dot(wave.direction, position) - omega * time;
    
    let cos_phase = cos(phase);
    let sin_phase = sin(phase);
    
    let wa = wave.steepness;
    
    // Normal calculation for Gerstner waves
    let normal_x = -wave.direction.x * wa * cos_phase;
    let normal_y = 1.0 - wa * sin_phase;
    let normal_z = -wave.direction.y * wa * cos_phase;
    
    return vec3<f32>(normal_x, normal_y, normal_z);
}

/// Calculate combined normal from all waves
fn calculate_total_normal(position: vec2<f32>, time: f32) -> vec3<f32> {
    var total_normal = vec3<f32>(0.0, 1.0, 0.0);
    
    for (var i = 0u; i < water_uniforms.num_waves; i++) {
        total_normal += gerstner_wave_normal(water_uniforms.waves[i], position, time);
    }
    
    return normalize(total_normal);
}

// ============================================================================
// Vertex Shader
// ============================================================================

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    
    // Get initial world position
    var world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0)
    );
    
    // Apply Gerstner wave displacement to top faces
#ifdef VERTEX_NORMALS
    let is_top_face = vertex.normal.y > 0.5;
    if is_top_face {
        let position_2d = world_position.xz;
        let displacement = calculate_total_displacement(position_2d, water_uniforms.time);
        
        // Apply displacement
        world_position.x += displacement.x;
        world_position.y += displacement.y;
        world_position.z += displacement.z;
        
        // Calculate perturbed normal
        out.world_normal = calculate_total_normal(position_2d, water_uniforms.time);
    } else {
        // Non-top faces use standard normal
        out.world_normal = mesh_functions::mesh_normal_local_to_world(
            vertex.normal,
            vertex.instance_index
        );
    }
#endif
    
    out.world_position = world_position;
    out.position = position_world_to_clip(world_position.xyz);

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif

#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif

    return out;
}

// ============================================================================
// Fragment Shader (Basic PBR Water)
// ============================================================================

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = in.world_position.xyz;
    let camera_pos = view.world_position;
    let view_dir = normalize(camera_pos - world_pos);
    
    // Base water color
    let base_color = vec4<f32>(0.1, 0.4, 0.6, 0.7);
    let deep_color = vec3<f32>(0.05, 0.2, 0.4);
    
    // Fresnel effect
    let ndotv = max(dot(in.world_normal, view_dir), 0.0);
    let fresnel = pow(1.0 - ndotv, 3.0);
    
    // Simple reflection (sky color)
    let sky_color = vec3<f32>(0.5, 0.7, 0.9);
    
    // Combine base color with reflection
    var final_color = mix(base_color.rgb, sky_color, fresnel * 0.5);
    final_color = mix(final_color, deep_color, (1.0 - ndotv) * 0.3);
    
    // Specular highlight
    let sun_dir = normalize(vec3<f32>(0.3, 0.8, 0.5));
    let half_vec = normalize(sun_dir + view_dir);
    let spec = pow(max(dot(in.world_normal, half_vec), 0.0), 128.0);
    final_color += vec3<f32>(1.0, 1.0, 0.9) * spec * 0.5;
    
    return vec4<f32>(final_color, base_color.a);
}

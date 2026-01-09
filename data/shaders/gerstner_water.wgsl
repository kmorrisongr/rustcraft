// Gerstner Waves Water Shader for Rustcraft
// Implements Gerstner wave vertex animation for water surfaces
//
// Based on GPU Gems Chapter 1: Effective Water Simulation from Physical Models
// https://developer.nvidia.com/gpugems/gpugems/part-i-natural-effects/chapter-1-effective-water-simulation-physical-models
//
// This shader extends Bevy's StandardMaterial using the MaterialExtension system.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::globals,
}

// ============================================================================
// Material Extension Bindings (group 2)
// ============================================================================

struct WaterMaterialUniform {
    amplitude: f32,
    clarity: f32,
    deep_color: vec4<f32>,
    shallow_color: vec4<f32>,
    edge_color: vec4<f32>,
    edge_scale: f32,
    coord_scale: vec2<f32>,
    coord_offset: vec2<f32>,
}

@group(2) @binding(100)
var<uniform> water_material: WaterMaterialUniform;

// ============================================================================
// Constants
// ============================================================================

const PI: f32 = 3.14159265359;

// ============================================================================
// Gerstner Wave Structures and Functions
// ============================================================================

struct GerstnerWave {
    direction: vec2<f32>,
    steepness: f32,
    wavelength: f32,
    speed: f32,
}

/// Calculate wave number (k = 2π / wavelength)
fn wave_number(wavelength: f32) -> f32 {
    return 2.0 * PI / wavelength;
}

/// Calculate Gerstner wave displacement for a single wave
fn gerstner_wave(wave: GerstnerWave, position: vec2<f32>, time: f32) -> vec3<f32> {
    let k = wave_number(wave.wavelength);
    let omega = k * wave.speed;
    let phase = k * dot(wave.direction, position) - omega * time;
    
    let cos_phase = cos(phase);
    let sin_phase = sin(phase);
    
    let amplitude = wave.steepness / k;
    let horizontal = wave.direction * amplitude * sin_phase;
    let vertical = amplitude * cos_phase;
    
    return vec3<f32>(horizontal.x, vertical, horizontal.y);
}

/// Calculate combined displacement from predefined ocean waves
fn calculate_total_displacement(position: vec2<f32>, time: f32, amplitude_scale: f32) -> vec3<f32> {
    var total = vec3<f32>(0.0);
    
    // Primary wave - largest, slowest
    let wave1 = GerstnerWave(normalize(vec2<f32>(1.0, 0.3)), 0.5 * amplitude_scale, 8.0, 1.5);
    total += gerstner_wave(wave1, position, time);
    
    // Secondary wave - medium size, different direction
    let wave2 = GerstnerWave(normalize(vec2<f32>(-0.7, 1.0)), 0.4 * amplitude_scale, 5.0, 1.8);
    total += gerstner_wave(wave2, position, time);
    
    // Tertiary wave - smaller, faster
    let wave3 = GerstnerWave(normalize(vec2<f32>(0.5, -1.0)), 0.3 * amplitude_scale, 3.0, 2.2);
    total += gerstner_wave(wave3, position, time);
    
    // Detail wave - smallest ripples
    let wave4 = GerstnerWave(normalize(vec2<f32>(-1.0, -0.5)), 0.2 * amplitude_scale, 1.5, 2.8);
    total += gerstner_wave(wave4, position, time);
    
    return total;
}

/// Calculate Gerstner wave normal contribution
fn gerstner_wave_normal(wave: GerstnerWave, position: vec2<f32>, time: f32) -> vec3<f32> {
    let k = wave_number(wave.wavelength);
    let omega = k * wave.speed;
    let phase = k * dot(wave.direction, position) - omega * time;
    
    let cos_phase = cos(phase);
    let sin_phase = sin(phase);
    let wa = wave.steepness;
    
    return vec3<f32>(
        -wave.direction.x * wa * cos_phase,
        1.0 - wa * sin_phase,
        -wave.direction.y * wa * cos_phase
    );
}

/// Calculate combined normal from predefined ocean waves
fn calculate_total_normal(position: vec2<f32>, time: f32, amplitude_scale: f32) -> vec3<f32> {
    var total = vec3<f32>(0.0, 1.0, 0.0);
    
    let wave1 = GerstnerWave(normalize(vec2<f32>(1.0, 0.3)), 0.5 * amplitude_scale, 8.0, 1.5);
    total += gerstner_wave_normal(wave1, position, time);
    
    let wave2 = GerstnerWave(normalize(vec2<f32>(-0.7, 1.0)), 0.4 * amplitude_scale, 5.0, 1.8);
    total += gerstner_wave_normal(wave2, position, time);
    
    let wave3 = GerstnerWave(normalize(vec2<f32>(0.5, -1.0)), 0.3 * amplitude_scale, 3.0, 2.2);
    total += gerstner_wave_normal(wave3, position, time);
    
    let wave4 = GerstnerWave(normalize(vec2<f32>(-1.0, -0.5)), 0.2 * amplitude_scale, 1.5, 2.8);
    total += gerstner_wave_normal(wave4, position, time);
    
    return normalize(total);
}

// ============================================================================
// Fragment Shader - Custom Water Rendering
// ============================================================================

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    // Get world position and calculate wave-based effects
    let world_pos = in.world_position.xyz;
    let position_2d = world_pos.xz * water_material.coord_scale + water_material.coord_offset;
    let time = globals.time;
    
    // Calculate animated normal from Gerstner waves
    let wave_normal = calculate_total_normal(position_2d, time, water_material.amplitude);
    
    // Get PBR input from base material
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    
    // Apply animated normal to PBR input
    pbr_input.N = wave_normal;
    pbr_input.world_normal = wave_normal;
    
    // Calculate view-dependent effects
    let view_dir = normalize(pbr_input.V);
    let ndotv = max(dot(wave_normal, view_dir), 0.0);
    
    // Fresnel effect for water reflectivity
    let fresnel = pow(1.0 - ndotv, 3.0);
    
    // Mix between shallow and deep water colors based on view angle
    let depth_factor = 1.0 - ndotv;
    var water_color = mix(
        water_material.shallow_color.rgb,
        water_material.deep_color.rgb,
        depth_factor * 0.5
    );
    
    // Add sky reflection using fresnel
    let sky_color = vec3<f32>(0.5, 0.7, 0.9);
    water_color = mix(water_color, sky_color, fresnel * 0.6);
    
    // Apply clarity - more clarity means more of the underlying color shows through
    let alpha = mix(0.9, water_material.shallow_color.a, water_material.clarity);
    
    // Simple specular highlight for sun reflection
    let sun_dir = normalize(vec3<f32>(0.3, 0.8, 0.5));
    let half_vec = normalize(sun_dir + view_dir);
    let spec = pow(max(dot(wave_normal, half_vec), 0.0), 128.0);
    water_color += vec3<f32>(1.0, 1.0, 0.9) * spec * 0.5;
    
    // Output final color
    var out: FragmentOutput;
    out.color = vec4<f32>(water_color, alpha);
    
    return out;
}

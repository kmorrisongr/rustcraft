// Gerstner Waves Water Shader for Rustcraft
// Implements Gerstner wave animation for water surfaces
//
// Based on GPU Gems Chapter 1: Effective Water Simulation from Physical Models
// https://developer.nvidia.com/gpugems/gpugems/part-i-natural-effects/chapter-1-effective-water-simulation-physical-models
//
// This shader extends Bevy's StandardMaterial using the MaterialExtension system.

#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::globals,
    mesh_view_bindings::view,
}

// ============================================================================
// Constants
// ============================================================================

const PI: f32 = 3.14159265359;

// Water colors (hardcoded for now to avoid uniform alignment issues)
const SHALLOW_COLOR: vec3<f32> = vec3<f32>(0.15, 0.35, 0.45);
const DEEP_COLOR: vec3<f32> = vec3<f32>(0.05, 0.15, 0.25);
const SKY_COLOR: vec3<f32> = vec3<f32>(0.5, 0.7, 0.9);
const WATER_ALPHA: f32 = 0.8;
const AMPLITUDE_SCALE: f32 = 0.5;

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
fn calculate_total_normal(position: vec2<f32>, time: f32) -> vec3<f32> {
    var total = vec3<f32>(0.0, 1.0, 0.0);
    
    let wave1 = GerstnerWave(normalize(vec2<f32>(1.0, 0.3)), 0.5 * AMPLITUDE_SCALE, 8.0, 1.5);
    total += gerstner_wave_normal(wave1, position, time);
    
    let wave2 = GerstnerWave(normalize(vec2<f32>(-0.7, 1.0)), 0.4 * AMPLITUDE_SCALE, 5.0, 1.8);
    total += gerstner_wave_normal(wave2, position, time);
    
    let wave3 = GerstnerWave(normalize(vec2<f32>(0.5, -1.0)), 0.3 * AMPLITUDE_SCALE, 3.0, 2.2);
    total += gerstner_wave_normal(wave3, position, time);
    
    let wave4 = GerstnerWave(normalize(vec2<f32>(-1.0, -0.5)), 0.2 * AMPLITUDE_SCALE, 1.5, 2.8);
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
    // Get world position and time
    let world_pos = in.world_position.xyz;
    let position_2d = world_pos.xz;
    let time = globals.time;
    
    // Calculate animated normal from Gerstner waves
    let wave_normal = calculate_total_normal(position_2d, time);
    
    // Calculate view direction
    let view_dir = normalize(view.world_position.xyz - world_pos);
    let ndotv = max(dot(wave_normal, view_dir), 0.0);
    
    // Fresnel effect for water reflectivity
    let fresnel = pow(1.0 - ndotv, 3.0);
    
    // Mix between shallow and deep water colors based on view angle
    let depth_factor = 1.0 - ndotv;
    var water_color = mix(SHALLOW_COLOR, DEEP_COLOR, depth_factor * 0.5);
    
    // Add sky reflection using fresnel
    water_color = mix(water_color, SKY_COLOR, fresnel * 0.6);
    
    // Simple specular highlight for sun reflection
    let sun_dir = normalize(vec3<f32>(0.3, 0.8, 0.5));
    let half_vec = normalize(sun_dir + view_dir);
    let spec = pow(max(dot(wave_normal, half_vec), 0.0), 128.0);
    water_color += vec3<f32>(1.0, 1.0, 0.9) * spec * 0.5;
    
    // Output final color with alpha
    var out: FragmentOutput;
    out.color = vec4<f32>(water_color, WATER_ALPHA);
    
    return out;
}

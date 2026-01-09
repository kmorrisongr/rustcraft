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
// Uniforms - Water colors passed from CPU
// ============================================================================

// Water color uniforms - using vec4 for proper 16-byte alignment
// RGB channels hold the color, alpha channel holds additional parameters
struct WaterColors {
    // xyz = shallow water color, w = water alpha
    shallow_color: vec4<f32>,
    // xyz = deep water color, w = amplitude scale  
    deep_color: vec4<f32>,
    // xyz = sky reflection color, w = unused (padding)
    sky_color: vec4<f32>,
}

@group(2) @binding(100)
var<uniform> water_colors: WaterColors;

// ============================================================================
// Constants
// ============================================================================

const PI: f32 = 3.14159265359;
const TWO_PI: f32 = 6.28318530718;

// Precomputed normalized wave directions (avoids per-fragment normalize calls)
const WAVE_DIR_1: vec2<f32> = vec2<f32>(0.9578262, 0.2873478);   // normalize(vec2(1.0, 0.3))
const WAVE_DIR_2: vec2<f32> = vec2<f32>(-0.5734623, 0.8192319);  // normalize(vec2(-0.7, 1.0))
const WAVE_DIR_3: vec2<f32> = vec2<f32>(0.4472136, -0.8944272);  // normalize(vec2(0.5, -1.0))
const WAVE_DIR_4: vec2<f32> = vec2<f32>(-0.8944272, -0.4472136); // normalize(vec2(-1.0, -0.5))

// Precomputed wave numbers (k = 2π / wavelength)
const WAVE_K_1: f32 = 0.7853982;  // 2π / 8.0
const WAVE_K_2: f32 = 1.2566371;  // 2π / 5.0
const WAVE_K_3: f32 = 2.0943951;  // 2π / 3.0
const WAVE_K_4: f32 = 4.1887902;  // 2π / 1.5

// Wave speeds
const WAVE_SPEED_1: f32 = 1.5;
const WAVE_SPEED_2: f32 = 1.8;
const WAVE_SPEED_3: f32 = 2.2;
const WAVE_SPEED_4: f32 = 2.8;

// Base steepness values (will be scaled by amplitude_scale)
const WAVE_STEEPNESS_1: f32 = 0.5;
const WAVE_STEEPNESS_2: f32 = 0.4;
const WAVE_STEEPNESS_3: f32 = 0.3;
const WAVE_STEEPNESS_4: f32 = 0.2;

// Sum of all steepness values for height normalization
const TOTAL_STEEPNESS: f32 = 1.4;  // 0.5 + 0.4 + 0.3 + 0.2

// ============================================================================
// Gerstner Wave Output Structure
// ============================================================================

struct WaveResult {
    normal: vec3<f32>,
    height: f32,
}

/// Calculate both normal and height contribution from a single wave
/// This avoids redundant phase and trig calculations
fn gerstner_wave(
    direction: vec2<f32>,
    k: f32,
    speed: f32,
    steepness: f32,
    position: vec2<f32>,
    time: f32
) -> WaveResult {
    let omega = k * speed;
    let phase = k * dot(direction, position) - omega * time;
    
    // Compute sin and cos once per wave
    let sin_phase = sin(phase);
    let cos_phase = cos(phase);
    
    var result: WaveResult;
    result.height = steepness * sin_phase;
    result.normal = vec3<f32>(
        -direction.x * steepness * cos_phase,
        1.0 - steepness * sin_phase,
        -direction.y * steepness * cos_phase
    );
    return result;
}

/// Calculate combined normal and height from all waves in a single pass
fn calculate_waves(position: vec2<f32>, time: f32) -> WaveResult {
    let amplitude_scale = water_colors.deep_color.w;
    
    var total_normal = vec3<f32>(0.0, 1.0, 0.0);
    var total_height: f32 = 0.0;
    
    // Wave 1
    let w1 = gerstner_wave(WAVE_DIR_1, WAVE_K_1, WAVE_SPEED_1, WAVE_STEEPNESS_1 * amplitude_scale, position, time);
    total_normal += w1.normal;
    total_height += w1.height;
    
    // Wave 2
    let w2 = gerstner_wave(WAVE_DIR_2, WAVE_K_2, WAVE_SPEED_2, WAVE_STEEPNESS_2 * amplitude_scale, position, time);
    total_normal += w2.normal;
    total_height += w2.height;
    
    // Wave 3
    let w3 = gerstner_wave(WAVE_DIR_3, WAVE_K_3, WAVE_SPEED_3, WAVE_STEEPNESS_3 * amplitude_scale, position, time);
    total_normal += w3.normal;
    total_height += w3.height;
    
    // Wave 4
    let w4 = gerstner_wave(WAVE_DIR_4, WAVE_K_4, WAVE_SPEED_4, WAVE_STEEPNESS_4 * amplitude_scale, position, time);
    total_normal += w4.normal;
    total_height += w4.height;
    
    var result: WaveResult;
    result.normal = normalize(total_normal);
    // Normalize height to 0-1 range
    let max_height = TOTAL_STEEPNESS * amplitude_scale;
    result.height = (total_height / max_height + 1.0) * 0.5;
    return result;
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
    
    // Calculate normal and height in a single pass
    let waves = calculate_waves(position_2d, time);
    let wave_normal = waves.normal;
    let wave_height = waves.height;
    
    // Calculate view direction
    let view_dir = normalize(view.world_position.xyz - world_pos);
    let ndotv = max(dot(wave_normal, view_dir), 0.0);
    
    // Fresnel effect for water reflectivity
    let fresnel = pow(1.0 - ndotv, 3.0);
    
    // Extract colors from uniforms (RGB channels)
    let shallow_color = water_colors.shallow_color.xyz;
    let deep_color = water_colors.deep_color.xyz;
    let sky_color = water_colors.sky_color.xyz;
    let water_alpha = water_colors.shallow_color.w;
    
    // Mix between deep and shallow water colors based on wave height
    // Wave troughs (low height) get deep color, crests (high height) get shallow color
    var water_color = mix(deep_color, shallow_color, wave_height);
    
    // Add sky reflection using fresnel
    water_color = mix(water_color, sky_color, fresnel * 0.6);
    
    // Simple specular highlight for sun reflection
    let sun_dir = normalize(vec3<f32>(0.3, 0.8, 0.5));
    let half_vec = normalize(sun_dir + view_dir);
    let spec = pow(max(dot(wave_normal, half_vec), 0.0), 128.0);
    water_color += vec3<f32>(1.0, 1.0, 0.9) * spec * 0.5;
    
    // Output final color with alpha from uniform
    var out: FragmentOutput;
    out.color = vec4<f32>(water_color, water_alpha);
    
    return out;
}

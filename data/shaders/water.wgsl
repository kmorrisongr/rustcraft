// Water shader with standing wave animation
// 
// This shader creates animated standing waves on water surfaces using
// sine waves with multiple frequencies for a natural look.

#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
}

// ============================================================================
// Water material uniforms - matches WaterMaterial struct in water.rs
// ============================================================================
struct WaterUniforms {
    time: f32,
    wave_amplitude: f32,
    wave_frequency: f32,
    wave_speed: f32,
    base_color: vec4<f32>,
    deep_color: vec4<f32>,
}

@group(2) @binding(0)
var<uniform> water: WaterUniforms;

@group(2) @binding(1)
var water_texture: texture_2d<f32>;
@group(2) @binding(2)
var water_sampler: sampler;

// ============================================================================
// Noise functions for natural variation
// ============================================================================

// Simple hash function for pseudo-random values
fn hash(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

// Smooth noise interpolation
fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f); // smoothstep
    
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Fractal Brownian Motion - layered noise for natural look
fn fbm(p: vec2<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var pos = p;
    
    for (var i = 0; i < 4; i++) {
        value += amplitude * noise(pos);
        pos *= 2.0;
        amplitude *= 0.5;
    }
    return value;
}

// ============================================================================
// Wave calculation functions
// ============================================================================

// Calculate a single traveling wave with noise perturbation
fn traveling_wave(pos: vec2<f32>, dir: vec2<f32>, freq: f32, speed: f32, time: f32, phase_offset: f32) -> f32 {
    let d = normalize(dir);
    let phase = dot(pos, d) * freq - time * speed + phase_offset;
    return sin(phase);
}

// Calculate combined wave height using multiple traveling waves
fn calculate_wave_height(pos: vec2<f32>, time: f32) -> f32 {
    let freq = water.wave_frequency;
    let speed = water.wave_speed;
    
    // Add subtle noise-based position offset to break patterns
    let noise_offset = vec2<f32>(
        fbm(pos * 0.1 + time * 0.05) - 0.5,
        fbm(pos * 0.1 + vec2<f32>(50.0, 50.0) + time * 0.05) - 0.5
    ) * 0.8;
    let perturbed_pos = pos + noise_offset;
    
    var height = 0.0;
    
    // Use more waves with irrational-ish frequency ratios to avoid patterns
    // Primary waves - large scale
    height += traveling_wave(perturbed_pos, vec2<f32>(1.0, 0.2), freq * 0.7, speed * 0.8, time, 0.0) * 0.30;
    height += traveling_wave(perturbed_pos, vec2<f32>(-0.3, 1.0), freq * 0.9, speed * 0.95, time, 1.3) * 0.25;
    
    // Secondary waves - medium scale
    height += traveling_wave(perturbed_pos, vec2<f32>(0.8, -0.6), freq * 1.3, speed * 1.1, time, 2.7) * 0.18;
    height += traveling_wave(perturbed_pos, vec2<f32>(-0.7, -0.4), freq * 1.7, speed * 1.25, time, 4.1) * 0.12;
    
    // Detail waves - small scale, faster
    height += traveling_wave(perturbed_pos, vec2<f32>(0.5, 0.85), freq * 2.3, speed * 1.5, time, 5.3) * 0.08;
    height += traveling_wave(perturbed_pos, vec2<f32>(-0.9, 0.3), freq * 3.1, speed * 1.7, time, 6.7) * 0.05;
    
    // Add subtle noise variation to final height
    height += (fbm(pos * 0.3 + time * 0.1) - 0.5) * 0.04;
    
    return height * water.wave_amplitude;
}

// Calculate wave normal using finite differences
fn calculate_wave_normal(pos: vec2<f32>, time: f32) -> vec3<f32> {
    let epsilon = 0.08;
    let height_center = calculate_wave_height(pos, time);
    let height_x = calculate_wave_height(pos + vec2<f32>(epsilon, 0.0), time);
    let height_z = calculate_wave_height(pos + vec2<f32>(0.0, epsilon), time);
    let dx = (height_x - height_center) / epsilon;
    let dz = (height_z - height_center) / epsilon;
    return normalize(vec3<f32>(-dx, 1.0, -dz));
}

// ============================================================================
// Vertex shader
// ============================================================================
@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    
    // Get model matrix using Bevy's mesh functions
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    
    // Calculate world position
    var world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0)
    );
    
    // Apply wave displacement only to top faces (normal pointing up)
#ifdef VERTEX_NORMALS
    let is_top_face = vertex.normal.y > 0.5;
    if is_top_face {
        let wave_height = calculate_wave_height(world_position.xz, water.time);
        world_position.y += wave_height;
    }
    
    // Transform normal
    if is_top_face {
        out.world_normal = calculate_wave_normal(world_position.xz, water.time);
    } else {
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
// Fragment shader
// ============================================================================
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate wave height for shading effects
    let wave_height = calculate_wave_height(in.world_position.xz, water.time);
    let height_factor = clamp(wave_height / water.wave_amplitude * 0.5 + 0.5, 0.0, 1.0);
    
    // Base color with depth variation - troughs are darker
    var color = mix(water.deep_color.rgb, water.base_color.rgb, height_factor * 0.6 + 0.4);

    // Directional light (sun-like, from upper right)
    let light_dir = normalize(vec3<f32>(0.6, 0.75, 0.35));
    
#ifdef VERTEX_NORMALS
    // Normal-based lighting for wave visibility
    let ndotl = max(dot(in.world_normal, light_dir), 0.0);
    
    // Calculate slope for shading
    let slope = 1.0 - in.world_normal.y;
    
    // Lighting with wave-aware contrast
    let lighting = 0.55 + ndotl * 0.5 + slope * 0.1;
#else
    let lighting = 0.75;
#endif
    
    color *= lighting;
    
#ifdef VERTEX_NORMALS
    let view_dir = normalize(-in.world_position.xyz);
    
    // Foam with noise for natural variation - no more spotty pattern
    let foam_noise = fbm(in.world_position.xz * 0.5 + water.time * 0.15);
    let foam_threshold = 0.65 + foam_noise * 0.2; // Varying threshold
    let foam_intensity = smoothstep(foam_threshold, foam_threshold + 0.25, height_factor);
    // Add turbulence-based foam in steep areas
    let turbulence_foam = slope * foam_noise * 0.4;
    let total_foam = clamp(foam_intensity + turbulence_foam, 0.0, 1.0);
    let foam_color = vec3<f32>(0.82, 0.88, 0.92);
    color = mix(color, foam_color, total_foam * 0.5);
    
    // Specular highlights with noise variation for natural sun glints
    let half_vec = normalize(light_dir + view_dir);
    let spec_noise = noise(in.world_position.xz * 2.0 + water.time * 0.5);
    let spec = pow(max(dot(in.world_normal, half_vec), 0.0), 80.0);
    color += vec3<f32>(1.0, 0.98, 0.92) * spec * (0.5 + spec_noise * 0.4);
    
    // Broader specular for overall shine
    let broad_spec = pow(max(dot(in.world_normal, half_vec), 0.0), 12.0);
    color += vec3<f32>(0.65, 0.75, 0.85) * broad_spec * 0.12;
    
    // Fresnel effect - water appears lighter at glancing angles
    let fresnel = pow(1.0 - max(dot(normalize(view_dir), in.world_normal), 0.0), 3.5);
    color = mix(color, vec3<f32>(0.55, 0.70, 0.78), fresnel * 0.3);
#endif
    
    // Return with transparency
    return vec4<f32>(color, water.base_color.a);
}

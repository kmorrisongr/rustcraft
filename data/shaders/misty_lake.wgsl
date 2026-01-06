// Misty Lake Water Shader - WGSL Port
// Original: Created by Reinder Nijhoff 2013
// Creative Commons Attribution-NonCommercial-ShareAlike 4.0 International License.
// @reindernijhoff
// https://www.shadertoy.com/view/MsB3WR
//
// Simplified for water material use in Rustcraft

#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
}
#import bevy_render::view::View

@group(0) @binding(0)
var<uniform> view: View;

// ============================================================================
// Constants
// ============================================================================
const BUMPFACTOR: f32 = 0.2;
const EPSILON: f32 = 0.05;
const BUMPDISTANCE: f32 = 150.0;  // Distance at which bump fades (in world units)

// Rotation matrix (creates swirling pattern)
const M2: mat2x2<f32> = mat2x2<f32>(0.60, -0.80, 0.80, 0.60);

// 3D rotation matrix for fbm
const M3: mat3x3<f32> = mat3x3<f32>(
    vec3<f32>(0.00, 0.80, 0.60),
    vec3<f32>(-0.80, 0.36, -0.48),
    vec3<f32>(-0.60, -0.48, 0.64)
);

// ============================================================================
// Uniforms
// ============================================================================
struct MistyWaterUniforms {
    time: f32,
    wave_scale: f32,        // Controls wave size (default ~8.0)
    bump_strength: f32,     // Controls normal perturbation (default ~0.1)
    _padding1: f32,
    water_color: vec4<f32>, // Base water color with alpha
    deep_color: vec4<f32>,  // Deep water color
    sun_direction: vec3<f32>,  // Dynamic sun direction for reflections
    fog_density: f32,       // Distance fog factor
    moon_direction: vec3<f32>, // Dynamic moon direction for night reflections
    _padding2: f32,
}

@group(2) @binding(0)
var<uniform> water: MistyWaterUniforms;

@group(2) @binding(1)
var noise_texture: texture_2d<f32>;     // 256x256 noise texture
@group(2) @binding(2)
var noise_sampler: sampler;

// ============================================================================
// Noise functions (by Inigo Quilez)
// ============================================================================

fn hash(n: f32) -> f32 {
    return fract(sin(n) * 43758.5453);
}

fn hash2d(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn noise2d(x: vec2<f32>) -> f32 {
    let p = floor(x);
    let f = fract(x);
    let u = f * f * (3.0 - 2.0 * f);
    
    let uv = p.xy + u.xy;
    return textureSampleLevel(noise_texture, noise_sampler, (uv + 0.5) / 256.0, 0.0).x;
}

// Fallback noise when texture unavailable
fn noise2d_procedural(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    
    let a = hash2d(i);
    let b = hash2d(i + vec2<f32>(1.0, 0.0));
    let c = hash2d(i + vec2<f32>(0.0, 1.0));
    let d = hash2d(i + vec2<f32>(1.0, 1.0));
    
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn noise3d(x: vec3<f32>) -> f32 {
    let p = floor(x);
    let f = fract(x);
    let u = f * f * (3.0 - 2.0 * f);
    
    let uv = p.xy + vec2<f32>(37.0, 17.0) * p.z + u.xy;
    let rg = textureSampleLevel(noise_texture, noise_sampler, (uv + 0.5) / 256.0, 0.0).yx;
    return mix(rg.x, rg.y, u.z);
}

// Fallback 3D noise
fn noise3d_procedural(x: vec3<f32>) -> f32 {
    let p = floor(x);
    let f = fract(x);
    let u = f * f * (3.0 - 2.0 * f);
    
    let n = p.x + p.y * 157.0 + 113.0 * p.z;
    return mix(
        mix(mix(hash(n + 0.0), hash(n + 1.0), u.x),
            mix(hash(n + 157.0), hash(n + 158.0), u.x), u.y),
        mix(mix(hash(n + 113.0), hash(n + 114.0), u.x),
            mix(hash(n + 270.0), hash(n + 271.0), u.x), u.y),
        u.z
    );
}

// Fractal Brownian Motion - creates natural-looking turbulence
fn fbm(p_in: vec3<f32>) -> f32 {
    var p = p_in;
    var f = 0.0;
    f += 0.5000 * noise3d_procedural(p); p = M3 * p * 2.02;
    f += 0.2500 * noise3d_procedural(p); p = M3 * p * 2.03;
    f += 0.1250 * noise3d_procedural(p); p = M3 * p * 2.01;
    f += 0.0625 * noise3d_procedural(p);
    return f / 0.9375;
}

// ============================================================================
// Water surface functions
// ============================================================================

// Generates the water height displacement
fn water_height(pos: vec2<f32>) -> f32 {
    let posm = pos * M2;
    let scale = water.wave_scale;
    return abs(fbm(vec3<f32>(scale * posm, water.time)) - 0.5) * 0.1;
}

// Calculate water normal from height field using finite differences
fn water_normal(pos: vec2<f32>, bump_strength: f32) -> vec3<f32> {
    let dx = vec2<f32>(EPSILON, 0.0);
    let dz = vec2<f32>(0.0, EPSILON);
    
    var normal = vec3<f32>(0.0, 1.0, 0.0);
    normal.x = -bump_strength * (water_height(pos + dx) - water_height(pos - dx)) / (2.0 * EPSILON);
    normal.z = -bump_strength * (water_height(pos + dz) - water_height(pos - dz)) / (2.0 * EPSILON);
    return normalize(normal);
}

// Sky/environment color for reflections - uses dynamic sun/moon positions
fn sky_color(rd: vec3<f32>, sun_dir: vec3<f32>, moon_dir: vec3<f32>) -> vec3<f32> {
    // Sun contribution (warm golden light)
    let sun = clamp(dot(sun_dir, rd), 0.0, 1.0);
    // Moon contribution (cool blue-white light)
    let moon = clamp(dot(moon_dir, rd), 0.0, 1.0);
    
    // Determine day/night blend based on sun height
    let day_factor = clamp(sun_dir.y + 0.2, 0.0, 1.0);
    
    // Daytime sky color
    var day_col = vec3<f32>(0.5, 0.52, 0.55) - rd.y * 0.2 * vec3<f32>(1.0, 0.8, 1.0) + 0.1125;
    day_col += vec3<f32>(1.0, 0.6, 0.1) * pow(sun, 8.0); // Sun glow
    
    // Nighttime sky color (darker, bluish)
    var night_col = vec3<f32>(0.05, 0.08, 0.15) - rd.y * 0.1 * vec3<f32>(0.3, 0.4, 0.6);
    night_col += vec3<f32>(0.6, 0.7, 0.9) * pow(moon, 12.0) * 0.3; // Moon glow (softer)
    
    // Blend between day and night
    var col = mix(night_col, day_col, day_factor);
    col *= 0.95;
    return col;
}

// ============================================================================
// Vertex shader
// ============================================================================
@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    
    var world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0)
    );
    
    // Apply wave displacement to top faces
#ifdef VERTEX_NORMALS
    let is_top_face = vertex.normal.y > 0.5;
    if is_top_face {
        let height = water_height(world_position.xz);
        world_position.y += height * water.bump_strength * 2.0;
        
        // Use perturbed normal for lighting
        out.world_normal = water_normal(world_position.xz, water.bump_strength);
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
    let world_pos = in.world_position.xyz;
    
    // Get camera position from Bevy's view uniform
    let camera_pos = view.world_position;
    
    // Calculate proper view direction from camera to fragment
    let view_dir = normalize(camera_pos - world_pos);
    
    // Distance from camera for LOD and fog
    let dist = length(camera_pos - world_pos);
    
    // Get water normal - recalculate in fragment for better quality
    // Use stronger bump at close range, fade with distance
    let bump_atten = 1.0 - smoothstep(0.0, BUMPDISTANCE, dist);
    let bump_strength = water.bump_strength * bump_atten;
    let normal = water_normal(world_pos.xz, bump_strength);
    
    // Fresnel effect - water is more reflective at glancing angles
    let ndotv = max(dot(normal, view_dir), 0.0);
    let fresnel = pow(1.0 - ndotv, 5.0);
    
    // Get light directions from uniforms
    let sun_dir = normalize(water.sun_direction);
    let moon_dir = normalize(water.moon_direction);
    
    // Reflection direction
    let reflect_dir = reflect(-view_dir, normal);
    
    // Sky reflection color (now uses dynamic sun/moon positions)
    let reflect_col = sky_color(reflect_dir, sun_dir, moon_dir);
    
    // Base water color (from uniforms)
    let base_col = water.water_color.rgb;
    let deep_col = water.deep_color.rgb;
    
    // Blend between deep and surface color based on view angle
    let depth_factor = clamp(1.0 - ndotv, 0.0, 1.0);
    var water_col = mix(base_col, deep_col, depth_factor * 0.5);
    
    // Combine reflection and water color using fresnel
    // Higher fresnel influence for more reflective water like the original
    var col = mix(water_col, reflect_col, fresnel * 0.85 + 0.1);
    
    // Add subtle refraction tint (underwater color showing through)
    col += deep_col * (1.0 - fresnel) * 0.1;
    
    // Day/night factor for specular intensity
    let day_factor = clamp(sun_dir.y + 0.2, 0.0, 1.0);
    
    // Sun specular highlight (sun glint) - brighter and sharper during day
    let sun_half_vec = normalize(sun_dir + view_dir);
    let sun_spec = pow(max(dot(normal, sun_half_vec), 0.0), 96.0);
    col += vec3<f32>(1.0, 0.9, 0.7) * sun_spec * 1.2 * day_factor;
    
    // Moon specular highlight - softer, blue-tinted at night
    let moon_half_vec = normalize(moon_dir + view_dir);
    let moon_spec = pow(max(dot(normal, moon_half_vec), 0.0), 64.0);
    col += vec3<f32>(0.6, 0.7, 0.9) * moon_spec * 0.4 * (1.0 - day_factor);
    
    // Broader specular for overall shine (blended between sun and moon)
    let broad_spec_sun = pow(max(dot(normal, sun_half_vec), 0.0), 16.0);
    let broad_spec_moon = pow(max(dot(normal, moon_half_vec), 0.0), 16.0);
    col += vec3<f32>(0.8, 0.85, 0.9) * broad_spec_sun * 0.2 * day_factor;
    col += vec3<f32>(0.5, 0.55, 0.7) * broad_spec_moon * 0.1 * (1.0 - day_factor);
    
    // Distance fog - blend toward sky color at distance
    let fog_factor = 1.0 - exp(-water.fog_density * dist * dist);
    let fog_color = sky_color(vec3<f32>(0.0, 0.2, 1.0), sun_dir, moon_dir);
    col = mix(col, fog_color, clamp(fog_factor, 0.0, 0.5));
    
    return vec4<f32>(col, water.water_color.a);
}

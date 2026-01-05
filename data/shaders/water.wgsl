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
// Wave calculation functions
// ============================================================================

// Calculate standing wave height at a given position and time
fn calculate_wave_height(pos: vec2<f32>, time: f32) -> f32 {
    let freq1 = water.wave_frequency;
    let freq2 = water.wave_frequency * 1.7;
    let freq3 = water.wave_frequency * 0.5;
    
    let speed1 = water.wave_speed;
    let speed2 = water.wave_speed * 0.8;
    let speed3 = water.wave_speed * 1.2;
    
    // Standing waves: sin(kx) * cos(wt)
    let wave1 = sin(pos.x * freq1) * cos(time * speed1);
    let wave2 = sin(pos.y * freq2 + 1.5) * cos(time * speed2 + 0.5);
    let wave3 = sin((pos.x + pos.y) * freq3) * cos(time * speed3);
    
    let combined = wave1 * 0.5 + wave2 * 0.3 + wave3 * 0.2;
    return combined * water.wave_amplitude;
}

// Calculate wave normal for lighting
fn calculate_wave_normal(pos: vec2<f32>, time: f32) -> vec3<f32> {
    let epsilon = 0.1;
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
    // Base water color
    var color = water.base_color.rgb;
    
#ifdef VERTEX_COLORS
    // Mix with vertex color if available
    color = mix(color, in.color.rgb * water.base_color.rgb, 0.5);
#endif

    // Simple directional lighting
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    
#ifdef VERTEX_NORMALS
    let ndotl = max(dot(in.world_normal, light_dir), 0.0);
#else
    let ndotl = 0.8;
#endif
    
    let lighting = 0.4 + ndotl * 0.6;
    color *= lighting;
    
    // Add some specular highlight
#ifdef VERTEX_NORMALS
    let view_dir = normalize(-in.world_position.xyz);
    let half_vec = normalize(light_dir + view_dir);
    let spec = pow(max(dot(in.world_normal, half_vec), 0.0), 32.0);
    color += vec3<f32>(1.0) * spec * 0.3;
#endif
    
    // Return with alpha for transparency
    return vec4<f32>(color, water.base_color.a);
}

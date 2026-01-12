//! Water mesh generation for rendering.
//!
//! This module generates triangle meshes from water surface data.
//! Meshes are generated per-chunk and optimized for the Gerstner wave shader.
//!
//! ## Mesh Structure
//! - Each water surface cell generates vertices for a quad at the water level
//! - Adjacent cells share edges where possible for smoother wave deformation
//! - UV coordinates map to world position for consistent wave patterns
//! - Vertex colors encode water volume and wave scale:
//!   - Red channel: wave scale (0.0 = flat, 1.0 = full waves)
//!   - Green/Blue: fixed depth hint colors
//!   - Alpha: volume/transparency

#![allow(unused_variables)] // Some parameters reserved for future shader integration

use bevy::{
    math::IVec3,
    prelude::*,
    render::mesh::{Indices, Mesh, PrimitiveTopology},
};
use shared::world::{
    calculate_wave_scale, BlockTransparency, ChunkWaterStorage, WaveScaleConfig, WorldMap,
    FULL_WATER_HEIGHT,
};
use shared::CHUNK_SIZE;

use crate::world::ClientWorldMap;

/// Data needed to generate a water mesh for a chunk.
pub struct WaterMeshInput<'a> {
    /// Chunk position in chunk coordinates
    pub chunk_pos: IVec3,
    /// Water storage for this chunk
    pub water: &'a ChunkWaterStorage,
    /// Reference to the world map for neighbor lookups
    pub world_map: &'a ClientWorldMap,
    /// Configuration for wave scale calculation
    pub wave_scale_config: WaveScaleConfig,
    /// Number of subdivisions per water cell for tessellation
    pub tessellation: u32,
    /// LOD tessellation level for distant water
    pub tessellation_lod: u32,
}

/// Generated water mesh data ready for GPU upload.
#[derive(Debug, Default, Clone)]
pub struct WaterMeshData {
    /// Vertex positions (local to chunk)
    pub positions: Vec<[f32; 3]>,
    /// Vertex normals (initially up, modified by shader)
    pub normals: Vec<[f32; 3]>,
    /// UV coordinates (world X, Z for wave alignment)
    pub uvs: Vec<[f32; 2]>,
    /// Vertex colors (encode volume/depth info)
    pub colors: Vec<[f32; 4]>,
    /// Triangle indices
    pub indices: Vec<u32>,
}

impl WaterMeshData {
    /// Creates a new empty mesh data container.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if this mesh has no geometry.
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Converts this data into a Bevy mesh.
    pub fn into_mesh(self) -> Option<Mesh> {
        if self.is_empty() {
            return None;
        }

        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, Default::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_indices(Indices::U32(self.indices));

        // Generate tangents for normal mapping (if needed in future)
        if let Err(e) = mesh.generate_tangents() {
            warn!("Failed to generate tangents for water mesh: {:?}", e);
        }

        Some(mesh)
    }
}

/// Face directions for water surface quads
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterFace {
    Top,    // Y+ (primary surface)
    Side,   // X+, X-, Z+, Z- edges
    Bottom, // Y- (underwater view)
}

/// Generates a water mesh for a chunk.
///
/// This creates a mesh from all water surface cells in the chunk.
/// The mesh is optimized for the Gerstner wave shader.
///
/// # Arguments
/// * `input` - Data needed for mesh generation
///
/// # Returns
/// * `Option<WaterMeshData>` - Mesh data if there's water to render, None otherwise
pub fn generate_water_mesh(input: &WaterMeshInput) -> Option<WaterMeshData> {
    if input.water.is_empty() {
        return None;
    }

    let mut data = WaterMeshData::new();
    let chunk_world_x = input.chunk_pos.x * CHUNK_SIZE;
    let chunk_world_z = input.chunk_pos.z * CHUNK_SIZE;

    // Process each water cell
    for (local_pos, cell) in input.water.iter() {
        let volume = cell.volume();
        let surface_height = cell.surface_height();

        // Calculate wave scale based on local water volume
        let wave_scale = calculate_wave_scale(input.water, local_pos, &input.wave_scale_config);

        // World position for UV coordinates
        let world_x = chunk_world_x + local_pos.x;
        let world_z = chunk_world_z + local_pos.z;

        // Local position within chunk (for mesh vertices)
        let x = local_pos.x as f32;
        let y = local_pos.y as f32 + surface_height;
        let z = local_pos.z as f32;

        // Check neighbors to determine which faces to render
        let should_render_top = should_render_top_face(input, local_pos);
        let should_render_sides = should_render_side_faces(input, local_pos);

        // Generate top face (main water surface)
        if should_render_top {
            add_top_face(
                &mut data,
                x,
                y,
                z,
                world_x as f32,
                world_z as f32,
                volume,
                wave_scale,
                input.tessellation,
            );
        }

        // Generate side faces where water meets air
        if should_render_sides.x_pos {
            add_side_face_x_pos(
                &mut data,
                x,
                local_pos.y as f32,
                z,
                surface_height,
                world_x as f32,
                world_z as f32,
                volume,
                wave_scale,
            );
        }
        if should_render_sides.x_neg {
            add_side_face_x_neg(
                &mut data,
                x,
                local_pos.y as f32,
                z,
                surface_height,
                world_x as f32,
                world_z as f32,
                volume,
                wave_scale,
            );
        }
        if should_render_sides.z_pos {
            add_side_face_z_pos(
                &mut data,
                x,
                local_pos.y as f32,
                z,
                surface_height,
                world_x as f32,
                world_z as f32,
                volume,
                wave_scale,
            );
        }
        if should_render_sides.z_neg {
            add_side_face_z_neg(
                &mut data,
                x,
                local_pos.y as f32,
                z,
                surface_height,
                world_x as f32,
                world_z as f32,
                volume,
                wave_scale,
            );
        }
    }

    if data.is_empty() {
        None
    } else {
        Some(data)
    }
}

/// Determines if the top face should be rendered.
/// Top face is rendered if there's no solid block and no water directly above.
fn should_render_top_face(input: &WaterMeshInput, local_pos: &IVec3) -> bool {
    let above_local = *local_pos + IVec3::new(0, 1, 0);

    // Check if there's water above (don't render surface between water layers)
    if input.water.has_water(&above_local) {
        return false;
    }

    // Check if there's a solid block above
    let global_pos = IVec3::new(
        input.chunk_pos.x * CHUNK_SIZE + local_pos.x,
        input.chunk_pos.y * CHUNK_SIZE + local_pos.y + 1,
        input.chunk_pos.z * CHUNK_SIZE + local_pos.z,
    );

    if let Some(block) = input.world_map.get_block_by_coordinates(&global_pos) {
        // Solid blocks block water visibility
        if block.id.get_visibility() == BlockTransparency::Solid {
            return false;
        }
    }

    true
}

/// Side face rendering flags
#[derive(Debug, Default)]
struct SideFaces {
    x_pos: bool,
    x_neg: bool,
    z_pos: bool,
    z_neg: bool,
}

/// Determines which side faces should be rendered.
/// Side faces are rendered where water meets air (not solid and not water).
fn should_render_side_faces(input: &WaterMeshInput, local_pos: &IVec3) -> SideFaces {
    let mut faces = SideFaces::default();

    let offsets = [
        (IVec3::new(1, 0, 0), &mut faces.x_pos),
        (IVec3::new(-1, 0, 0), &mut faces.x_neg),
        (IVec3::new(0, 0, 1), &mut faces.z_pos),
        (IVec3::new(0, 0, -1), &mut faces.z_neg),
    ];

    for (offset, flag) in offsets {
        let neighbor_local = *local_pos + offset;

        // Check if neighbor has water (don't render face between water cells)
        if input.water.has_water(&neighbor_local) {
            continue;
        }

        // Check global position for solid blocks
        let global_pos = IVec3::new(
            input.chunk_pos.x * CHUNK_SIZE + neighbor_local.x,
            input.chunk_pos.y * CHUNK_SIZE + neighbor_local.y,
            input.chunk_pos.z * CHUNK_SIZE + neighbor_local.z,
        );

        let is_solid = if let Some(block) = input.world_map.get_block_by_coordinates(&global_pos) {
            block.id.get_visibility() == BlockTransparency::Solid
        } else {
            false
        };

        // Render face if neighbor is not solid (air or out of bounds)
        *flag = !is_solid;
    }

    faces
}

/// Adds a top (Y+) water surface quad with tessellation for smooth waves.
///
/// The quad is subdivided into a grid of smaller triangles to allow
/// the vertex shader to displace individual vertices smoothly.
fn add_top_face(
    data: &mut WaterMeshData,
    x: f32,
    y: f32,
    z: f32,
    world_x: f32,
    world_z: f32,
    volume: f32,
    wave_scale: f32,
    tessellation: u32,
) {
    add_top_face_tessellated(
        data,
        x,
        y,
        z,
        world_x,
        world_z,
        volume,
        wave_scale,
        tessellation,
    );
}

/// Adds a tessellated top face with configurable subdivision level.
fn add_top_face_tessellated(
    data: &mut WaterMeshData,
    x: f32,
    y: f32,
    z: f32,
    world_x: f32,
    world_z: f32,
    volume: f32,
    wave_scale: f32,
    subdivisions: u32,
) {
    let base_idx = data.positions.len() as u32;
    let step = 1.0 / subdivisions as f32;
    let verts_per_side = subdivisions + 1;

    // Vertex colors encode:
    // - Red: wave scale (0.0 = flat/ripples, 1.0 = full waves)
    // - Green/Blue: depth hint colors (fixed)
    // - Alpha: volume/transparency
    let alpha = volume.clamp(0.5, 1.0);
    let color = [wave_scale, 0.5, 0.8, alpha];

    // Generate vertex grid
    for row in 0..verts_per_side {
        for col in 0..verts_per_side {
            let local_x = x + col as f32 * step;
            let local_z = z + row as f32 * step;
            let wx = world_x + col as f32 * step;
            let wz = world_z + row as f32 * step;

            data.positions.push([local_x, y, local_z]);
            data.normals.push([0.0, 1.0, 0.0]); // Will be recalculated by vertex shader
            data.uvs.push([wx, wz]);
            data.colors.push(color);
        }
    }

    // Generate triangle indices (two triangles per grid cell)
    for row in 0..subdivisions {
        for col in 0..subdivisions {
            let tl = base_idx + row * verts_per_side + col;
            let tr = tl + 1;
            let bl = tl + verts_per_side;
            let br = bl + 1;

            // First triangle (counterclockwise)
            data.indices.extend_from_slice(&[tl, bl, tr]);
            // Second triangle
            data.indices.extend_from_slice(&[tr, bl, br]);
        }
    }
}

/// Adds a side face in the X+ direction.
fn add_side_face_x_pos(
    data: &mut WaterMeshData,
    x: f32,
    y: f32,
    z: f32,
    height: f32,
    world_x: f32,
    world_z: f32,
    volume: f32,
    wave_scale: f32,
) {
    let base_idx = data.positions.len() as u32;
    let top_y = y + height;

    data.positions.extend_from_slice(&[
        [x + 1.0, y, z],
        [x + 1.0, y, z + 1.0],
        [x + 1.0, top_y, z + 1.0],
        [x + 1.0, top_y, z],
    ]);

    data.normals.extend_from_slice(&[
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
    ]);

    data.uvs.extend_from_slice(&[
        [world_z, y],
        [world_z + 1.0, y],
        [world_z + 1.0, top_y],
        [world_z, top_y],
    ]);

    let alpha = volume.clamp(0.5, 1.0);
    let color = [wave_scale, 0.4, 0.7, alpha];
    data.colors.extend_from_slice(&[color, color, color, color]);

    data.indices
        .extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2]);
    data.indices
        .extend_from_slice(&[base_idx, base_idx + 2, base_idx + 3]);
}

/// Adds a side face in the X- direction.
fn add_side_face_x_neg(
    data: &mut WaterMeshData,
    x: f32,
    y: f32,
    z: f32,
    height: f32,
    world_x: f32,
    world_z: f32,
    volume: f32,
    wave_scale: f32,
) {
    let base_idx = data.positions.len() as u32;
    let top_y = y + height;

    data.positions.extend_from_slice(&[
        [x, y, z + 1.0],
        [x, y, z],
        [x, top_y, z],
        [x, top_y, z + 1.0],
    ]);

    data.normals.extend_from_slice(&[
        [-1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
    ]);

    data.uvs.extend_from_slice(&[
        [world_z + 1.0, y],
        [world_z, y],
        [world_z, top_y],
        [world_z + 1.0, top_y],
    ]);

    let alpha = volume.clamp(0.5, 1.0);
    let color = [wave_scale, 0.4, 0.7, alpha];
    data.colors.extend_from_slice(&[color, color, color, color]);

    data.indices
        .extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2]);
    data.indices
        .extend_from_slice(&[base_idx, base_idx + 2, base_idx + 3]);
}

/// Adds a side face in the Z+ direction.
fn add_side_face_z_pos(
    data: &mut WaterMeshData,
    x: f32,
    y: f32,
    z: f32,
    height: f32,
    world_x: f32,
    world_z: f32,
    volume: f32,
    wave_scale: f32,
) {
    let base_idx = data.positions.len() as u32;
    let top_y = y + height;

    data.positions.extend_from_slice(&[
        [x + 1.0, y, z + 1.0],
        [x, y, z + 1.0],
        [x, top_y, z + 1.0],
        [x + 1.0, top_y, z + 1.0],
    ]);

    data.normals.extend_from_slice(&[
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ]);

    data.uvs.extend_from_slice(&[
        [world_x + 1.0, y],
        [world_x, y],
        [world_x, top_y],
        [world_x + 1.0, top_y],
    ]);

    let alpha = volume.clamp(0.5, 1.0);
    let color = [wave_scale, 0.4, 0.7, alpha];
    data.colors.extend_from_slice(&[color, color, color, color]);

    data.indices
        .extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2]);
    data.indices
        .extend_from_slice(&[base_idx, base_idx + 2, base_idx + 3]);
}

/// Adds a side face in the Z- direction.
fn add_side_face_z_neg(
    data: &mut WaterMeshData,
    x: f32,
    y: f32,
    z: f32,
    height: f32,
    world_x: f32,
    world_z: f32,
    volume: f32,
    wave_scale: f32,
) {
    let base_idx = data.positions.len() as u32;
    let top_y = y + height;

    data.positions.extend_from_slice(&[
        [x, y, z],
        [x + 1.0, y, z],
        [x + 1.0, top_y, z],
        [x, top_y, z],
    ]);

    data.normals.extend_from_slice(&[
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -1.0],
    ]);

    data.uvs.extend_from_slice(&[
        [world_x, y],
        [world_x + 1.0, y],
        [world_x + 1.0, top_y],
        [world_x, top_y],
    ]);

    let alpha = volume.clamp(0.5, 1.0);
    let color = [wave_scale, 0.4, 0.7, alpha];
    data.colors.extend_from_slice(&[color, color, color, color]);

    data.indices
        .extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2]);
    data.indices
        .extend_from_slice(&[base_idx, base_idx + 2, base_idx + 3]);
}

/// Simplified LOD mesh generation for distant water.
///
/// At far distances, we collapse water surfaces to flat planes
/// and reduce wave detail.
pub fn generate_water_mesh_lod(input: &WaterMeshInput) -> Option<WaterMeshData> {
    if input.water.is_empty() {
        return None;
    }

    let mut data = WaterMeshData::new();
    let chunk_world_x = input.chunk_pos.x * CHUNK_SIZE;
    let chunk_world_z = input.chunk_pos.z * CHUNK_SIZE;

    // For LOD, we just render top faces (no sides) and use a fixed height
    // with reduced tessellation for better performance
    for (local_pos, cell) in input.water.iter() {
        if !should_render_top_face(input, local_pos) {
            continue;
        }

        let x = local_pos.x as f32;
        let y = local_pos.y as f32 + FULL_WATER_HEIGHT; // Fixed height for LOD
        let z = local_pos.z as f32;
        let world_x = (chunk_world_x + local_pos.x) as f32;
        let world_z = (chunk_world_z + local_pos.z) as f32;

        // For LOD, use full wave scale (distant water appears as ocean)
        // This avoids the cost of local volume calculation for distant chunks
        let wave_scale = 0.8;

        // Use lower tessellation for LOD meshes
        add_top_face_tessellated(
            &mut data,
            x,
            y,
            z,
            world_x,
            world_z,
            1.0,
            wave_scale,
            input.tessellation_lod,
        );
    }

    if data.is_empty() {
        None
    } else {
        Some(data)
    }
}

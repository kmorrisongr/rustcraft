use std::collections::HashSet;
use std::f32::consts::PI;
use std::{collections::HashMap, time::Instant};

use crate::world::{ClientChunk, ClientWorldMap};
use bevy::{
    math::IVec3,
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
};
use shared::world::{
    to_global_pos, BlockDirection, BlockId, BlockTransparency, LodLevel, WorldMap,
};
use shared::CHUNK_SIZE;

use super::voxel::{Face, FaceDirection, VoxelShape};

#[derive(Copy, Clone, Debug)]
pub struct UvCoords {
    pub u0: f32,
    pub u1: f32,
    pub v0: f32,
    pub v1: f32,
}

impl UvCoords {
    pub fn new(u0: f32, u1: f32, v0: f32, v1: f32) -> Self {
        Self { u0, u1, v0, v1 }
    }
}

#[derive(Default)]
pub struct MeshCreator {
    pub vertices: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices_offset: u32,
}

fn build_mesh(creator: &MeshCreator) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, Default::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, creator.vertices.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, creator.normals.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, creator.uvs.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, creator.colors.clone());
    mesh.insert_indices(Indices::U32(creator.indices.clone()));
    mesh
}

/// Generates a continuous water surface mesh where adjacent water blocks share vertices.
/// This prevents gaps when vertex displacement (waves) is applied by the water shader.
///
/// Returns the mesh, the total number of water surface blocks, and whether water touches chunk edges.
/// The edge-touching flag is used for cross-chunk water body size estimation.
///
/// The algorithm:
/// 1. Collect all water surface positions (water blocks with air above)
/// 2. Group them by Y level
/// 3. For each Y level, create a grid of vertices at block corners (shared between adjacent blocks)
/// 4. Generate triangle indices connecting the vertices
/// 5. Check if any water block is at a chunk boundary (x=0, x=15, z=0, or z=15)
fn generate_water_surface_mesh(
    world_map: &ClientWorldMap,
    chunk: &ClientChunk,
    chunk_pos: &IVec3,
) -> (Option<Mesh>, usize, bool) {
    // Collect water surface positions grouped by Y level
    // Key: Y level, Value: Set of (x, z) positions in local chunk coordinates
    let mut water_surfaces: HashMap<i32, HashSet<(i32, i32)>> = HashMap::new();

    for (local_block_pos, block) in chunk.map.iter() {
        if block.id != BlockId::Water {
            continue;
        }

        let global_block_pos = to_global_pos(chunk_pos, local_block_pos);

        // Check if there's air above (this is a surface water block)
        let above_pos = global_block_pos + IVec3::new(0, 1, 0);
        if world_map.get_block_by_coordinates(&above_pos).is_some() {
            continue; // Not a surface block
        }

        water_surfaces
            .entry(local_block_pos.y)
            .or_insert_with(HashSet::new)
            .insert((local_block_pos.x, local_block_pos.z));
    }

    if water_surfaces.is_empty() {
        return (None, 0, false);
    }

    // Count total water surface blocks across all Y levels
    let total_water_blocks: usize = water_surfaces.values().map(|s| s.len()).sum();

    // Check if any water block touches a chunk edge (suggesting cross-chunk water body)
    // Chunk coordinates are 0 to CHUNK_SIZE-1 (typically 0-15)
    let chunk_edge_max = (CHUNK_SIZE - 1) as i32;
    let touches_edge = water_surfaces.values().any(|positions| {
        positions
            .iter()
            .any(|(x, z)| *x == 0 || *x == chunk_edge_max || *z == 0 || *z == chunk_edge_max)
    });

    let mut vertices: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();

    // Water surface is slightly below the top of the block (like Minecraft)
    let water_surface_offset = 0.875; // 14/16 of a block

    // Process each Y level
    for (y_level, xz_positions) in water_surfaces.iter() {
        let y = *y_level as f32 + water_surface_offset;

        // Create a vertex index map for this Y level
        // Key: (corner_x, corner_z), Value: vertex index
        // Corners are at integer positions (block boundaries)
        let mut vertex_index_map: HashMap<(i32, i32), u32> = HashMap::new();

        // For each water block, we need vertices at its 4 corners
        // Corners are shared between adjacent blocks
        for (block_x, block_z) in xz_positions.iter() {
            // The 4 corners of this block
            let corners = [
                (*block_x, *block_z),         // bottom-left
                (*block_x + 1, *block_z),     // bottom-right
                (*block_x, *block_z + 1),     // top-left
                (*block_x + 1, *block_z + 1), // top-right
            ];

            // Ensure all corners have vertices
            for (cx, cz) in corners.iter() {
                if !vertex_index_map.contains_key(&(*cx, *cz)) {
                    let vertex_idx = vertices.len() as u32;
                    vertex_index_map.insert((*cx, *cz), vertex_idx);

                    // Vertex position in local chunk space
                    vertices.push([*cx as f32, y, *cz as f32]);
                    normals.push([0.0, 1.0, 0.0]); // Up-facing normal

                    // UV coordinates based on world position for proper wave tiling
                    // The bevy_water shader uses coord_offset + (uv * coord_scale) to get world coords
                    let world_x = (chunk_pos.x * CHUNK_SIZE + *cx) as f32;
                    let world_z = (chunk_pos.z * CHUNK_SIZE + *cz) as f32;
                    uvs.push([world_x, world_z]);

                    // Water color with alpha for transparency
                    colors.push([1.0, 1.0, 1.0, 0.7]);
                }
            }

            // Generate two triangles for this water block quad
            // Using counter-clockwise winding for front-facing
            let bl = vertex_index_map[&(*block_x, *block_z)];
            let br = vertex_index_map[&(*block_x + 1, *block_z)];
            let tl = vertex_index_map[&(*block_x, *block_z + 1)];
            let tr = vertex_index_map[&(*block_x + 1, *block_z + 1)];

            // Triangle 1: bottom-left, top-left, top-right
            indices.push(bl);
            indices.push(tl);
            indices.push(tr);

            // Triangle 2: bottom-left, top-right, bottom-right
            indices.push(bl);
            indices.push(tr);
            indices.push(br);
        }
    }

    if vertices.is_empty() {
        return (None, 0, false);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, Default::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));

    // Generate tangents for proper normal mapping in the shader
    if let Err(e) = mesh.generate_tangents() {
        warn!("Error generating tangents for water surface mesh: {:?}", e);
    }

    (Some(mesh), total_water_blocks, touches_edge)
}

/// Water body size category based on surface area.
/// Used to determine appropriate wave amplitude.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WaterBodySize {
    /// 1-4 blocks: tiny puddle, no waves
    Puddle,
    /// 5-16 blocks: small pond, minimal waves
    Small,
    /// 17-64 blocks: medium pond/pool, gentle waves
    #[default]
    Medium,
    /// 65-256 blocks: large lake, moderate waves
    Large,
    /// 257+ blocks: ocean/sea, full waves
    Ocean,
}

impl WaterBodySize {
    /// Determine water body size from surface block count
    pub fn from_block_count(count: usize) -> Self {
        match count {
            0..=4 => WaterBodySize::Puddle,
            5..=16 => WaterBodySize::Small,
            17..=64 => WaterBodySize::Medium,
            65..=256 => WaterBodySize::Large,
            _ => WaterBodySize::Ocean,
        }
    }

    /// When water touches chunk edges, always use Ocean amplitude.
    /// This ensures that adjacent chunks have matching wave animations at their
    /// shared boundaries, preventing visible gaps/seams.
    ///
    /// The trade-off is that some medium-sized lakes spanning chunks will have
    /// larger waves than if they were contained in a single chunk, but this is
    /// visually preferable to gaps in the water surface.
    pub fn with_edge_boost(self) -> Self {
        // Always use Ocean for edge-touching water to guarantee amplitude matching
        // at chunk boundaries. Any other approach risks mismatched amplitudes
        // causing visible gaps where vertices animate differently.
        WaterBodySize::Ocean
    }

    /// Get the wave amplitude for this water body size
    pub fn amplitude(&self) -> f32 {
        match self {
            WaterBodySize::Puddle => 0.0,  // No waves
            WaterBodySize::Small => 0.05,  // Barely visible ripples
            WaterBodySize::Medium => 0.15, // Gentle waves
            WaterBodySize::Large => 0.3,   // Moderate waves
            WaterBodySize::Ocean => 0.5,   // Full waves
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ChunkMeshResponse {
    pub solid_mesh: Option<Mesh>,
    pub water_mesh: Option<Mesh>,
    /// Size category of the water body in this chunk (for amplitude selection)
    pub water_body_size: WaterBodySize,
}

pub(crate) fn generate_chunk_mesh(
    world_map: &ClientWorldMap,
    chunk: &ClientChunk,
    chunk_pos: &IVec3,
    uv_map: &HashMap<String, UvCoords>,
) -> ChunkMeshResponse {
    let start = Instant::now();

    let mut solid_mesh_creator = MeshCreator::default();

    // Generate solid block meshes (skip water - it's handled separately)
    for (local_block_pos, block) in chunk.map.iter() {
        // Skip water blocks - they're rendered as a continuous surface mesh
        if block.id == BlockId::Water {
            continue;
        }

        let x = local_block_pos.x as f32;
        let y = local_block_pos.y as f32;
        let z = local_block_pos.z as f32;

        let global_block_pos = &to_global_pos(chunk_pos, local_block_pos);
        let visibility = block.id.get_visibility();

        if is_block_surrounded(world_map, global_block_pos, &visibility, &block.id) {
            continue;
        }

        let mut local_vertices: Vec<[f32; 3]> = vec![];
        let mut local_indices: Vec<u32> = vec![];
        let mut local_normals: Vec<[f32; 3]> = vec![];
        let mut local_uvs: Vec<[f32; 2]> = vec![];
        let mut local_colors: Vec<[f32; 4]> = vec![];

        let voxel: VoxelShape = VoxelShape::create_from_block(block);

        for face in voxel.faces.iter() {
            let uv_coords: &UvCoords;

            if let Some(uvs) = uv_map.get(&face.texture) {
                uv_coords = uvs;
            } else {
                uv_coords = uv_map.get("_Default").unwrap();
            }

            let alpha = match visibility {
                BlockTransparency::Liquid => 0.7,
                _ => 1.0,
            };

            let should_render =
                should_render_face(world_map, global_block_pos, &face.direction, &visibility);

            if should_render {
                render_face(
                    &mut local_vertices,
                    &mut local_indices,
                    &mut local_normals,
                    &mut local_uvs,
                    &mut local_colors,
                    &mut solid_mesh_creator.indices_offset,
                    face,
                    uv_coords,
                    1.0,
                    alpha,
                );

                // Add break overlay for blocks being broken
                if block.breaking_progress > 0 {
                    let breaking_progress = block.get_breaking_level();

                    render_face(
                        &mut local_vertices,
                        &mut local_indices,
                        &mut local_normals,
                        &mut local_uvs,
                        &mut local_colors,
                        &mut solid_mesh_creator.indices_offset,
                        face,
                        uv_map
                            .get(&format!("DestroyStage{breaking_progress}"))
                            .unwrap(),
                        1.0,
                        alpha,
                    );
                }
            }
        }

        let local_vertices: Vec<[f32; 3]> = local_vertices
            .iter()
            .map(|v| {
                let v = rotate_vertices(v, &block.direction);
                [v[0] + x, v[1] + y, v[2] + z]
            })
            .collect();

        solid_mesh_creator.vertices.extend(local_vertices);
        solid_mesh_creator.indices.extend(local_indices);
        solid_mesh_creator.normals.extend(local_normals);
        solid_mesh_creator.uvs.extend(local_uvs);
        solid_mesh_creator.colors.extend(local_colors);
    }

    // Build solid mesh
    let mut solid_mesh = build_mesh(&solid_mesh_creator);

    trace!("Render time : {:?}", Instant::now() - start);

    let should_return_solid = !solid_mesh_creator.vertices.is_empty();
    if should_return_solid {
        if let Err(e) = solid_mesh.generate_tangents() {
            warn!(
                "Error while generating tangents for the mesh SOLID : {:?} | {:?}",
                e, solid_mesh
            );
        }
    };

    // Generate continuous water surface mesh (vertices shared between adjacent blocks)
    let (water_mesh, water_block_count, touches_edge) =
        generate_water_surface_mesh(world_map, chunk, chunk_pos);

    // Determine water body size, boosting if water touches chunk boundaries
    // (suggesting it's part of a larger cross-chunk body)
    let water_body_size = {
        let base_size = WaterBodySize::from_block_count(water_block_count);
        if touches_edge {
            base_size.with_edge_boost()
        } else {
            base_size
        }
    };

    ChunkMeshResponse {
        solid_mesh: if should_return_solid {
            Some(solid_mesh)
        } else {
            None
        },
        water_mesh,
        water_body_size,
    }
}

pub(crate) fn is_block_surrounded(
    world_map: &ClientWorldMap,
    global_block_pos: &IVec3,
    block_visibility: &BlockTransparency,
    block_id: &BlockId,
) -> bool {
    for offset in &shared::world::SIX_OFFSETS {
        let neighbor_pos = *global_block_pos + *offset;

        // Check if the block exists at the neighboring position
        if let Some(block) = world_map.get_block_by_coordinates(&neighbor_pos) {
            let vis = block.id.get_visibility();
            match vis {
                BlockTransparency::Solid => {}
                BlockTransparency::Decoration => return false,
                BlockTransparency::Liquid => {
                    if vis != *block_visibility {
                        return false;
                    }
                }
                BlockTransparency::Transparent => {
                    if *block_id != block.id {
                        return false;
                    }
                }
            }
        } else {
            return false;
        }
    }

    true
}

pub fn rotate_vertices(v: &[f32; 3], direction: &BlockDirection) -> [f32; 3] {
    let angle = match *direction {
        BlockDirection::Front => 0.,
        BlockDirection::Right => -PI / 2.,
        BlockDirection::Left => PI / 2.,
        BlockDirection::Back => PI,
    };

    [
        angle.cos() * v[0] + angle.sin() * v[2],
        v[1],
        (-angle).sin() * v[0] + angle.cos() * v[2],
    ]
}

fn render_face(
    local_vertices: &mut Vec<[f32; 3]>,
    local_indices: &mut Vec<u32>,
    local_normals: &mut Vec<[f32; 3]>,
    local_uvs: &mut Vec<[f32; 2]>,
    local_colors: &mut Vec<[f32; 4]>,
    indices_offset: &mut u32,
    face: &Face,
    uv_coords: &UvCoords,
    color_multiplier: f32,
    alpha: f32,
) {
    local_vertices.extend(face.vertices.iter());

    local_indices.extend(face.indices.iter().map(|x| x + *indices_offset));
    *indices_offset += face.vertices.len() as u32;

    local_normals.extend(face.normals.iter());

    let colors = face.colors.iter();
    let mut new_colors = vec![];
    for color in colors {
        new_colors.push([
            color[0] * color_multiplier,
            color[1] * color_multiplier,
            color[2] * color_multiplier,
            alpha,
        ]);
    }

    local_colors.extend(new_colors);

    local_uvs.extend(face.uvs.iter().map(|uv| {
        // !!! DO NOT REMOVE THE FLOAT OFFSET !!!
        // It removes seams between blocks in chunk meshes
        [
            (uv[0] + uv_coords.u0 + 0.001).min(uv_coords.u1 - 0.001),
            (uv[1] + uv_coords.v0 + 0.001).min(uv_coords.v1 - 0.001),
        ]
    }));
}

fn should_render_face(
    world_map: &ClientWorldMap,
    global_block_pos: &IVec3,
    direction: &FaceDirection,
    block_visibility: &BlockTransparency,
) -> bool {
    let offset = match *direction {
        FaceDirection::Front => IVec3::new(0, 0, -1),
        FaceDirection::Back => IVec3::new(0, 0, 1),
        FaceDirection::Top => IVec3::new(0, 1, 0),
        FaceDirection::Bottom => IVec3::new(0, -1, 0),
        FaceDirection::Left => IVec3::new(-1, 0, 0),
        FaceDirection::Right => IVec3::new(1, 0, 0),
        FaceDirection::Inset => return true,
    };

    if let Some(block) = world_map.get_block_by_coordinates(&(*global_block_pos + offset)) {
        let vis = block.id.get_visibility();
        match vis {
            BlockTransparency::Solid => false,
            BlockTransparency::Decoration => true,
            BlockTransparency::Transparent | BlockTransparency::Liquid => *block_visibility != vis,
        }
    } else {
        true
    }
}

// ============================================================================
// LOD Mesh Generation
// ============================================================================

/// Generate mesh at specified LOD level.
/// Delegates to generate_chunk_mesh() for LOD 0 (full detail).
///
/// For LOD 1, samples every 2nd block in each dimension and renders at 2× scale.
pub(crate) fn generate_chunk_mesh_lod(
    world_map: &ClientWorldMap,
    chunk: &ClientChunk,
    chunk_pos: &IVec3,
    uv_map: &HashMap<String, UvCoords>,
    lod_level: LodLevel,
) -> ChunkMeshResponse {
    if lod_level == LodLevel::Lod0 {
        return generate_chunk_mesh(world_map, chunk, chunk_pos, uv_map);
    }

    let start = Instant::now();
    let scale = lod_level.block_scale();
    let scale_f32 = scale as f32;

    let mut solid_mesh_creator = MeshCreator::default();
    // Note: Water is simplified at LOD 1 - we skip water mesh generation for distant chunks

    // Sample at LOD intervals: for scale=2, positions 0,2,4,6,8,10,12,14
    let samples_per_axis = CHUNK_SIZE / scale;

    for lod_x in 0..samples_per_axis {
        for lod_y in 0..samples_per_axis {
            for lod_z in 0..samples_per_axis {
                // Convert LOD coordinates to local block coordinates
                let local_pos = IVec3::new(lod_x * scale, lod_y * scale, lod_z * scale);

                // Get the block at this position
                let Some(block) = chunk.map.get(&local_pos) else {
                    continue;
                };

                // Skip non-solid blocks for LOD meshes (simplified rendering)
                let visibility = block.id.get_visibility();
                if matches!(
                    visibility,
                    BlockTransparency::Decoration | BlockTransparency::Liquid
                ) {
                    continue;
                }

                let global_block_pos = to_global_pos(chunk_pos, &local_pos);

                // Check if block is fully surrounded (skip rendering)
                if is_lod_block_surrounded(world_map, &global_block_pos, &visibility, scale) {
                    continue;
                }

                let x = local_pos.x as f32;
                let y = local_pos.y as f32;
                let z = local_pos.z as f32;

                let mut local_vertices: Vec<[f32; 3]> = vec![];
                let mut local_indices: Vec<u32> = vec![];
                let mut local_normals: Vec<[f32; 3]> = vec![];
                let mut local_uvs: Vec<[f32; 2]> = vec![];
                let mut local_colors: Vec<[f32; 4]> = vec![];

                // Create voxel shape (reuse existing logic)
                let voxel = VoxelShape::create_from_block(block);

                for face in voxel.faces.iter() {
                    let uv_coords = uv_map
                        .get(&face.texture)
                        .unwrap_or_else(|| uv_map.get("_Default").unwrap());

                    // Check if face should be rendered at LOD scale
                    if should_render_lod_face(
                        world_map,
                        chunk,
                        &global_block_pos,
                        &local_pos,
                        &face.direction,
                        &visibility,
                        scale,
                    ) {
                        render_face_scaled(
                            &mut local_vertices,
                            &mut local_indices,
                            &mut local_normals,
                            &mut local_uvs,
                            &mut local_colors,
                            &mut solid_mesh_creator.indices_offset,
                            face,
                            uv_coords,
                            1.0,
                            1.0,
                            scale_f32,
                        );
                    }
                }

                // Apply block position offset (at LOD scale, blocks are larger)
                let local_vertices: Vec<[f32; 3]> = local_vertices
                    .iter()
                    .map(|v| {
                        let v = rotate_vertices(v, &block.direction);
                        [v[0] + x, v[1] + y, v[2] + z]
                    })
                    .collect();

                solid_mesh_creator.vertices.extend(local_vertices);
                solid_mesh_creator.indices.extend(local_indices);
                solid_mesh_creator.normals.extend(local_normals);
                solid_mesh_creator.uvs.extend(local_uvs);
                solid_mesh_creator.colors.extend(local_colors);
            }
        }
    }

    let solid_mesh = build_mesh(&solid_mesh_creator);
    let should_return_solid = !solid_mesh_creator.vertices.is_empty();

    // Skip tangent generation for LOD meshes (no visual benefit at distance)
    trace!("LOD render time : {:?}", Instant::now() - start);

    ChunkMeshResponse {
        solid_mesh: if should_return_solid {
            Some(solid_mesh)
        } else {
            None
        },
        water_mesh: None,                          // No water at LOD 1
        water_body_size: WaterBodySize::default(), // Not used since water_mesh is None
    }
}

/// Check if a block is surrounded at LOD scale.
/// Conservative check: only returns true if the block is definitely fully occluded.
/// This is an optimization to skip blocks that don't need any face rendering.
fn is_lod_block_surrounded(
    world_map: &ClientWorldMap,
    global_block_pos: &IVec3,
    _block_visibility: &BlockTransparency,
    scale: i32,
) -> bool {
    // For LOD, we need to check if all 6 faces would be fully occluded.
    // To be conservative and avoid holes, we check if the LOD sample point
    // in each direction has a solid block. This may over-render but won't cause holes.
    let offsets = [
        IVec3::new(scale, 0, 0),
        IVec3::new(-scale, 0, 0),
        IVec3::new(0, scale, 0),
        IVec3::new(0, -scale, 0),
        IVec3::new(0, 0, scale),
        IVec3::new(0, 0, -scale),
    ];

    for offset in &offsets {
        let neighbor_pos = *global_block_pos + *offset;

        match world_map.get_block_by_coordinates(&neighbor_pos) {
            Some(block) if block.id.get_visibility() == BlockTransparency::Solid => {}
            _ => return false, // Not solid or missing: block is not fully surrounded
        }
    }

    true
}

/// Determine if a face should be rendered at LOD scale.
/// Conservative at chunk boundaries: always render faces at edges.
fn should_render_lod_face(
    world_map: &ClientWorldMap,
    chunk: &ClientChunk,
    global_block_pos: &IVec3,
    local_block_pos: &IVec3,
    direction: &FaceDirection,
    block_visibility: &BlockTransparency,
    scale: i32,
) -> bool {
    let (offset, is_chunk_edge) = match *direction {
        FaceDirection::Front => (
            IVec3::new(0, 0, -scale),
            local_block_pos.z < scale, // Near Z=0 edge
        ),
        FaceDirection::Back => (
            IVec3::new(0, 0, scale),
            local_block_pos.z >= CHUNK_SIZE - scale, // Near Z=max edge
        ),
        FaceDirection::Top => (
            IVec3::new(0, scale, 0),
            local_block_pos.y >= CHUNK_SIZE - scale, // Near Y=max edge
        ),
        FaceDirection::Bottom => (
            IVec3::new(0, -scale, 0),
            local_block_pos.y < scale, // Near Y=0 edge
        ),
        FaceDirection::Left => (
            IVec3::new(-scale, 0, 0),
            local_block_pos.x < scale, // Near X=0 edge
        ),
        FaceDirection::Right => (
            IVec3::new(scale, 0, 0),
            local_block_pos.x >= CHUNK_SIZE - scale, // Near X=max edge
        ),
        FaceDirection::Inset => return true,
    };

    // For faces at chunk boundaries, check the world map (cross-chunk lookup)
    // For interior faces, we can check the local chunk for better performance
    let neighbor_pos = *global_block_pos + offset;

    if is_chunk_edge {
        // Cross-chunk boundary: use world map lookup, render face if neighbor is unknown
        if let Some(block) = world_map.get_block_by_coordinates(&neighbor_pos) {
            let vis = block.id.get_visibility();
            match vis {
                BlockTransparency::Solid => false,
                BlockTransparency::Decoration => true,
                BlockTransparency::Transparent | BlockTransparency::Liquid => {
                    *block_visibility != vis
                }
            }
        } else {
            true // Conservative: render faces at chunk boundaries if neighbor unknown
        }
    } else {
        // Interior: check local chunk for the LOD sample point of the neighbor region.
        // For performance, we only check the single block at the LOD grid position
        // (the block that represents the entire LOD cell). This is an approximation
        // that trades some accuracy for much better performance.
        let neighbor_lod_origin = *local_block_pos + offset;

        // Check the LOD sample point (origin of the neighbor LOD cell)
        match chunk.map.get(&neighbor_lod_origin) {
            Some(block) if block.id.get_visibility() == BlockTransparency::Solid => false,
            _ => true, // Render face if sample point is not solid
        }
    }
}

/// Render a face at scaled size for LOD meshes.
/// Vertices are multiplied by scale to create larger blocks.
fn render_face_scaled(
    local_vertices: &mut Vec<[f32; 3]>,
    local_indices: &mut Vec<u32>,
    local_normals: &mut Vec<[f32; 3]>,
    local_uvs: &mut Vec<[f32; 2]>,
    local_colors: &mut Vec<[f32; 4]>,
    indices_offset: &mut u32,
    face: &Face,
    uv_coords: &UvCoords,
    color_multiplier: f32,
    alpha: f32,
    scale: f32,
) {
    // Scale vertices by LOD factor
    local_vertices.extend(
        face.vertices
            .iter()
            .map(|v| [v[0] * scale, v[1] * scale, v[2] * scale]),
    );

    local_indices.extend(face.indices.iter().map(|x| x + *indices_offset));
    *indices_offset += face.vertices.len() as u32;

    local_normals.extend(face.normals.iter());

    let new_colors: Vec<[f32; 4]> = face
        .colors
        .iter()
        .map(|color| {
            [
                color[0] * color_multiplier,
                color[1] * color_multiplier,
                color[2] * color_multiplier,
                alpha,
            ]
        })
        .collect();

    local_colors.extend(new_colors);

    // UVs remain the same (texture repeats at LOD scale)
    local_uvs.extend(face.uvs.iter().map(|uv| {
        [
            (uv[0] + uv_coords.u0 + 0.001).min(uv_coords.u1 - 0.001),
            (uv[1] + uv_coords.v0 + 0.001).min(uv_coords.v1 - 0.001),
        ]
    }));
}

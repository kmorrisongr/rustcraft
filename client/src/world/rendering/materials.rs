use crate::constants::{BASE_ROUGHNESS, BASE_SPECULAR_HIGHLIGHT};
use crate::world::GlobalMaterial;
use crate::TexturePath;
use bevy::image::ImageSampler;
use bevy::platform::collections::HashMap as BevyHashMap;
use bevy::prelude::*;
use bevy::render::render_resource::Face;
use bevy_asset_loader::prelude::*;
use shared::world::{BlockId, GameElementId, ItemId};
use shared::GameFolderPaths;
use std::collections::HashMap;
use std::fs;
use std::marker::PhantomData;

use super::meshing::UvCoords;

/// Asset collection for block textures loaded dynamically from a folder.
/// Uses bevy_asset_loader with iyes_progress integration.
#[derive(AssetCollection, Resource)]
pub struct BlockTextureAssets {
    #[asset(key = "block_textures", collection(typed, mapped))]
    pub textures: BevyHashMap<String, Handle<Image>>,
}

/// Asset collection for item textures (same as block textures).
#[derive(AssetCollection, Resource)]
pub struct ItemTextureAssets {
    #[asset(key = "item_textures", collection(typed, mapped))]
    pub textures: BevyHashMap<String, Handle<Image>>,
}

#[derive(Resource, Debug)]
pub struct AtlasWrapper {
    pub handles: HashMap<String, Handle<Image>>,
    pub texture: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
    pub sources: TextureAtlasSources,
    pub uvs: HashMap<String, UvCoords>,
}

#[derive(Resource, Default, Debug)]
pub struct MaterialResource {
    pub global_materials: HashMap<GlobalMaterial, Handle<StandardMaterial>>,
    pub items: Option<AtlasWrapper>,
    pub blocks: Option<AtlasWrapper>,
}

#[derive(Resource)]
pub struct AtlasHandles<T> {
    pub handles: Vec<(Handle<Image>, String)>,
    pub loaded: bool,
    /// Phantom to allow multiple instances of the struct
    _d: PhantomData<T>,
}

impl<T> Default for AtlasHandles<T> {
    fn default() -> Self {
        Self {
            handles: Vec::new(),
            loaded: false,
            _d: PhantomData {},
        }
    }
}

/// Configures dynamic assets for bevy_asset_loader based on the texture paths.
/// This runs before the loading state to set up the dynamic asset keys.
pub fn configure_dynamic_texture_assets(
    mut dynamic_assets: ResMut<DynamicAssets>,
    texture_path: Res<TexturePath>,
    paths: Res<GameFolderPaths>,
) {
    let blocks_path = paths
        .assets_folder_path
        .join(&texture_path.path)
        .join("blocks");

    info!("Configuring dynamic assets from: {}", blocks_path.display());

    // Collect all PNG files from the blocks directory
    if let Ok(dir) = fs::read_dir(&blocks_path) {
        let texture_files: Vec<String> = dir
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension()?.to_str()? == "png" {
                    Some(
                        blocks_path
                            .join(path.file_name()?)
                            .to_string_lossy()
                            .into_owned(),
                    )
                } else {
                    None
                }
            })
            .collect();

        info!("Found {} block textures", texture_files.len());

        // Register as dynamic assets using Files collection
        dynamic_assets.register_asset(
            "block_textures",
            Box::new(StandardDynamicAsset::Files {
                paths: texture_files.clone(),
            }),
        );
        dynamic_assets.register_asset(
            "item_textures",
            Box::new(StandardDynamicAsset::Files {
                paths: texture_files,
            }),
        );
    } else {
        warn!(
            "Could not read block textures directory: {}",
            blocks_path.display()
        );
        // Register empty collections to prevent loading state from hanging
        dynamic_assets.register_asset(
            "block_textures",
            Box::new(StandardDynamicAsset::Files { paths: vec![] }),
        );
        dynamic_assets.register_asset(
            "item_textures",
            Box::new(StandardDynamicAsset::Files { paths: vec![] }),
        );
    }
}

/// Sets up basic materials (sun, moon) that don't require loaded textures.
pub fn setup_basic_materials(
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut material_resource: ResMut<MaterialResource>,
) {
    let sun_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1., 0.95, 0.1),
        emissive: LinearRgba::new(1., 0.95, 0.1, 0.5),
        emissive_exposure_weight: 0.5,
        cull_mode: Some(Face::Front),
        ..Default::default()
    });

    let moon_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::WHITE,
        emissive_exposure_weight: 0.5,
        cull_mode: Some(Face::Front),
        ..Default::default()
    });

    material_resource
        .global_materials
        .insert(GlobalMaterial::Sun, sun_material);
    material_resource
        .global_materials
        .insert(GlobalMaterial::Moon, moon_material);
}

/// Initializes block atlas handles from the loaded BlockTextureAssets.
/// Called via `finally_init_resource` after bevy_asset_loader finishes loading.
pub fn init_block_atlas_handles(
    mut atlas_handles: ResMut<AtlasHandles<BlockId>>,
    block_assets: Res<BlockTextureAssets>,
) {
    atlas_handles.handles = block_assets
        .textures
        .iter()
        .map(|(name, handle)| {
            // Extract just the filename without path and extension
            let filename = std::path::Path::new(name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(name)
                .to_string();
            (handle.clone(), filename)
        })
        .collect();

    info!(
        "Initialized {} block atlas handles",
        atlas_handles.handles.len()
    );
}

/// Initializes item atlas handles from the loaded ItemTextureAssets.
/// Called via `finally_init_resource` after bevy_asset_loader finishes loading.
pub fn init_item_atlas_handles(
    mut atlas_handles: ResMut<AtlasHandles<ItemId>>,
    item_assets: Res<ItemTextureAssets>,
) {
    atlas_handles.handles = item_assets
        .textures
        .iter()
        .map(|(name, handle)| {
            // Extract just the filename without path and extension
            let filename = std::path::Path::new(name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(name)
                .to_string();
            (handle.clone(), filename)
        })
        .collect();

    info!(
        "Initialized {} item atlas handles",
        atlas_handles.handles.len()
    );
}

/// Creates texture atlases from loaded assets.
/// This system runs after bevy_asset_loader has finished loading all textures.
/// The assets are guaranteed to be loaded when this runs.
pub fn create_all_atlases(
    mut atlases: (ResMut<AtlasHandles<BlockId>>, ResMut<AtlasHandles<ItemId>>),
    mut images: ResMut<Assets<Image>>,
    mut material_resource: ResMut<MaterialResource>,
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Skip if atlases are already built
    if atlases.0.loaded && atlases.1.loaded {
        return;
    }

    // Skip if no handles loaded yet
    if atlases.0.handles.is_empty() && atlases.1.handles.is_empty() {
        return;
    }

    // Build block atlas
    if !atlases.0.loaded && !atlases.0.handles.is_empty() && material_resource.blocks.is_none() {
        if let Some(blocks) = build_texture_atlas(
            &mut atlases.0,
            &mut images,
            &mut texture_atlases,
            None,
            Some(ImageSampler::nearest()),
        ) {
            material_resource.global_materials.insert(
                GlobalMaterial::Blocks,
                materials.add(StandardMaterial {
                    base_color_texture: Some(blocks.texture.clone_weak()),
                    perceptual_roughness: BASE_ROUGHNESS,
                    reflectance: BASE_SPECULAR_HIGHLIGHT,
                    alpha_mode: AlphaMode::AlphaToCoverage,
                    ..default()
                }),
            );

            material_resource.blocks = Some(blocks);
            atlases.0.loaded = true;
            info!("Block texture atlas created");
        }
    }

    // Build item atlas
    if !atlases.1.loaded && !atlases.1.handles.is_empty() && material_resource.items.is_none() {
        if let Some(items) = build_texture_atlas(
            &mut atlases.1,
            &mut images,
            &mut texture_atlases,
            None,
            Some(ImageSampler::nearest()),
        ) {
            material_resource.global_materials.insert(
                GlobalMaterial::Items,
                materials.add(StandardMaterial {
                    base_color_texture: Some(items.texture.clone_weak()),
                    perceptual_roughness: BASE_ROUGHNESS,
                    reflectance: BASE_SPECULAR_HIGHLIGHT,
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                }),
            );
            material_resource.items = Some(items);
            atlases.1.loaded = true;
            info!("Item texture atlas created");
        }
    }
}

fn build_texture_atlas<T: GameElementId>(
    atlas_handles: &mut AtlasHandles<T>,
    images: &mut ResMut<Assets<Image>>,
    texture_atlases: &mut ResMut<Assets<TextureAtlasLayout>>,
    padding: Option<UVec2>,
    sampling: Option<ImageSampler>,
) -> Option<AtlasWrapper> {
    if atlas_handles.loaded {
        // Blocks if this atlas is loaded but game setup phase is not done yet
        return None;
    }

    let mut texture_atlas_builder = TextureAtlasBuilder::default();
    texture_atlas_builder.padding(padding.unwrap_or_default());

    for handle in atlas_handles.handles.iter() {
        let id = handle.0.id();
        let Some(texture) = images.get(id) else {
            // Not all images are loaded yet
            return None;
        };

        texture_atlas_builder.add_texture(Some(id), texture);
    }

    let (texture_atlas_layout, texture_atlas_sources, texture) =
        texture_atlas_builder.build().unwrap();

    let size = texture.size_f32();
    let texture = images.add(texture);
    // Update the sampling settings of the texture atlas
    let image = images.get_mut(&texture).unwrap();
    image.sampler = sampling.unwrap_or_default();

    // Create UV references
    let mut handles = HashMap::new();
    let mut uvs = HashMap::new();
    for i in atlas_handles.handles.iter() {
        handles.insert(i.1.clone(), i.0.clone_weak());
        let rect = texture_atlas_sources
            .texture_rect(&texture_atlas_layout, i.0.id())
            .unwrap_or_default();

        let uv_coords = UvCoords::new(
            rect.min.x as f32 / size.x,
            rect.max.x as f32 / size.x,
            rect.min.y as f32 / size.y,
            rect.max.y as f32 / size.y,
        );

        uvs.insert(i.1.clone(), uv_coords);
    }

    // Create the atlas
    Some(AtlasWrapper {
        texture,
        layout: texture_atlases.add(texture_atlas_layout),
        sources: texture_atlas_sources,
        handles,
        uvs,
    })
}

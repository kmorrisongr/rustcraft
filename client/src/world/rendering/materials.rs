use crate::constants::{BASE_ROUGHNESS, BASE_SPECULAR_HIGHLIGHT};
use crate::world::GlobalMaterial;
use crate::TexturePath;
use bevy::image::ImageSampler;
use bevy::platform::collections::HashMap as BevyHashMap;
use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use std::collections::HashMap;

use super::meshing::UvCoords;

/// Asset collection for block textures loaded dynamically from a folder.
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

/// Wrapper for a built texture atlas with UV coordinates.
#[derive(Debug)]
pub struct AtlasWrapper {
    pub handles: HashMap<String, Handle<Image>>,
    pub texture: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
    pub sources: TextureAtlasSources,
    pub uvs: HashMap<String, UvCoords>,
}

/// Contains all game materials.
#[derive(Resource, Default)]
pub struct MaterialResource {
    pub global_materials: HashMap<GlobalMaterial, Handle<StandardMaterial>>,
}

/// Resource that holds the built texture atlases.
/// Initialized via `finally_init_resource` after assets are loaded.
#[derive(Resource)]
pub struct TextureAtlases {
    pub blocks: AtlasWrapper,
    pub items: AtlasWrapper,
}

impl FromWorld for TextureAtlases {
    fn from_world(world: &mut World) -> Self {
        // Build block atlas
        let block_handles: Vec<(Handle<Image>, String)> = world
            .resource::<BlockTextureAssets>()
            .textures
            .iter()
            .map(|(name, handle)| {
                let filename = std::path::Path::new(name)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(name)
                    .to_string();
                (handle.clone(), filename)
            })
            .collect();

        let blocks = world.resource_scope(|world, mut images: Mut<Assets<Image>>| {
            world.resource_scope(|_world, mut layouts: Mut<Assets<TextureAtlasLayout>>| {
                build_texture_atlas(&block_handles, &mut images, &mut layouts)
                    .expect("Failed to build block texture atlas")
            })
        });

        info!(
            "Block texture atlas created with {} textures",
            block_handles.len()
        );

        // Build item atlas
        let item_handles: Vec<(Handle<Image>, String)> = world
            .resource::<ItemTextureAssets>()
            .textures
            .iter()
            .map(|(name, handle)| {
                let filename = std::path::Path::new(name)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(name)
                    .to_string();
                (handle.clone(), filename)
            })
            .collect();

        let items = world.resource_scope(|world, mut images: Mut<Assets<Image>>| {
            world.resource_scope(|_world, mut layouts: Mut<Assets<TextureAtlasLayout>>| {
                build_texture_atlas(&item_handles, &mut images, &mut layouts)
                    .expect("Failed to build item texture atlas")
            })
        });

        info!(
            "Item texture atlas created with {} textures",
            item_handles.len()
        );

        TextureAtlases { blocks, items }
    }
}

/// System to create all materials from the texture atlases.
/// Runs after TextureAtlases is initialized.
pub fn setup_atlas_materials(
    atlases: Res<TextureAtlases>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut material_resource: ResMut<MaterialResource>,
) {
    // Create block material
    let block_material = materials.add(StandardMaterial {
        base_color_texture: Some(atlases.blocks.texture.clone()),
        perceptual_roughness: BASE_ROUGHNESS,
        reflectance: BASE_SPECULAR_HIGHLIGHT,
        alpha_mode: AlphaMode::AlphaToCoverage,
        ..default()
    });
    material_resource
        .global_materials
        .insert(GlobalMaterial::Blocks, block_material);

    // Create item material
    let item_material = materials.add(StandardMaterial {
        base_color_texture: Some(atlases.items.texture.clone()),
        perceptual_roughness: BASE_ROUGHNESS,
        reflectance: BASE_SPECULAR_HIGHLIGHT,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    material_resource
        .global_materials
        .insert(GlobalMaterial::Items, item_material);

    info!("All materials created");
}

/// Configures dynamic assets for bevy_asset_loader based on the texture paths.
/// This runs before the loading state to set up the dynamic asset keys.
pub fn configure_dynamic_texture_assets(
    mut dynamic_assets: ResMut<DynamicAssets>,
    texture_path: Res<TexturePath>,
) {
    // Build the relative path from the assets folder
    // Note: Folder is not supported for web builds - use Files variant if web support is needed
    let blocks_path = format!("{}/blocks", texture_path.path);

    info!("Configuring dynamic assets from folder: {}", blocks_path);

    // Register block textures using Folder - bevy_asset_loader will discover all files
    dynamic_assets.register_asset(
        "block_textures",
        Box::new(StandardDynamicAsset::Folder {
            path: blocks_path.clone(),
        }),
    );

    // Register item textures using the same folder
    dynamic_assets.register_asset(
        "item_textures",
        Box::new(StandardDynamicAsset::Folder { path: blocks_path }),
    );

    info!("Dynamic texture assets configured for folder loading");
}

/// Builds a texture atlas from a list of image handles and names.
fn build_texture_atlas(
    handles: &[(Handle<Image>, String)],
    images: &mut Assets<Image>,
    layouts: &mut Assets<TextureAtlasLayout>,
) -> Option<AtlasWrapper> {
    let mut builder = TextureAtlasBuilder::default();
    builder.padding(UVec2::ZERO);

    for (handle, _name) in handles {
        let id = handle.id();
        let Some(texture) = images.get(id) else {
            warn!("Texture not loaded: {:?}", id);
            return None;
        };
        builder.add_texture(Some(id), texture);
    }

    let (layout, sources, mut texture) = builder.build().ok()?;
    texture.sampler = ImageSampler::nearest();

    let size = texture.size_f32();
    let texture_handle = images.add(texture);

    // Build UV coordinates and handle mappings
    let mut handle_map = HashMap::new();
    let mut uvs = HashMap::new();

    for (handle, name) in handles {
        handle_map.insert(name.clone(), handle.clone());

        let rect = sources
            .texture_rect(&layout, handle.id())
            .unwrap_or_default();

        uvs.insert(
            name.clone(),
            UvCoords::new(
                rect.min.x as f32 / size.x,
                rect.max.x as f32 / size.x,
                rect.min.y as f32 / size.y,
                rect.max.y as f32 / size.y,
            ),
        );
    }

    Some(AtlasWrapper {
        handles: handle_map,
        texture: texture_handle,
        layout: layouts.add(layout),
        sources,
        uvs,
    })
}

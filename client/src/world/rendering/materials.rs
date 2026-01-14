use crate::constants::{BASE_ROUGHNESS, BASE_SPECULAR_HIGHLIGHT};
use crate::world::GlobalMaterial;
use bevy::image::ImageSampler;
use bevy::platform::collections::HashMap as BevyHashMap;
use bevy::prelude::*;
use bevy::render::render_resource::Face;
use bevy_asset_loader::prelude::*;
use shared::game_state::GameState;
use std::collections::HashMap;

use super::meshing::UvCoords;

/// Dynamically loaded texture assets from the `base_textures/blocks` folder.
/// Textures are mapped by their file stem (filename without extension).
/// For example, `Grass.png` becomes key `"Grass"`.
#[derive(AssetCollection, Resource)]
pub struct TextureAssets {
    /// All block textures loaded from the `base_textures/blocks` folder.
    /// Keys are the file stems (e.g., "Grass", "Stone", "_Default").
    #[asset(path = "base_textures/blocks", collection(typed, mapped))]
    pub textures: BevyHashMap<String, Handle<Image>>,
}

impl TextureAssets {
    /// Returns all texture handles with their names (matching the original filename without extension).
    pub fn get_all_handles(&self) -> Vec<(Handle<Image>, String)> {
        self.textures
            .iter()
            .map(|(key, handle)| (handle.clone(), key.clone()))
            .collect()
    }

    #[allow(dead_code)]
    /// Get a specific texture by name. Returns None if the texture doesn't exist.
    pub fn get(&self, name: &str) -> Option<&Handle<Image>> {
        self.textures.get(name)
    }
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

pub fn setup_materials(
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

/// Creates texture atlases from the loaded TextureAssets.
/// This system runs after bevy_asset_loader has loaded all textures.
pub fn create_all_atlases(
    texture_assets: Res<TextureAssets>,
    mut images: ResMut<Assets<Image>>,
    mut material_resource: ResMut<MaterialResource>,
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Collect all texture handles from TextureAssets
    let texture_handles = texture_assets.get_all_handles();

    // Build block atlas
    if material_resource.blocks.is_none() {
        if let Some(blocks) = build_texture_atlas(
            &texture_handles,
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
        } else {
            panic!("Failed to build block texture atlas");
        }
    }

    // Build item atlas (uses same textures as blocks)
    if material_resource.items.is_none() {
        if let Some(items) = build_texture_atlas(
            &texture_handles,
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
        } else {
            panic!("Failed to build item texture atlas");
        }
    }

    info!("Texture atlases created successfully");
}

fn build_texture_atlas(
    texture_handles: &[(Handle<Image>, String)],
    images: &mut ResMut<Assets<Image>>,
    texture_atlases: &mut ResMut<Assets<TextureAtlasLayout>>,
    padding: Option<UVec2>,
    sampling: Option<ImageSampler>,
) -> Option<AtlasWrapper> {
    let mut texture_atlas_builder = TextureAtlasBuilder::default();
    texture_atlas_builder.padding(padding.unwrap_or_default());

    for (handle, _name) in texture_handles.iter() {
        let id = handle.id();
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
    for (handle, name) in texture_handles.iter() {
        handles.insert(name.clone(), handle.clone_weak());
        let rect = texture_atlas_sources
            .texture_rect(&texture_atlas_layout, handle.id())
            .unwrap_or_default();

        let uv_coords = UvCoords::new(
            rect.min.x as f32 / size.x,
            rect.max.x as f32 / size.x,
            rect.min.y as f32 / size.y,
            rect.max.y as f32 / size.y,
        );

        uvs.insert(name.clone(), uv_coords);
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

pub struct MaterialsPlugin;
impl Plugin for MaterialsPlugin {
    fn build(&self, app: &mut App) {
        app.configure_loading_state(
            LoadingStateConfig::new(GameState::PreGameLoading).load_collection::<TextureAssets>(),
        );
    }
}

//! This module is concerned about texture atlases for sprites.
//!
//! Sprite textures are 130M x 130N pixels, where each texture is 128px by 128px and spans 1 tile high.
//! (There is some wasted space at the right & left edges, aswell as in the corners).
//!
//! The transparent pixels should be colorized (eg with <https://github.com/urraka/alpha-bleeding>),
//! in order to prevent alpha bleeding.
//!

use bevy::prelude::*;

use crate::tilemap::TILE_SCALE;

/// Texture resolution for a single tile.
const TILE_RESOLUTION: u32 = 128;
/// The padding in pixels to each side for each tile.
const TILE_PADDING: u32 = 1;

const Z_LAYER_TERRAIN: f32 = 0.1;
const Z_LAYER_RAILS: f32 = 0.2;
const Z_LAYER_TRAINS: f32 = 0.3;

pub struct AssetPlugin;
impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        if cfg!(debug_assertions) {
            app.add_systems(PreStartup, load_texture_atlases);
        }
    }
}

#[derive(Resource)]
pub struct SpriteAssets {
    terrain: (Handle<TextureAtlasLayout>, Handle<Image>),
    rails: (Handle<TextureAtlasLayout>, Handle<Image>),
    vehicles: (Handle<TextureAtlasLayout>, Handle<Image>),
}

#[derive(Debug, Clone, Copy)]
pub enum TerrainSprite {
    Land1 = 0,
    Land2 = 1,
    Land3 = 2,
}

#[derive(Debug, Clone, Copy)]
pub enum RailSprite {
    Straight = 0,
    CurvedRight = 1,
}

#[derive(Debug, Clone, Copy)]
pub enum VehicleSprite {
    GreyBox = 0,
    PurpleBullet = 1,
}

/// A [`Bundle`] of components for drawing a single sprite from a sprite sheet
#[derive(Bundle, Clone, Debug, Default)]
pub struct BaseSpriteBundle {
    /// Specifies the rendering properties of the sprite, such as color tint and flip.
    pub sprite: Sprite,
    /// The sprite sheet base texture
    pub texture: Handle<Image>,
    /// The sprite sheet texture atlas, allowing to draw a custom section of `texture`.
    pub atlas: TextureAtlas,
    /// The spatial properties of the sprite, including position and visiblity.
    pub spatial: SpatialBundle,
}

impl SpriteAssets {
    fn sprite_bundle(
        tex_atlas: &(Handle<TextureAtlasLayout>, Handle<Image>),
        index: usize,
        z: f32,
    ) -> BaseSpriteBundle {
        BaseSpriteBundle {
            sprite: Sprite {
                custom_size: Some(Vec2::splat(TILE_SCALE)),
                ..Default::default()
            },
            texture: tex_atlas.1.clone(),
            atlas: TextureAtlas {
                layout: tex_atlas.0.clone(),
                index: index,
                ..Default::default()
            },
            spatial: SpatialBundle::from_transform(Transform::from_translation(Vec3::Z * z)),
            ..Default::default()
        }
    }
    pub fn terrain_sprite(&self, sprite: TerrainSprite) -> BaseSpriteBundle {
        Self::sprite_bundle(&self.terrain, sprite as usize, Z_LAYER_TERRAIN)
    }

    pub fn rail_sprite(&self, sprite: RailSprite) -> BaseSpriteBundle {
        Self::sprite_bundle(&self.rails, sprite as usize, Z_LAYER_RAILS)
    }

    pub fn vehicle_sprite(&self, sprite: VehicleSprite) -> BaseSpriteBundle {
        Self::sprite_bundle(&self.vehicles, sprite as usize, Z_LAYER_TRAINS)
    }
}

/// This system loads the sprite atlases from disk.
fn load_texture_atlases(
    mut commands: Commands,
    mut texture_atlas: ResMut<Assets<TextureAtlasLayout>>,
    asset_server: Res<AssetServer>,
) {
    let terrain = TextureAtlasLayout::from_grid(
        UVec2::new(TILE_RESOLUTION, TILE_RESOLUTION),
        1,
        3,
        Some(UVec2::splat(2 * TILE_PADDING)),
        Some(UVec2::splat(TILE_PADDING)),
    );
    let rails = TextureAtlasLayout::from_grid(
        UVec2::new(TILE_RESOLUTION, TILE_RESOLUTION),
        1,
        2,
        Some(UVec2::splat(2 * TILE_PADDING)),
        Some(UVec2::splat(TILE_PADDING)),
    );
    let vehicles = TextureAtlasLayout::from_grid(
        UVec2::new(TILE_RESOLUTION, TILE_RESOLUTION),
        1,
        2,
        Some(UVec2::splat(2 * TILE_PADDING)),
        Some(UVec2::splat(TILE_PADDING)),
    );

    let terrain_tex = asset_server.load("TerrainAtlas.png");
    let rails_tex = asset_server.load("RailAtlas.png");
    let vehicles_tex = asset_server.load("TrainAtlas.png");

    commands.insert_resource(SpriteAssets {
        terrain: (texture_atlas.add(terrain), terrain_tex),
        rails: (texture_atlas.add(rails), rails_tex),
        vehicles: (texture_atlas.add(vehicles), vehicles_tex),
    });
}

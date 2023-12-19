use bevy::prelude::*;

use crate::tilemap::TILE_SCALE;

/// Texture resolution for a single tile.
const TILE_RESOLUTION: f32 = 128.;

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
pub struct SpriteAtlases {
    pub terrain: Handle<TextureAtlas>,
    pub rails: Handle<TextureAtlas>,
    pub vehicles: Handle<TextureAtlas>,
}

#[derive(Debug, Clone, Copy)]
pub enum TerrainSprite {
    Green = 0,
    Red = 1,
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

impl SpriteAtlases {
    fn sprite_bundle(tex_atlas: &Handle<TextureAtlas>, index: usize, z: f32) -> SpriteSheetBundle {
        SpriteSheetBundle {
            sprite: TextureAtlasSprite {
                index: index,
                custom_size: Some(Vec2::splat(TILE_SCALE)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::Z * z,
                ..Default::default()
            },
            texture_atlas: tex_atlas.clone(),
            ..Default::default()
        }
    }
    pub fn terrain_sprite(&self, sprite: TerrainSprite) -> SpriteSheetBundle {
        Self::sprite_bundle(&self.terrain, sprite as usize, Z_LAYER_TERRAIN)
    }

    pub fn rail_sprite(&self, sprite: RailSprite) -> SpriteSheetBundle {
        Self::sprite_bundle(&self.rails, sprite as usize, Z_LAYER_RAILS)
    }

    pub fn vehicle_sprite(&self, sprite: VehicleSprite) -> SpriteSheetBundle {
        Self::sprite_bundle(&self.vehicles, sprite as usize, Z_LAYER_TRAINS)
    }
}

/// This system loads the sprite atlases.
///
/// They are 130M x 130N pixels, where each texture is 128px by 128px and spans 1 tile high.
/// (Wasted space at the right & left edges).
fn load_texture_atlases(
    mut commands: Commands,
    mut texture_atlas: ResMut<Assets<TextureAtlas>>,
    asset_server: Res<AssetServer>,
) {
    let terrain = TextureAtlas::from_grid(
        asset_server.load("TerrainAtlas.png"),
        Vec2::new(TILE_RESOLUTION, TILE_RESOLUTION),
        1,
        2,
        Some(Vec2::splat(2.0)),
        Some(Vec2::splat(1.0)),
    );
    let rails = TextureAtlas::from_grid(
        asset_server.load("RailAtlas.png"),
        Vec2::new(TILE_RESOLUTION, TILE_RESOLUTION),
        1,
        2,
        Some(Vec2::splat(2.0)),
        Some(Vec2::splat(1.0)),
    );
    let vehicles = TextureAtlas::from_grid(
        asset_server.load("TrainAtlas.png"),
        Vec2::new(TILE_RESOLUTION, TILE_RESOLUTION),
        1,
        2,
        Some(Vec2::splat(2.0)),
        Some(Vec2::splat(1.0)),
    );

    commands.insert_resource(SpriteAtlases {
        terrain: texture_atlas.add(terrain),
        rails: texture_atlas.add(rails),
        vehicles: texture_atlas.add(vehicles),
    });
}

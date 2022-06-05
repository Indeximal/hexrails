use crate::tilemap::*;
use bevy::prelude::*;

const Z_LAYER_TERRAIN: f32 = 0.1;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system_to_stage(StartupStage::PreStartup, load_texture_atlas)
            .add_startup_system(spawn_tiles);
    }
}

enum TerrainType {
    Green,
    Red,
}

#[derive(Component)]
struct Tile {}

fn spawn_tiles(mut commands: Commands, atlas_h: Res<TerrainAtlas>) {
    const RADIUS: i32 = 3;
    for y in -RADIUS..=RADIUS {
        for x in if y >= 0 {
            -RADIUS..=(RADIUS - y)
        } else {
            (-RADIUS - y)..=RADIUS
        } {
            spawn_terrain_tile(
                &mut commands,
                &atlas_h,
                match (x, y) {
                    (0, 0) => TerrainType::Red,
                    (_, _) => TerrainType::Green,
                },
                TileCoordinate(x, y),
            );
        }
    }
}

fn spawn_terrain_tile(
    commands: &mut Commands,
    atlas: &Res<TerrainAtlas>,
    terrain_type: TerrainType,
    position: TileCoordinate,
) {
    let index = match terrain_type {
        TerrainType::Green => 0,
        TerrainType::Red => 1,
    };
    let mut sprite = TextureAtlasSprite::new(index);
    sprite.custom_size = Some(Vec2::splat(TILE_SCALE));

    commands
        .spawn_bundle(SpriteSheetBundle {
            sprite: sprite,
            texture_atlas: atlas.0.clone(),
            transform: Transform {
                translation: position.into_world_pos().extend(Z_LAYER_TERRAIN),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(Name::new(format!("Tile {}", position)))
        .insert(Tile {})
        .insert(position);
}

struct TerrainAtlas(Handle<TextureAtlas>);

fn load_texture_atlas(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas: ResMut<Assets<TextureAtlas>>,
) {
    let image = asset_server.load("TerrainAtlas.png");
    let atlas = TextureAtlas::from_grid_with_padding(
        image,
        Vec2::new(TILE_SIZE, TILE_SIZE),
        1,
        2,
        Vec2::splat(1.0),
    );
    let atlas_handle = texture_atlas.add(atlas);
    commands.insert_resource(TerrainAtlas(atlas_handle));
}

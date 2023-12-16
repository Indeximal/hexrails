use crate::tilemap::*;
use bevy::prelude::*;

const Z_LAYER_TERRAIN: f32 = 0.1;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        // Todo: this need to be revamped with better ordering
        // but simply setting .before /.after doesn't work?
        // Maybe because the commands are only run after the stage
        app.add_startup_system_to_stage(StartupStage::PreStartup, load_texture_atlas)
            .add_startup_system_to_stage(StartupStage::PreStartup, spawn_terrain_root)
            .add_startup_system(spawn_tiles);
    }
}

#[derive(Component)]
struct TerrainRoot;

enum TerrainType {
    Green,
    Red,
}

#[derive(Component)]
struct Tile {}

/// This system spawns the root node for all the terrain sprites, useful mostly for inspecting.
fn spawn_terrain_root(mut commands: Commands) {
    commands
        .spawn_bundle(SpatialBundle::default())
        .insert(TerrainRoot)
        .insert(Name::new("Terrain"));
}

/// system to spawn some tiles in a hexagon
fn spawn_tiles(
    mut commands: Commands,
    atlas_h: Res<TerrainAtlas>,
    root_query: Query<Entity, With<TerrainRoot>>,
) {
    let root_entity = root_query.single();
    const RADIUS: i32 = 20;
    for y in -RADIUS..=RADIUS {
        for x in if y >= 0 {
            -RADIUS..=(RADIUS - y)
        } else {
            (-RADIUS - y)..=RADIUS
        } {
            spawn_terrain_tile(
                &mut commands,
                &atlas_h,
                root_entity,
                match (x, y) {
                    (0, 0) => TerrainType::Red,
                    (_, _) => TerrainType::Green,
                },
                TileCoordinate(x, y),
            );
        }
    }
}

/// helper function to spawn a tile
fn spawn_terrain_tile(
    commands: &mut Commands,
    atlas: &Res<TerrainAtlas>,
    root_entity: Entity,
    terrain_type: TerrainType,
    position: TileCoordinate,
) {
    let index = match terrain_type {
        TerrainType::Green => 0,
        TerrainType::Red => 1,
    };
    let mut sprite = TextureAtlasSprite::new(index);
    sprite.custom_size = Some(Vec2::splat(TILE_SCALE));

    let child = commands
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
        .insert(position)
        .id();

    commands.entity(root_entity).add_child(child);
}

struct TerrainAtlas(Handle<TextureAtlas>);

/// System to load the sprite sheet
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
        Vec2::splat(2.0),
        Vec2::splat(1.0),
    );
    let atlas_handle = texture_atlas.add(atlas);
    commands.insert_resource(TerrainAtlas(atlas_handle));
}

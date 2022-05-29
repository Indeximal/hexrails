use bevy::prelude::*;

pub const TILE_SCALE: f32 = 0.5;
pub const TILE_SIZE: f32 = 128.;
const SQRT3_HALF: f32 = 0.866025404;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system_to_stage(StartupStage::PreStartup, load_texture_atlas)
            .add_startup_system(spawn_tiles);
    }
}

#[derive(Component, Clone, Copy)]
pub struct TileCoordinate((i32, i32));

impl TileCoordinate {
    fn east(&self) -> TileCoordinate {
        TileCoordinate((self.0 .0 + 1, self.0 .1))
    }

    fn west(&self) -> TileCoordinate {
        TileCoordinate((self.0 .0 - 1, self.0 .1))
    }

    fn north_east(&self) -> TileCoordinate {
        TileCoordinate((self.0 .0, self.0 .1 + 1))
    }

    fn north_west(&self) -> TileCoordinate {
        TileCoordinate((self.0 .0 - 1, self.0 .1 + 1))
    }

    fn south_east(&self) -> TileCoordinate {
        TileCoordinate((self.0 .0 + 1, self.0 .1 - 1))
    }

    fn south_west(&self) -> TileCoordinate {
        TileCoordinate((self.0 .0, self.0 .1 - 1))
    }
}

impl From<TileCoordinate> for Vec2 {
    fn from(tc: TileCoordinate) -> Self {
        let (x, y) = tc.0;
        let yy = y as f32 * TILE_SCALE;
        Vec2::new(
            x as f32 * TILE_SCALE * SQRT3_HALF + SQRT3_HALF / 2. * yy,
            yy * 0.75,
        )
    }
}

enum TerrainType {
    Green,
    Red,
}

#[derive(Component)]
struct Tile {
    terrain_type: TerrainType,
}

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
                TileCoordinate((x, y)),
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
                translation: Vec3::from((position.into(), 100.)),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(Name::new("idk"))
        .insert(Tile { terrain_type })
        .insert(position);
}

struct TerrainAtlas(Handle<TextureAtlas>);

fn load_texture_atlas(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas: ResMut<Assets<TextureAtlas>>,
) {
    let image = asset_server.load("Grasstile.png");
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

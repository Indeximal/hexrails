use bevy::prelude::*;
use rand::seq::SliceRandom;

use crate::sprites::{SpriteAssets, TerrainSprite};
use crate::tilemap::Tile;

pub struct TerrainPlugin;
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        // Todo: this need to be revamped with better ordering
        // but simply setting .before /.after doesn't work?
        // Maybe because the commands are only run after the stage
        app.add_systems(PreStartup, spawn_terrain_root)
            .add_systems(Startup, spawn_tiles);
    }
}

#[derive(Component)]
struct TerrainRoot;

#[derive(Component)]
struct TileMarker;

enum TerrainType {
    Land,
}

/// This system spawns the root node for all the terrain sprites, useful mostly for inspecting.
fn spawn_terrain_root(mut commands: Commands) {
    commands
        .spawn(SpatialBundle::default())
        .insert(TerrainRoot)
        .insert(Name::new("Terrain"));
}

/// system to spawn some tiles in a hexagon
fn spawn_tiles(
    mut commands: Commands,
    assets: Res<SpriteAssets>,
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
            commands.entity(root_entity).with_children(|c| {
                c.spawn(terrain_tile_bundle(&assets, Tile(x, y), TerrainType::Land));
            });
        }
    }
}

/// Generates a (not deterministic) bundle for a tile entity for a given type and position
fn terrain_tile_bundle(
    assets: &SpriteAssets,
    position: Tile,
    terrain_type: TerrainType,
) -> impl Bundle {
    let &sprite_id = match terrain_type {
        TerrainType::Land => [
            TerrainSprite::Land1,
            TerrainSprite::Land1,
            TerrainSprite::Land2,
            TerrainSprite::Land3,
            TerrainSprite::Land3,
            TerrainSprite::Land3,
        ]
        .choose(&mut rand::thread_rng())
        .expect("constant array is non-empty"),
    };

    let mut sprite = assets.terrain_sprite(sprite_id);
    // Place in the world
    sprite.spatial.transform.translation += position.world_pos().extend(0.);

    (
        sprite,
        Name::new(format!("Tile {:?}", position)),
        TileMarker,
        position,
    )
}

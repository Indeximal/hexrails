use bevy::prelude::*;
use rand::seq::SliceRandom;

use crate::sprites::{SpriteAtlases, TerrainSprite};
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
    atlas: Res<SpriteAtlases>,
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
                &atlas,
                root_entity,
                TerrainType::Land,
                Tile(x, y),
            );
        }
    }
}

/// helper function to spawn a tile
fn spawn_terrain_tile(
    commands: &mut Commands,
    atlas: &SpriteAtlases,
    root_entity: Entity,
    terrain_type: TerrainType,
    position: Tile,
) {
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

    let mut sprite = atlas.terrain_sprite(sprite_id);
    sprite.transform.translation += position.world_pos().extend(0.);

    let child = commands
        .spawn(sprite)
        .insert(Name::new(format!("Tile {}", position)))
        .insert(TileMarker)
        .insert(position)
        .id();

    commands.entity(root_entity).add_child(child);
}

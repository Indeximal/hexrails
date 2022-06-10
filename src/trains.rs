use bevy::prelude::*;
use bevy_inspector_egui::Inspectable;

use crate::tilemap::{TileCoordinate, TileFace, TileSide, TILE_SCALE, TILE_SIZE};

const Z_LAYER_TRAINS: f32 = 0.3;

pub struct TrainPlugin;

impl Plugin for TrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system_to_stage(StartupStage::PreStartup, load_texture_atlas)
            .add_startup_system(create_test_train);
    }
}

#[derive(Component, Inspectable)]
pub struct TrainHead {
    pub path: Vec<TileFace>,
    pub path_progress: f32,
    pub velocity: f32,
}

#[derive(Component, Inspectable)]
pub struct TrainUnit {
    /// Starting with 0, this is subtracted from the path progress
    pub position: u32,
}

fn create_test_path() -> Vec<TileFace> {
    let mut path = Vec::new();
    for i in -5..=5 {
        path.push(TileFace {
            tile: TileCoordinate(i, 0),
            side: TileSide::EAST,
        })
    }
    path
}

/// System to spawn a train for now
fn create_test_train(mut commands: Commands, atlas: Res<TrainAtlas>) {
    commands
        .spawn()
        .insert_bundle(TransformBundle::default())
        .insert(Name::new("Testing Train"))
        .insert(TrainHead {
            path: create_test_path(),
            path_progress: 5.0,
            velocity: 0.1,
        })
        .with_children(|builder| {
            for i in 0..5 {
                let mut sprite = TextureAtlasSprite::new(if i == 0 { 0 } else { 1 });
                sprite.custom_size = Some(Vec2::splat(TILE_SCALE));
                builder
                    .spawn_bundle(SpriteSheetBundle {
                        sprite: sprite,
                        texture_atlas: atlas.0.clone(),
                        transform: Transform::from_translation(Vec3::Z * Z_LAYER_TRAINS),
                        ..Default::default()
                    })
                    .insert(TrainUnit { position: i })
                    .insert(Name::new(format!("Wagon {}", i)));
            }
        });
}

struct TrainAtlas(Handle<TextureAtlas>);

/// System to load the sprite sheet
fn load_texture_atlas(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas: ResMut<Assets<TextureAtlas>>,
) {
    let image = asset_server.load("TrainAtlas.png");
    let atlas = TextureAtlas::from_grid_with_padding(
        image,
        Vec2::new(TILE_SIZE, TILE_SIZE),
        1,
        2,
        Vec2::splat(1.0),
    );
    let atlas_handle = texture_atlas.add(atlas);
    commands.insert_resource(TrainAtlas(atlas_handle));
}

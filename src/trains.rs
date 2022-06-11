use bevy::{core::FixedTimestep, prelude::*};
use bevy_inspector_egui::Inspectable;

use crate::{
    railroad::RailGraph,
    tilemap::{TileCoordinate, TileFace, TileSide, TILE_SCALE, TILE_SIZE},
};

const Z_LAYER_TRAINS: f32 = 0.3;

pub struct TrainPlugin;

impl Plugin for TrainPlugin {
    fn build(&self, app: &mut App) {
        // Todo: don't rely on stages
        app.add_startup_system_to_stage(StartupStage::PreStartup, load_texture_atlas)
            .add_startup_system_to_stage(StartupStage::PostStartup, create_test_train)
            .add_system(move_trains)
            // Todo: move to iyes_loopless instead
            .add_system_set(
                SystemSet::new()
                    .with_run_criteria(FixedTimestep::steps_per_second(20.))
                    .with_system(drive_trains_tick),
            );
    }
}

#[derive(Component, Inspectable)]
pub struct TrainHead {
    /// From 0 at the start to len at the destination
    pub path: Vec<TileFace>,
    /// A fractional index into path
    pub path_progress: f32,
    pub velocity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrainUnitType {
    Locomotive,
    Wagon,
}

#[derive(Component, Inspectable)]
pub struct TrainUnit {
    /// Starting with 0, this is subtracted from the path progress
    pub position: u32,
}

fn drive_trains_tick(mut trains: Query<&mut TrainHead>) {
    for mut train in trains.iter_mut() {
        train.path_progress += train.velocity;
        train.path_progress = train.path_progress.clamp(0., (train.path.len() - 1) as f32);
    }
}

/// System to update the tranform of the train wagons.
/// Precondition: progress <= path.len() - 1
fn move_trains(
    mut train_wagons: Query<(&Parent, &mut Transform, &TrainUnit)>,
    trains: Query<&TrainHead>,
) {
    for (parent, mut transform, unit) in train_wagons.iter_mut() {
        let head = trains
            .get(parent.0)
            .expect("Train Unit did not have a Train Head as a Parent");

        let progress = head.path_progress - unit.position as f32;
        if progress < 1. {
            error!("Train Unit is not on path!");
            continue;
        }
        let render_progress = progress - 0.5;
        let start = head.path[render_progress.ceil() as usize - 1];
        let end = head.path[render_progress.ceil() as usize];
        move_train_unit(transform.as_mut(), start, end, render_progress.fract())
    }
}

fn face_position(face: TileFace) -> Vec2 {
    let origin = face.tile.into_world_pos();
    let angle = face.side.to_angle();
    let offset = Vec2::new(angle.cos(), angle.sin()) * TILE_SCALE / 2.;
    origin + offset
}

fn move_train_unit(output: &mut Transform, start: TileFace, end: TileFace, t: f32) {
    let start_pos = face_position(start);
    let end_pos = face_position(end);
    let pos = start_pos * (1. - t) + end_pos * t;
    let start_angle = start.side.to_angle();
    let end_angle = end.side.to_angle();
    let angle = start_angle * (1. - t) + end_angle * t;
    output.translation = pos.extend(Z_LAYER_TRAINS);
    output.rotation = Quat::from_rotation_z(angle);
}

fn create_test_path(rail_graph: &RailGraph) -> Vec<TileFace> {
    let start = TileFace {
        tile: TileCoordinate(-7, 0),
        side: TileSide::WEST,
    };
    let end = TileFace {
        tile: TileCoordinate(0, 7),
        side: TileSide::SOUTH_WEST,
    };

    let result = petgraph::algo::astar(
        &rail_graph.graph,
        start,
        |v| v == end,
        |_| 1.0,
        |e| {
            end.tile
                .into_world_pos()
                .distance_squared(e.tile.into_world_pos())
        },
    );

    result.expect("No path found!").1
}

/// System to spawn a train for now
fn create_test_train(mut commands: Commands, atlas: Res<TrainAtlas>, rail_graph: Res<RailGraph>) {
    let graph = rail_graph.as_ref();
    commands
        .spawn()
        .insert_bundle(TransformBundle::default())
        .insert(Name::new("Testing Train"))
        .insert(TrainHead {
            path: create_test_path(graph),
            path_progress: 5.0,
            velocity: 0.05,
        })
        .with_children(|builder| {
            for i in 0..4 {
                let mut sprite = TextureAtlasSprite::new(if i == 0 { 1 } else { 0 });
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

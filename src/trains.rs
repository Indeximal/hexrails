use std::f32::consts::PI;

use bevy::{core::FixedTimestep, prelude::*};
use bevy_inspector_egui::Inspectable;
use petgraph::EdgeDirection;

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
            .add_system(position_train_units)
            // Todo: move to iyes_loopless instead
            .add_system_set(
                SystemSet::new()
                    .with_run_criteria(FixedTimestep::steps_per_second(20.))
                    .with_system(tick_trains),
            );
    }
}

#[derive(Component, Inspectable)]
pub struct TrainHead {
    /// From 0 at the start to len at the destination. Must be contain at least two values.
    pub path: Vec<TileFace>,
    /// A fractional index into path
    pub path_progress: f32,
    /// Current change of progress per tick. Should be non-negative
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

/// Fixed timestep system to update the progress of the trains
fn tick_trains(mut trains: Query<&mut TrainHead>, graph_res: Res<RailGraph>) {
    let graph = graph_res.as_ref();
    for mut train in trains.iter_mut() {
        train.path_progress += train.velocity;
        let max_progress = (train.path.len() - 1) as f32;
        if train.path_progress > max_progress {
            // attempt to extend the path if necessary: needed for manual driving
            graph
                .graph
                .neighbors_directed(
                    train
                        .path
                        .last()
                        .expect("TrainHead::path invariant broken!")
                        .clone(),
                    EdgeDirection::Outgoing,
                )
                .any(|a| {
                    // Todo: implement steering
                    // The remove operation is there to stop the path from growing continiously.
                    // But it does use O(n) time, but since n should stay constant this way, this
                    // is fine.
                    train.path.remove(0);
                    train.path_progress -= 1.;
                    train.path.push(a);
                    true // shortcircuit
                });
        }
        train.path_progress = train.path_progress.clamp(0., (train.path.len() - 1) as f32);
    }
}

/// System to update the transform of the train wagons.
/// Precondition: progress <= path.len() - 1
fn position_train_units(
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
        let start = head.path[render_progress.floor() as usize];
        let end = head.path[render_progress.floor() as usize + 1];
        move_train_unit(transform.as_mut(), start, end, render_progress.fract())
    }
}

fn face_position(face: TileFace) -> Vec2 {
    let origin = face.tile.into_world_pos();
    let angle = face.side.to_angle();
    let offset = Vec2::new(angle.cos(), angle.sin()) * TILE_SCALE / 2.;
    origin + offset
}

/// Helper to change the `output` Transform to intrapolate the start and end position and rotation
fn move_train_unit(output: &mut Transform, start: TileFace, end: TileFace, t: f32) {
    // todo: actually move in an arc and not linear
    let start_pos = face_position(start);
    let end_pos = face_position(end);
    let pos = start_pos * (1. - t) + end_pos * t;
    let start_angle = start.side.to_angle();
    let end_angle = end.side.to_angle();
    // From https://gist.github.com/shaunlebron/8832585
    let da = (end_angle - start_angle) % (2. * PI);
    let angle_diff = (2. * da) % (2. * PI) - da;
    let angle = start_angle + angle_diff * t;
    output.translation = pos.extend(Z_LAYER_TRAINS);
    output.rotation = Quat::from_rotation_z(angle);
}

/// Temporary helper to search for a hardcorded path
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

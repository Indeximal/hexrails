use std::f32::consts::PI;

use bevy::{core::FixedTimestep, ecs::schedule::ShouldRun, prelude::*};
use bevy_inspector_egui::Inspectable;
use petgraph::EdgeDirection;

use crate::{
    railroad::RailGraph,
    tilemap::{TileClickEvent, TileCoordinate, TileFace, TileSide, TILE_SCALE, TILE_SIZE},
    ui::InteractingState,
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
            )
            .add_system_set(
                SystemSet::new()
                    .with_run_criteria(train_builder_condition)
                    .with_system(train_builder),
            );
    }
}

#[derive(Component, Inspectable)]
pub struct TrainHead {
    /// A fractional index into path, where the front of the train is.
    pub path_progress: f32,
    /// Current change of progress per tick. Should be non-negative
    pub velocity: f32,
    /// Amount of wagons and locomotives. Must be equal to the number of TrainUnit children.
    pub length: u32,
    /// From 0 at the start to len at the destination. Must contain at least two values.
    pub path: Vec<TileFace>,
}

impl TrainHead {
    /// Returns the index of the wagon currently nearest to `face` or `length` if at the end.
    /// Returns none if the face is not on the path or not near any wagon.
    fn index_for_tile(&self, face: TileFace) -> Option<u32> {
        if self.path_progress - self.length as f32 <= 1. {
            // no space for a new wagon
            // todo: extend path instead ?
            return None;
        }
        let path_index = self.path.iter().position(|&f| f == face)? as f32;
        let index = self.path_progress - path_index;
        if index < 0. {
            None
        } else if index >= self.length as f32 + 1. {
            None
        } else {
            Some(index.floor() as u32)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrainUnitType {
    Locomotive,
    Wagon,
}

impl TrainUnitType {
    fn into_texture_atlas_index(&self) -> usize {
        match self {
            Self::Locomotive => 1,
            Self::Wagon => 0,
        }
    }
}

#[derive(Component, Inspectable)]
pub struct TrainUnit {
    /// Starting with 0, this is subtracted from the path progress
    pub position: u32,
}

fn train_builder_condition(state: Res<State<InteractingState>>) -> ShouldRun {
    match state.current() {
        InteractingState::PlaceTrains(_) => ShouldRun::Yes,
        _ => ShouldRun::No,
    }
}

/// This system tries to place a train wagon or a new train on click
fn train_builder(
    mut commands: Commands,
    atlas: Res<TrainAtlas>,
    mut click_event: EventReader<TileClickEvent>,
    rail_graph: Res<RailGraph>,
    state: Res<State<InteractingState>>,
    mut trains: Query<(Entity, &Children, &mut TrainHead)>,
    wagons: Query<&mut TrainUnit>,
) {
    let graph = rail_graph.as_ref();
    let trail_type = match state.current() {
        InteractingState::PlaceTrains(v) => v.clone(),
        _ => unreachable!(
            "The run condition should insure that the train builder is only run in the PlaceTrains state!"
        ),
    };
    for ev in click_event.iter() {
        if ev.side.is_none() {
            // for now ignore clicks in the center: might be ambigous
            continue;
        }
        // todo: also consider the opposite tile face
        let face = TileFace {
            tile: ev.coord,
            side: ev.side.unwrap(),
        };
        let neighbor_count = graph
            .graph
            .neighbors_directed(face, EdgeDirection::Outgoing)
            .count();
        if neighbor_count == 0 {
            // skip if there is no rail in the graph at this position
            continue;
        }

        for (parent, children, mut train) in trains.iter_mut() {
            if let Some(new_index) = train.index_for_tile(face) {
                insert_wagon(
                    &mut commands,
                    &atlas,
                    children,
                    wagons,
                    parent,
                    trail_type.clone(),
                    new_index,
                );
                train.length += 1;
                break;
            }
        }
        // todo: remove limit for one click per frame, needed for borrow checker. (Why?)
        break;
    }
}

fn insert_wagon(
    commands: &mut Commands,
    atlas: &TrainAtlas,
    sibling_ids: &Children,
    mut sibling_query: Query<&mut TrainUnit>,
    parent: Entity,
    wagon_type: TrainUnitType,
    insert_index: u32,
) {
    info!("Inserting wagon at {}", insert_index);

    // Shift wagons back
    for sibling_id in sibling_ids.iter() {
        if let Ok(mut sibling) = sibling_query.get_mut(sibling_id.clone()) {
            if sibling.position >= insert_index {
                sibling.position += 1;
            }
        }
    }

    let mut sprite = TextureAtlasSprite::new(wagon_type.into_texture_atlas_index());
    sprite.custom_size = Some(Vec2::splat(TILE_SCALE));
    let new_wagon = commands
        .spawn_bundle(SpriteSheetBundle {
            sprite: sprite,
            texture_atlas: atlas.0.clone(),
            transform: Transform::from_translation(Vec3::Z * Z_LAYER_TRAINS),
            ..Default::default()
        })
        .insert(TrainUnit {
            position: insert_index,
        })
        .insert(Name::new("Wagon ..."))
        .id();
    commands.entity(parent).add_child(new_wagon);
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

/// Temporary system to spawn a train for now
fn create_test_train(mut commands: Commands, atlas: Res<TrainAtlas>, rail_graph: Res<RailGraph>) {
    let graph = rail_graph.as_ref();
    commands
        .spawn()
        .insert_bundle(TransformBundle::default())
        .insert(Name::new("Testing Train"))
        .insert(TrainHead {
            path: create_test_path(graph),
            path_progress: 5.0,
            velocity: 0.0,
            length: 4,
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

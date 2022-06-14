use std::{f32::consts::PI, ops::Add};

use bevy::{core::FixedTimestep, ecs::schedule::ShouldRun, prelude::*};
use bevy_inspector_egui::Inspectable;
use petgraph::EdgeDirection;
use serde::{Deserialize, Serialize};

use crate::{
    railroad::RailGraph,
    tilemap::{TileClickEvent, TileFace, TILE_SCALE, TILE_SIZE},
    ui::InteractingState,
};

const Z_LAYER_TRAINS: f32 = 0.3;

pub struct TrainPlugin;

impl Plugin for TrainPlugin {
    fn build(&self, app: &mut App) {
        // Todo: don't rely on stages. Even possible because of Commands dependencies?
        app.add_startup_system_to_stage(StartupStage::PreStartup, load_texture_atlas)
            .add_system_to_stage(CoreStage::PostUpdate, position_train_units)
            // Todo: move to iyes_loopless instead
            .add_system_set(
                SystemSet::new()
                    .with_run_criteria(FixedTimestep::steps_per_second(60.))
                    .with_system(tick_velocity.before(tick_trains))
                    .with_system(tick_trains),
            )
            .add_system_set(
                SystemSet::new()
                    .with_run_criteria(train_builder_condition)
                    .with_system(train_builder),
            )
            .add_system(manual_train_driving);
    }
}

#[derive(Component)]
pub struct PlayerControlledTrain;

#[derive(Component, Inspectable, Serialize, Deserialize)]
pub struct WagonStats {
    pub weight: f32,
    pub acceleration_power: f32,
    pub braking_power: f32,
}

impl WagonStats {
    fn default_for_type(wagon_type: TrainUnitType) -> Self {
        match wagon_type {
            TrainUnitType::Locomotive => Self {
                weight: 25000.0,
                acceleration_power: 8.0,
                braking_power: 10.0,
            },
            TrainUnitType::Wagon => Self {
                weight: 5000.0,
                acceleration_power: 0.0,
                braking_power: 5.0,
            },
        }
    }

    fn additive_identiy() -> Self {
        WagonStats {
            weight: 0.,
            acceleration_power: 0.,
            braking_power: 0.,
        }
    }
}

impl Add<&WagonStats> for WagonStats {
    type Output = WagonStats;

    fn add(self, rhs: &WagonStats) -> Self::Output {
        WagonStats {
            weight: self.weight + rhs.weight,
            acceleration_power: self.acceleration_power + rhs.acceleration_power,
            braking_power: self.braking_power + rhs.braking_power,
        }
    }
}

#[derive(Component, Inspectable, Serialize, Deserialize)]
pub struct TrainHead {
    /// A fractional index into path, where the front of the train is.
    /// Must obey `length + 1 < path_progress <= path.len() - 1` (todo: check)
    pub path_progress: f32,
    /// Amount of wagons and locomotives. Must be equal to the number of TrainUnit children.
    pub length: u32,
    /// From 0 at the start to len at the destination. Must contain at least two values.
    pub path: Vec<TileFace>,
}

#[derive(Component, Inspectable, Serialize, Deserialize)]
pub struct Velocity {
    /// Current change of progress per tick. Should be non-negative
    pub velocity: f32,
    /// Current change of velocity per tick.
    pub acceleration: f32,
    /// Velocity will be clamped to this value.
    pub max_velocity: f32,
}

#[derive(Bundle)]
pub struct TrainBundle {
    pub controller: TrainHead,
    pub velocity: Velocity,

    /// Currently always "Train" for inspection
    pub name: Name,
    /// always default, used for hierarchy
    pub local_transform: Transform,
    pub global_transform: GlobalTransform,
}

impl TrainBundle {
    fn new(controller: TrainHead, max_velocity: f32) -> Self {
        Self {
            controller: controller,
            velocity: Velocity {
                velocity: 0.0,
                acceleration: 0.0,
                max_velocity: max_velocity,
            },
            name: Name::new("Train"),
            local_transform: Default::default(),
            global_transform: Default::default(),
        }
    }
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

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// System to set the acceleration of the player driven train
fn manual_train_driving(
    mut train: Query<(&mut Velocity, &Children), With<PlayerControlledTrain>>,
    stats: Query<&WagonStats>,
    input: Res<Input<KeyCode>>,
) {
    for (mut velocity, children) in train.iter_mut() {
        let total_stats = children
            .iter()
            .filter_map(|&id| stats.get(id).ok())
            .fold(WagonStats::additive_identiy(), WagonStats::add);

        let mut acceleration = 0.0;
        if input.pressed(KeyCode::Up) {
            acceleration += total_stats.acceleration_power;
        }
        if input.pressed(KeyCode::Down) {
            acceleration -= total_stats.braking_power;
        }
        // allow for somewhat of a one pedal drive
        if acceleration == 0.0 {
            acceleration = -10.0;
        }
        velocity.acceleration = acceleration / total_stats.weight;
    }
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
    let wagon_type = match state.current() {
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

        let mut found_train = false;
        for (parent, children, mut train) in trains.iter_mut() {
            if let Some(new_index) = train.index_for_tile(face) {
                insert_wagon(
                    &mut commands,
                    &atlas,
                    children,
                    wagons,
                    parent,
                    wagon_type.clone(),
                    WagonStats::default_for_type(wagon_type),
                    new_index,
                );
                train.length += 1;
                found_train = true;
                break;
            }
        }
        if !found_train {
            create_new_train(&mut commands, &atlas, face, &graph, wagon_type);
        }
        // todo: remove limit for one click per frame, needed for borrow checker. (Why?)
        break;
    }
}

/// Creates a new train with a single wagon.
/// Precondition: `face` is in the `rail_graph` and has a neighbor.
fn create_new_train(
    commands: &mut Commands,
    atlas: &TrainAtlas,
    face: TileFace,
    rail_graph: &RailGraph,
    wagon_type: TrainUnitType,
) {
    info!("Creating train at @{}", face.tile);

    let next_face = rail_graph
        .graph
        .neighbors_directed(face, EdgeDirection::Outgoing)
        .next()
        .expect("Broke precondition: `face is in the graph and has a neighbor`!");

    let first_wagon = spawn_wagon(
        commands,
        atlas,
        wagon_type,
        WagonStats::default_for_type(wagon_type),
        0,
    );

    commands
        .spawn_bundle(TrainBundle::new(
            TrainHead {
                path: vec![face, next_face],
                path_progress: 1.0,
                length: 1,
            },
            20. / 60.,
        ))
        .insert(PlayerControlledTrain) // todo: actually select this
        .add_child(first_wagon);
}

/// Helper to insert a wagon into an existing train and move all other wagons accordingly
fn insert_wagon(
    commands: &mut Commands,
    atlas: &TrainAtlas,
    sibling_ids: &Children,
    mut sibling_query: Query<&mut TrainUnit>,
    parent: Entity,
    wagon_type: TrainUnitType,
    wagon_stats: WagonStats,
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

    let new_wagon = spawn_wagon(commands, atlas, wagon_type, wagon_stats, insert_index);
    commands.entity(parent).add_child(new_wagon);
}

// Helper to spawn a wagon sprite
pub fn spawn_wagon(
    commands: &mut Commands,
    atlas: &TrainAtlas,
    wagon_type: TrainUnitType,
    wagon_stats: WagonStats,
    insert_index: u32,
) -> Entity {
    let mut sprite = TextureAtlasSprite::new(wagon_type.into_texture_atlas_index());
    sprite.custom_size = Some(Vec2::splat(TILE_SCALE));

    commands
        .spawn_bundle(SpriteSheetBundle {
            sprite: sprite,
            texture_atlas: atlas.0.clone(),
            transform: Transform::from_translation(Vec3::Z * Z_LAYER_TRAINS),
            ..Default::default()
        })
        .insert(TrainUnit {
            position: insert_index,
        })
        .insert(wagon_type)
        .insert(wagon_stats)
        .insert(Name::new("Wagon"))
        .id()
}

/// System to apply acceleration to the velocity
fn tick_velocity(mut velocities: Query<&mut Velocity>) {
    for mut velocity in velocities.iter_mut() {
        velocity.velocity =
            (velocity.velocity + velocity.acceleration).clamp(0., velocity.max_velocity);
    }
}

/// Fixed timestep system to update the progress of the trains
fn tick_trains(mut trains: Query<(&mut TrainHead, &Velocity)>, graph_res: Res<RailGraph>) {
    let graph = graph_res.as_ref();
    for (mut train, velocity) in trains.iter_mut() {
        train.path_progress += velocity.velocity;
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
                    if train.path.len() >= train.length as usize + 5 {
                        // The remove operation is there to stop the path from growing continiously.
                        // But it does use O(n) time, but since n should stay constant this way, this
                        // is fine.
                        train.path.remove(0);
                        train.path_progress -= 1.;
                    }
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

pub struct TrainAtlas(Handle<TextureAtlas>);

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

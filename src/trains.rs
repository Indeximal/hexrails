use std::{f32::consts::PI, ops::Add};

use bevy::{core::FixedTimestep, prelude::*};
// use bevy_inspector_egui::Inspectable;
use petgraph::EdgeDirection;
use serde::{Deserialize, Serialize};

use crate::{
    railroad::{RailGraph, RailType},
    tilemap::{TileClickEvent, TileFace},
    ui::InteractingState,
};

pub struct TrainPlugin;

impl Plugin for TrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_to_stage(CoreStage::PostUpdate, position_train_units)
            .add_system_set(
                SystemSet::new()
                    .with_run_criteria(FixedTimestep::steps_per_second(60.))
                    .with_system(tick_velocity.before(tick_trains))
                    .with_system(tick_trains),
            )
            .add_system(manual_train_driving)
            .add_system(auto_extend_train_path)
            .add_system(reverse_train_system)
            .add_system_set(
                SystemSet::on_update(InteractingState::SelectTrain)
                    // after, so that acceleration gets cleared properly
                    .with_system(train_selection_system.after(manual_train_driving)),
            );
    }
}

#[derive(Component)]
pub struct PlayerControlledTrain;

#[derive(Component, Serialize, Deserialize)]
pub struct WagonStats {
    pub weight: f32,
    pub acceleration_power: f32,
    pub braking_power: f32,
}

impl WagonStats {
    pub fn default_for_type(wagon_type: TrainUnitType) -> Self {
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

    pub fn additive_identiy() -> Self {
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

#[derive(Component, Serialize, Deserialize)]
pub struct TrainHead {
    /// A fractional index into path, where the front of the train is.
    /// Must obey `length + 1 < path_progress <= path.len() - 1` (todo: check)
    pub path_progress: f32,
    /// Amount of wagons and locomotives. Must be equal to the number of TrainUnit children.
    pub length: u32,
    /// From 0 at the start to len at the destination. Must contain at least two values.
    pub path: Vec<TileFace>,
}

impl TrainHead {
    /// Shortens the path to not contain any extra tiles in front
    pub fn trim_front(&mut self) {
        while self.path.len() as f32 > self.path_progress + 2.0 {
            self.path.pop();
        }
    }
}

#[derive(Component, Serialize, Deserialize)]
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
    pub fn new(controller: TrainHead, max_velocity: f32) -> Self {
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

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrainUnitType {
    Locomotive,
    Wagon,
}

#[derive(Component)]
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

/// System to extend the path of trains if necessary. Useful mosty for manual driving.
fn auto_extend_train_path(
    mut trains: Query<(&mut TrainHead, Option<&PlayerControlledTrain>)>,
    input: Res<Input<KeyCode>>,
    graph_res: Res<RailGraph>,
) {
    let graph = &graph_res.graph;
    let preferrs_left = input.pressed(KeyCode::Left);
    let preferrs_right = input.pressed(KeyCode::Right);
    let preferred_direction = if preferrs_left == preferrs_right {
        RailType::Straight
    } else if preferrs_left {
        RailType::CurvedLeft
    } else {
        RailType::CurvedRight
    };

    for (mut train, is_player_controlled) in trains.iter_mut() {
        let max_progress = (train.path.len() - 1) as f32;
        // todo: does this put a hard limit on the velocity of player controlled trains?
        // The 0.5 anticipates the future
        if train.path_progress + 0.5 > max_progress {
            let path_end = train
                .path
                .last()
                .expect("TrainHead::path invariant broken: contains no elements")
                .clone();
            let mut next_tile = None;
            graph
                .edges_directed(path_end, EdgeDirection::Outgoing)
                .for_each(|(_, next, edge)| {
                    // Prefer input direction, if this train is steered by a player
                    if is_player_controlled.is_some() && edge.rail_type == preferred_direction {
                        next_tile = Some(next);
                    } else if next_tile.is_none() {
                        next_tile = Some(next);
                    }
                });

            if let Some(next_tile) = next_tile {
                train.path.push(next_tile);
                if train.path.len() >= train.length as usize + 5 {
                    // The remove operation is there to stop the path from growing continiously.
                    // But it does use O(n) time, but since n should stay constant this way, this
                    // is fine.
                    train.path.remove(0);
                    train.path_progress -= 1.;
                }
            }
        }
    }
}

/// System to reverse a whole train on key press
fn reverse_train_system(
    mut trains: Query<(&mut TrainHead, &Velocity, &Children), With<PlayerControlledTrain>>,
    mut wagons: Query<&mut TrainUnit>,
    input: Res<Input<KeyCode>>,
) {
    if !input.just_pressed(KeyCode::R) {
        return;
    }
    for (mut controller, velocity, children) in trains.iter_mut() {
        if velocity.velocity != 0. {
            info!("Cannot reverse moving train!");
            continue;
        }
        // Reverse the path and use reversed edges
        controller.path.reverse();
        for d in controller.path.iter_mut() {
            *d = d.opposite();
        }
        controller.path_progress =
            controller.path.len() as f32 - controller.path_progress + controller.length as f32 - 1.;

        // Allow the player to steer
        controller.trim_front();

        // Reverse the wagon indices
        for &x in children.iter() {
            if let Ok(mut wagon) = wagons.get_mut(x) {
                wagon.position = controller.length - wagon.position - 1;
            }
        }
    }
}

/// System to enter and exit trains on click
/// This will likely get replace with circle colliders soon
fn train_selection_system(
    mut commands: Commands,
    mut controlled_train: Query<(Entity, &mut Velocity), With<PlayerControlledTrain>>,
    other_trains: Query<(Entity, &TrainHead), Without<PlayerControlledTrain>>,
    mut click_event: EventReader<TileClickEvent>,
) {
    // copied from trainbuilder::train_builder
    for ev in click_event.iter() {
        if let Ok((entity, mut velocity)) = controlled_train.get_single_mut() {
            velocity.acceleration = 0.0;
            commands.entity(entity).remove::<PlayerControlledTrain>();
        }

        if ev.side.is_none() {
            continue;
        }
        let face = TileFace {
            tile: ev.coord,
            side: ev.side.unwrap(),
        };

        for (entity, train) in other_trains.iter() {
            // this might behave weird when clicking on long paths, but it should
            // be fine, since this is temporary.
            if train.path.contains(&face) {
                info!("Selected train");
                commands.entity(entity).insert(PlayerControlledTrain);
                break;
            }
        }
        break;
    }
}

/// System to apply acceleration to the velocity
fn tick_velocity(mut velocities: Query<&mut Velocity>) {
    for mut velocity in velocities.iter_mut() {
        velocity.velocity =
            (velocity.velocity + velocity.acceleration).clamp(0., velocity.max_velocity);
    }
}
/// Fixed timestep system to update the progress of the trains
fn tick_trains(mut trains: Query<(&mut TrainHead, &Velocity)>) {
    for (mut train, velocity) in trains.iter_mut() {
        train.path_progress += velocity.velocity;
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

/// Helper to change the `output` Transform to intrapolate the start and end position and rotation
fn move_train_unit(output: &mut Transform, start: TileFace, end: TileFace, t: f32) {
    // todo: actually move in an arc and not linear
    let start_pos = start.into_world_position();
    let end_pos = end.into_world_position();
    let pos = start_pos * (1. - t) + end_pos * t;
    let start_angle = start.side.to_angle();
    let end_angle = end.side.to_angle();
    // From https://gist.github.com/shaunlebron/8832585
    let da = (end_angle - start_angle) % (2. * PI);
    let angle_diff = (2. * da) % (2. * PI) - da;
    let angle = start_angle + angle_diff * t;
    output.translation = pos.extend(output.translation.z);
    output.rotation = Quat::from_rotation_z(angle);
}

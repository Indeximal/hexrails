use std::{f32::consts::PI, ops::Add};

use bevy::prelude::*;
use petgraph::EdgeDirection;
use serde::{Deserialize, Serialize};

use crate::railroad::{RailGraph, Track, TrackType};
use crate::tilemap::Joint;

/// The length in meters that a single track covers.
///
/// I.e. the width of the hexagons and length of the vehicles in meters.
const METER_PER_TRACK: f32 = 10.;

pub struct TrainPlugin;
impl Plugin for TrainPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_seconds(1. / 60.))
            .add_systems(PostUpdate, position_train_units)
            .add_systems(
                FixedUpdate,
                (tick_velocity.before(tick_trains), tick_trains),
            )
            .add_systems(Update, manual_driving::throttling_system)
            .add_systems(Update, manual_driving::auto_extend_train_path)
            .add_systems(Update, manual_driving::reverse_train_system)
            .add_systems(
                Update,
                manual_driving::train_selection_system
                    // after, so that acceleration gets cleared properly
                    .after(manual_driving::throttling_system),
            );
    }
}

/// The components of an entity that make up a logical train.
///
/// Wagons and locomotives are children of this entity with components of [`VehicleBundle`].
#[derive(Bundle)]
pub struct TrainBundle {
    pub path: Trail,
    pub velocity: Velocity,
    pub controller: Controller,

    /// Currently always "Train" for inspection
    pub name: Name,
    /// Always default, used for hierarchy
    pub spatial: SpatialBundle,
}

/// A single vehicle, child to a [`TrainBundle`] entity.
///
/// Also has colliders as children.
#[derive(Bundle)]
pub struct VehicleBundle {
    pub index: TrainIndex,
    pub tyype: VehicleType,
    pub stats: VehicleStats,
    pub name: Name,

    /// The visuals, including transform and visibility.
    pub visuals: SpriteSheetBundle,
}

#[derive(Component, Serialize, Deserialize)]
pub struct Trail {
    /// From 0 at the start to len at the destination. Must contain at least `length + 1` values.
    pub path: Vec<Joint>,
    /// A fractional index into path, where the front of the train is.
    /// Must obey `length <= path_progress <= path.len() - 1` (todo: check)
    pub path_progress: f32,
    /// Amount of wagons and locomotives. Must be equal to the number of [`TrainIndex`] children.
    pub length: u16,
}

#[derive(Component, Serialize, Deserialize)]
pub struct VehicleStats {
    /// Inertial mass in tons of this vehicle
    pub weight: f32,
    /// roughly in kN of force when full throttle.
    pub acceleration_force: f32,
    /// Roughly kN of force applied when braking.
    ///
    /// In the UIC, "Bremsgewicht" in tons is used, see <https://de.wikipedia.org/wiki/Bremsgewicht>,
    /// but the relevant UIC Merkblatt 544-1 is not free and a constant force is easier.
    pub braking_force: f32,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VehicleType {
    Locomotive,
    Wagon,
}

#[derive(Component)]
pub struct TrainIndex {
    /// Starting with 0, this is subtracted from [`Train::path_progress`].
    pub position: u16,
}

/// The component attached to the collider children of vehicles.
#[derive(Component)]
pub enum BumperNode {
    Front,
    Back,
}

#[derive(Component, Reflect, Serialize, Deserialize)]
pub struct Velocity {
    /// Current velocity in m/s. Should be non-negative
    pub velocity: f32,
    /// `velocity` will be clamped to this value.
    /// TODO: move into stats
    pub max_velocity: f32,
    // Stats that affect the moving object are given by the sum of all vehicles.
}

#[derive(Component, Default)]
pub struct Controller {
    /// The fraction of the power being applied. Must be in the interval [0, 1].
    throttle: f32,
    /// The fraction of the brake being applied. Must be in the interval [0, 1].
    brake: f32,
}

#[derive(Component)]
pub struct PlayerControlledTrain;

impl VehicleStats {
    pub fn default_for_type(wagon_type: VehicleType) -> Self {
        match wagon_type {
            VehicleType::Locomotive => Self {
                weight: 84.0,
                acceleration_force: 300.0,
                braking_force: 200.0,
            },
            VehicleType::Wagon => Self {
                weight: 50.0,
                acceleration_force: 0.0,
                braking_force: 40.0,
            },
        }
    }

    pub fn additive_identiy() -> Self {
        VehicleStats {
            weight: 0.,
            acceleration_force: 0.,
            braking_force: 0.,
        }
    }
}

impl Add<&VehicleStats> for VehicleStats {
    type Output = VehicleStats;

    fn add(self, rhs: &VehicleStats) -> Self::Output {
        VehicleStats {
            weight: self.weight + rhs.weight,
            acceleration_force: self.acceleration_force + rhs.acceleration_force,
            braking_force: self.braking_force + rhs.braking_force,
        }
    }
}

impl Trail {
    /// Shortens the path to not contain any extra tiles in front
    pub fn trim_front(&mut self) {
        while self.path.len() as f32 > self.path_progress + 2.0 {
            self.path.pop();
        }
    }

    /// True if all (locally checkable) invariants are okay.
    #[inline]
    pub fn check_invariant(&self) -> bool {
        (self.path.len() >= self.length as usize + 1)
            && (self.path_progress >= self.length as f32 - 0.01)
            && (self.path_progress <= self.path.len() as f32 - 0.99)
    }
}

impl TrainBundle {
    pub fn new(trail: Trail, max_velocity: f32) -> Self {
        Self {
            path: trail,
            velocity: Velocity {
                velocity: 0.0,
                max_velocity,
            },
            controller: Default::default(),
            name: Name::new("Train"),
            spatial: Default::default(),
        }
    }
}

/// System to apply throttle/brake to the velocity
fn tick_velocity(
    time: Res<Time<Fixed>>,
    mut train: Query<(&Controller, &mut Velocity, &Children)>,
    stats: Query<&VehicleStats>,
) {
    for (controller, mut velocity, children) in train.iter_mut() {
        let total_stats = children
            .iter()
            .filter_map(|&id| stats.get(id).ok())
            .fold(VehicleStats::additive_identiy(), VehicleStats::add);

        // Model constant brake force
        let decceleration = controller.brake * total_stats.braking_force / total_stats.weight;

        // Model constant power force
        let acceleration =
            controller.throttle * total_stats.acceleration_force / total_stats.weight;

        let delta_velocity = (acceleration - decceleration) * time.delta_seconds();
        velocity.velocity = (velocity.velocity + delta_velocity).clamp(0., velocity.max_velocity);
    }
}

/// Fixed timestep system to update the progress of the trains
fn tick_trains(time: Res<Time<Fixed>>, mut trains: Query<(&mut Trail, &Velocity)>) {
    for (mut train, velocity) in trains.iter_mut() {
        train.path_progress += velocity.velocity * time.delta_seconds() / METER_PER_TRACK;
        // TODO: crash
        train.path_progress = train.path_progress.clamp(0., (train.path.len() - 1) as f32);
    }
}

/// System to update the transform of the train wagons.
/// Precondition: progress <= path.len() - 1
fn position_train_units(
    mut train_wagons: Query<(&Parent, &mut Transform, &TrainIndex)>,
    trains: Query<&Trail>,
) {
    for (parent, mut transform, unit) in train_wagons.iter_mut() {
        let head = trains
            .get(parent.get())
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
fn move_train_unit(output: &mut Transform, start: Joint, end: Joint, t: f32) {
    // todo: actually move in an arc and not linear
    let start_pos = start.world_position();
    let end_pos = end.world_position();
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

mod manual_driving {
    use crate::{
        input::{Action, GameInput},
        interact::NodeClickEvent,
        ui::InteractingState,
    };

    use super::*;

    /// System to extend the path of trains if necessary. Useful mosty for manual driving.
    pub(super) fn auto_extend_train_path(
        mut trains: Query<(&mut Trail, Option<&PlayerControlledTrain>)>,
        input: Res<GameInput>,
        graph_res: Res<RailGraph>,
    ) {
        let graph = &graph_res.graph;
        let steer_value = input.value(&Action::SwitchDirection);
        let preferred_direction = if steer_value > 0.0 {
            TrackType::CurvedRight
        } else if steer_value < 0.0 {
            TrackType::CurvedLeft
        } else {
            TrackType::Straight
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
                    .for_each(|(this, next, _edge)| {
                        // Prefer input direction, if this train is steered by a player
                        if is_player_controlled.is_some()
                            && (Track::from_joints(this, next))
                                .expect("Invariant: graph only has track edges")
                                .heading
                                == preferred_direction
                        {
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

    /// System to set the acceleration of the player driven train
    pub(super) fn throttling_system(
        mut train: Query<&mut Controller, With<PlayerControlledTrain>>,
        input: Res<GameInput>,
    ) {
        for mut controller in train.iter_mut() {
            controller.throttle = input.value(&Action::Accelerate);
            controller.brake = input.value(&Action::Brake);

            // allow for somewhat of a one pedal drive
            if !input.pressed(&Action::Brake) && !input.pressed(&Action::Accelerate) {
                controller.brake = 0.1;
            }
        }
    }

    /// System to reverse a whole train on key press
    pub(super) fn reverse_train_system(
        mut trains: Query<(&mut Trail, &Velocity, &Children), With<PlayerControlledTrain>>,
        mut wagons: Query<&mut TrainIndex>,
        input: Res<GameInput>,
    ) {
        if !input.just_pressed(&Action::Reverse) {
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
            controller.path_progress = controller.path.len() as f32 - controller.path_progress
                + controller.length as f32
                - 1.;

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
    pub(super) fn train_selection_system(
        mut commands: Commands,
        state: Res<State<InteractingState>>,
        mut click_event: EventReader<NodeClickEvent>,
        mut controlled_train: Query<(Entity, &mut Controller), With<PlayerControlledTrain>>,
        bumpers: Query<&Parent, With<BumperNode>>,
        vehicles: Query<&Parent>,
    ) {
        match state.get() {
            InteractingState::SelectTrain => {}
            _ => {
                // Events are irrelevant
                click_event.clear();
                return;
            }
        };

        // Skip all old events, why not. (please let this not lead to a bug xD)
        let Some(ev) = click_event.read().last() else {
            return;
        };
        let Ok(bump_parent) = bumpers.get(ev.node) else {
            return;
        };
        let Ok(vehicle_parent) = vehicles.get(bump_parent.get()) else {
            error!("BumperNode should always be attached to a vehicle!");
            return;
        };

        // Remove player control from previously active train and release all control.
        if let Ok((entity, mut control)) = controlled_train.get_single_mut() {
            control.throttle = 0.0;
            control.brake = 0.0;
            commands.entity(entity).remove::<PlayerControlledTrain>();
        }

        commands
            .entity(vehicle_parent.get())
            .insert(PlayerControlledTrain);

        info!("Selected train");
    }
}

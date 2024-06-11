use std::{f32::consts::PI, ops::Add};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::interact::{InteractSet, NodeClickEvent};
use crate::tilemap::Joint;

/// The length in meters that a single track covers.
///
/// I.e. the width of the hexagons and length of the vehicles in meters.
const METER_PER_TRACK: f32 = 10.;

pub struct TrainPlugin;
impl Plugin for TrainPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_seconds(1. / 64.))
            .add_event::<TrainClickEvent>()
            .add_systems(
                FixedUpdate,
                (tick_velocity.before(tick_trains), tick_trains),
            )
            .add_systems(PreUpdate, emit_train_events.after(InteractSet))
            .add_systems(PostUpdate, position_train_units);
    }
}

#[derive(Event)]
pub struct TrainClickEvent {
    /// The id of the [`TrainBundle`] entity.
    pub train: Entity,

    /// The "fence-post" index of where the train was clicked.
    ///
    /// I.e. between wagons with index `bumper_index` and `bumper_index+1`.
    pub bumper_index: u16,
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
    pub throttle: f32,
    /// The fraction of the brake being applied. Must be in the interval [0, 1].
    pub brake: f32,
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

fn emit_train_events(
    mut trigger: EventReader<NodeClickEvent>,
    mut writer: EventWriter<TrainClickEvent>,
    bumpers: Query<(&Parent, &BumperNode)>,
    vehicles: Query<(&TrainIndex, &Parent)>,
    trains: Query<(Entity, &Trail, &Children)>,
) {
    for ev in trigger.read() {
        if !ev.primary {
            // only trigger from one side, action should be symmetric.
            continue;
        }
        let Ok((bump_parent, bump_dir)) = bumpers.get(ev.node) else {
            continue;
        };
        let Ok((index, vehicle_parent)) = vehicles.get(bump_parent.get()) else {
            error!("BumperNode should always be attached to a vehicle!");
            continue;
        };
        let Ok((train, _trail, _train_children)) = trains.get(vehicle_parent.get()) else {
            error!("Vehicles should always be attached to a train!");
            continue;
        };

        let bumper_index = match bump_dir {
            BumperNode::Front => index.position,
            BumperNode::Back => index.position + 1,
        };

        // Debug some invariants:
        #[cfg(test)]
        {
            if !_trail.check_invariant() {
                error!("Trail invariant broken on {train:?}!");
            }
            if index.position >= _trail.length {
                error!(
                    "Found vehicle {:?} with index greater than train {train:?}'s length!",
                    bump_parent.get()
                );
            }
            if _train_children.len() != _trail.length as usize {
                error!("Train {train:?}'s length in trail does not match number of children!");
            }
        }

        debug!("Train {train:?} clicked at {bumper_index}");
        writer.send(TrainClickEvent {
            train,
            bumper_index,
        });
    }
}

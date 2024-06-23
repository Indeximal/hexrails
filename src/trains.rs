use std::{f32::consts::PI, ops::Add};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::ok_or_return;
use crate::tilemap::{Joint, Tile};

/// The length in meters that a single track covers.
///
/// I.e. the width of the hexagons and length of the vehicles in meters.
const METER_PER_TRACK: f32 = 10.;

pub struct TrainPlugin;
impl Plugin for TrainPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_seconds(1. / 64.))
            .add_systems(
                FixedUpdate,
                (tick_velocity.before(tick_trains), tick_trains),
            )
            .add_systems(PostUpdate, (position_train_units, update_tint));
    }
}

// ================================ TRAINS ===================================

/// The components of an entity that make up a logical train.
///
/// ## Additional components:
/// - Wagons and locomotives are [`Children`] of this entity with components of [`VehicleBundle`].
/// - At most one train has a [`PlayerControlledTrain`] attached, meaning it is the one
///   currently controlled by the player.
#[derive(Bundle)]
pub struct TrainBundle {
    pub marker: TrainMarker,
    pub path: Trail,
    pub velocity: Velocity,
    pub controller: Controller,

    /// Currently always "Train" for inspection
    pub name: Name,
    /// Always default, used for hierarchy only
    pub spatial: SpatialBundle,
}

#[derive(Component)]
pub struct TrainMarker;

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

#[derive(Component)]
pub struct Crashed;

// ================================ VEHICLES ===================================

/// A single vehicle, child to a [`TrainBundle`] entity.
///
/// ## Other components:
/// - Also has bumpers as children, with [`BumperNode`] and interaction colliders.
/// - Rapier collision stuff, i.e. a Cuboid collider and some flags.
#[derive(Bundle)]
pub struct VehicleBundle {
    pub index: TrainIndex,
    pub tyype: VehicleType,
    pub stats: VehicleStats,
    pub name: Name,

    /// The visuals, including transform and visibility.
    ///
    /// The transform gets set every frame to match the train position.
    pub visuals: SpriteSheetBundle,
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

// ============================= Vehicle parts ================================

/// The component attached to the collider children of vehicles.
#[derive(Component, Debug, Clone, Copy)]
pub enum BumperNode {
    Front,
    Back,
}

// =================================== impls ==================================

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
    /// Returns the point on the trail for a given fractional index
    /// as a pair of start, end joint plus an interpolation value.
    ///
    /// `index = 0` corresponds to the front bumper of the train,
    /// `index = length` to the back bumper.
    /// Interpolation value 0 means the first joint in the tuple, 1 the second.
    /// The first joint is always the one towards the back of the trail.
    ///
    /// Returns `None` if the index does not have joints anymore in this trail.
    /// This can be outside of the active segment of this trail.
    /// Should never return `None` if the trail is upholds invariants and
    /// `index` is in range [0, length).
    pub fn point_on_trail(&self, index: f32) -> Result<(Joint, Joint, f32), ()> {
        if !self.check_invariant() {
            return Err(());
        }
        let progress = self.path_progress - index;
        let &start = self.path.get(progress.floor() as usize).ok_or(())?;
        let &end = self.path.get(progress.floor() as usize + 1).ok_or(())?;
        Ok((start, end, progress.fract()))
    }

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

/// This is sligthly weird, but way more compact debug representation
///
/// todo: explain how to read this better, the idea is that the active part is
/// hightlighted, but right now, the split is not between the active splits,
/// but around the...
impl std::fmt::Debug for Trail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn map_tile_only(j: &Joint) -> &Tile {
            &j.tile
        }

        f.debug_list()
            .entries(
                self.path[..(self.path_progress.floor() as usize - self.length as usize)]
                    .iter()
                    .map(map_tile_only),
            )
            .entry(&"|")
            .entries(self.trim().iter().map(map_tile_only))
            .entry(&format!("{:.2} >", self.path_progress))
            .entries(
                self.path
                    .iter()
                    .skip((self.path_progress.ceil() as usize) + 1)
                    .map(map_tile_only),
            )
            .finish()
    }
}

impl TrainBundle {
    pub fn new(trail: Trail, max_velocity: f32) -> Self {
        Self {
            marker: TrainMarker,
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

// ================================ SYSTEMS ===================================

/// System to apply throttle/brake to the velocity
fn tick_velocity(
    time: Res<Time<Fixed>>,
    mut train: Query<
        (&Controller, &mut Velocity, &Children),
        (With<TrainMarker>, Without<Crashed>),
    >,
    vehicles: Query<&VehicleStats>,
) {
    for (controller, mut velocity, children) in train.iter_mut() {
        let total_stats = children
            .iter()
            .filter_map(|&id| vehicles.get(id).ok())
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
fn tick_trains(
    time: Res<Time<Fixed>>,
    mut trains: Query<(&mut Trail, &Velocity), With<TrainMarker>>,
) {
    for (mut train, velocity) in trains.iter_mut() {
        train.path_progress += velocity.velocity * time.delta_seconds() / METER_PER_TRACK;
        // TODO: crash
        train.path_progress = train.path_progress.clamp(0., (train.path.len() - 1) as f32);
    }
}

/// System to update the transform of the train wagons.
/// Precondition: progress <= path.len() - 1
fn position_train_units(
    mut vehicles: Query<(&Parent, &mut Transform, &TrainIndex)>,
    trains: Query<&Trail, With<TrainMarker>>,
) {
    for (parent, mut transform, unit) in vehicles.iter_mut() {
        let Ok(trail) = trains.get(parent.get()) else {
            error!("Vehilce did not have a Train as a Parent");
            continue;
        };
        let Ok((start, end, interp)) = trail.point_on_trail(unit.position as f32 + 0.5) else {
            error!("Vehicle not on trail!");
            continue;
        };
        move_train_unit(transform.as_mut(), start, end, interp);
    }
}

fn update_tint(
    trains: Query<(&Children, Option<&Crashed>), With<TrainMarker>>,
    mut vehicles: Query<&mut Sprite>,
) {
    for (children, has_crashed) in &trains {
        let color = if has_crashed.is_none() {
            Color::WHITE
        } else {
            Color::GRAY
        };
        for &child in children.iter() {
            if let Ok(mut sprite) = vehicles.get_mut(child) {
                sprite.color = color;
            }
        }
    }
}

// ================================ FUNCTIONS ===================================

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

/// System to reverse a whole train
pub fn reverse_train(
    In(train_id): In<Entity>,
    mut trains: Query<(&mut Trail, &Children)>,
    mut vehicles: Query<&mut TrainIndex>,
) {
    let (mut trail, children) = ok_or_return!(trains.get_mut(train_id));
    trail.reverse();

    // Reverse the wagon indices
    for &e in children.iter() {
        if let Ok(mut idx) = vehicles.get_mut(e) {
            idx.position = trail.length - idx.position - 1;
        }
    }
}

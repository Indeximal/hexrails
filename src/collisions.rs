use bevy::prelude::*;
use bevy_rapier2d::prelude::CollisionEvent;

use crate::trains::{Crashed, TrainIndex, VehicleType, Velocity};

pub struct CollisionPlugin;

impl Plugin for CollisionPlugin {
    fn build(&self, app: &mut App) {
        // Fixme: figure out ordering... Maybe Post?
        app.add_systems(Update, crash_on_collision);
    }
}

/// This system reads rapier [`CollisionEvent`] and stops trains.
///
/// This should be idempotent as I don't want to trust rapier events too much.
///
/// Self collisions are a bit of a special case, since two following wagons intersect
/// on curves, therefore those collisions need to be ignored.
fn crash_on_collision(
    mut commands: Commands,
    mut collision_events: EventReader<CollisionEvent>,
    vehicles: Query<(&Parent, &TrainIndex), With<VehicleType>>,
    mut trains: Query<&mut Velocity>,
) {
    for collision_event in collision_events.read() {
        // Check whether two vehicles collided, and get the respective trains and stop both.

        let CollisionEvent::Started(a, b, _flags) = collision_event else {
            // Don't care about stopped collision events.
            continue;
        };
        let (Ok((t1, index1)), Ok((t2, index2))) = (vehicles.get(*a), vehicles.get(*b)) else {
            // Wasn't two vehicles
            continue;
        };
        let (t1, t2) = (t1.get(), t2.get());

        if t1 == t2 {
            if index1.position + 1 == index2.position || index2.position + 1 == index1.position {
                // Ignore self collision from two consequtive vehicles
                continue;
            }

            debug!("Train {t1:?} self collided");
            if let Some(mut cmd) = commands.get_entity(t1) {
                cmd.insert(Crashed);
            }
            if let Ok(mut velocity1) = trains.get_mut(t1) {
                velocity1.velocity = 0.0;
            }
        } else {
            debug!("Trains {t1:?} and {t2:?} crashed");

            // ignore errors, should be impossible anyway.
            if let Some(mut cmd) = commands.get_entity(t1) {
                cmd.insert(Crashed);
            }
            if let Ok(mut velocity) = trains.get_mut(t1) {
                velocity.velocity = 0.0;
            }
            if let Some(mut cmd) = commands.get_entity(t2) {
                cmd.insert(Crashed);
            }
            if let Ok(mut velocity) = trains.get_mut(t2) {
                velocity.velocity = 0.0;
            }
        }
    }
}

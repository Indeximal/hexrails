//! Module for manually driving a train, including selection, throttle, brake and reverse.

use crate::{
    input::{DriveAction, DriveInput, MenuState},
    interact::TrainClickEvent,
    railroad::{RailGraph, Track, TrackType},
    trains::*,
};

use petgraph::EdgeDirection;

use bevy::{ecs::system::RunSystemOnce, prelude::*};

pub struct ManualDrivingPlugin;

impl Plugin for ManualDrivingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                throttling_system,
                auto_extend_train_path,
                reverse_train_system,
                // after, so that acceleration gets cleared properly
                train_selection_system.after(throttling_system),
            )
                .run_if(in_state(MenuState::Driving)),
        );
    }
}

/// System to extend the path of trains if necessary. Useful mosty for manual driving.
fn auto_extend_train_path(
    mut trains: Query<(&mut Trail, Option<&PlayerControlledTrain>)>,
    input: Res<DriveInput>,
    graph_res: Res<RailGraph>,
) {
    let graph = &graph_res.graph;
    let steer_value = input.value(&DriveAction::SwitchDirection);
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
fn throttling_system(
    mut train: Query<&mut Controller, With<PlayerControlledTrain>>,
    input: Res<DriveInput>,
) {
    for mut controller in train.iter_mut() {
        controller.throttle = input.value(&DriveAction::Accelerate);
        controller.brake = input.value(&DriveAction::Brake);

        // allow for somewhat of a one pedal drive
        if !input.pressed(&DriveAction::Brake) && !input.pressed(&DriveAction::Accelerate) {
            controller.brake = 0.1;
        }
    }
}

/// System to reverse a whole train on key press
fn reverse_train_system(
    mut commands: Commands,
    trains: Query<(Entity, &Velocity), With<PlayerControlledTrain>>,
    input: Res<DriveInput>,
) {
    if !input.just_pressed(&DriveAction::Reverse) {
        return;
    }
    // FIXME: Allow the player to steer with controller.trim_front();
    for (id, velocity) in trains.iter() {
        if velocity.velocity != 0. {
            warn!("Cannot reverse moving train!");
            continue;
        }

        commands.add(move |world: &mut World| world.run_system_once_with(id, reverse_train));
    }
}

/// System to enter and exit trains on click
fn train_selection_system(
    mut commands: Commands,
    mut click_event: EventReader<TrainClickEvent>,
    mut controlled_train: Query<(Entity, &mut Controller), With<PlayerControlledTrain>>,
) {
    // Skip all old events, why not. (please let this not lead to a bug xD)
    let Some(ev) = click_event.read().last() else {
        return;
    };

    // Remove player control from previously active train and release all control.
    if let Ok((entity, mut control)) = controlled_train.get_single_mut() {
        control.throttle = 0.0;
        control.brake = 0.0;
        commands.entity(entity).remove::<PlayerControlledTrain>();
    }

    commands.entity(ev.train).insert(PlayerControlledTrain);

    debug!("Selected train {:?}", ev.train);
}

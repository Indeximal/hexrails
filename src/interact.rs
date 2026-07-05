//! This module is concerned with mouse interaction with objects in the world.
//!
//! This currently consists of two events being emitted:
//! - [`NodeClickEvent`] whenever an [`InteractionNode`] was clicked on.
//! - [`TileClickEvent`] whenever the background world was clicked.
//!
//! A tile event will only be emitted if no node was interacted with and
//! a node event will be emitted for every node which is active, but only
//! one will have the `primary` flag set.
//!
//! Everything is updated in `PreUpdate`, if you need to run anything dependent,
//! use `.after()` with [`InteractSet`].
//!
//! [`TrainClickEvent`] are generated in addition to the other two events.
//!

use bevy::color::palettes;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::camera::WorldViewCam;
use crate::tilemap::{Direction, Joint, Tile};
use crate::trains::{BumperNode, Trail, TrainIndex, VehicleOf, Vehicles};

pub struct InteractPlugin;
impl Plugin for InteractPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<TileClickEvent>()
            .add_observer(emit_train_events)
            // Ordering the event to be after the input, but still in PreUpdate,
            // so in Update the events are available
            .add_systems(
                PreUpdate,
                (
                    update_interaction_status.before(emit_events),
                    emit_events.after(leafwing_input_manager::plugin::InputManagerSystem::Update),
                )
                    .in_set(InteractSet),
            )
            .add_systems(Update, draw_interaction_nodes);
    }
}

/// The systems in PreUpdate emitting the events.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct InteractSet;

/// A circle collider in the world that can be interacted with.
///
/// Also needs an [`InteractionStatus`] component.
#[derive(Component)]
pub struct InteractionNode {
    pub radius: f32,
}

/// The status of an [`InteractionNode`], i.e. whether the cursor is
/// away from this node, above it (Hit) or the nearest hit.
///
/// Updated by [`update_interaction_status`].
///
/// Potentially werid behavior just after a click, since then the cursor position
/// is sometimes None.
#[derive(Component, PartialEq, Eq, Default)]
pub enum InteractionStatus {
    #[default]
    None,
    Hit,
    NearestHit,
}

/// This message will be written by [`InteractPlugin`] whenever a tile was clicked on.
///
/// This is a plain broadcast [`Message`], since it isn't tied to any particular entity
/// and is read by several independent systems each frame.
#[derive(Message, Debug)]
pub struct TileClickEvent {
    pub coord: Tile,
    pub side: Option<Direction>,
    pub button: MouseButton,
}

/// This event will be triggered by [`InteractPlugin`] whenever an [`InteractionNode`] was clicked on.
///
/// This is a plain observable [`Event`] (not a buffered [`Message`]) since it always concerns a
/// specific node entity and consumers are independent, decoupled reactions rather than a shared
/// per-frame batch.
#[derive(Event, Debug)]
pub struct NodeClickEvent {
    /// The entity with the [`InteractionNode`] component.
    pub node: Entity,
    /// The action that was taken to trigger this event.
    // pub action: Action,
    /// Was this the nearest node hitting the click?
    ///
    /// If this is false, this was just another node that was also below the cursor,
    /// but deemed secondary.
    pub primary: bool,
}

/// Generated whenever a bumper of a train was clicked.
///
/// This is generated in addition to the [`NodeClickEvent`] and only for the primary one.
#[derive(Event)]
pub struct TrainClickEvent {
    /// The id of the [`TrainBundle`] entity.
    pub train: Entity,

    /// The "fence-post" index of where the train was clicked.
    ///
    /// I.e. between wagons with index `bumper_index` and `bumper_index+1`.
    pub bumper_index: u16,

    /// The bumper entity that was clicked.
    pub bumper_entity: Entity,
}

/// System for generating the interaction events..
///
/// FIXME
/// Currently uses `bevy_input` directly, see
/// <https://github.com/Leafwing-Studios/leafwing-input-manager/issues/527>
fn emit_events(
    mut commands: Commands,
    mouse_input: Res<ButtonInput<MouseButton>>,
    nodes: Query<(Entity, &InteractionStatus)>,
    mut tile_event_writer: MessageWriter<TileClickEvent>,
    mut world_interaction: WorldInteractionQuery,
) {
    // Only proceed if the build button has been pressed
    if !mouse_input.just_pressed(MouseButton::Left) {
        return;
    }

    let mut any_node_hit = false;
    for (id, status) in &nodes {
        let is_primary = match status {
            InteractionStatus::Hit => false,
            InteractionStatus::NearestHit => true,
            _ => continue,
        };
        any_node_hit = true;

        commands.trigger(NodeClickEvent {
            node: id,
            // FIXME: should depend on input of course...
            // action: Action::Build,
            primary: is_primary,
        });
    }

    if any_node_hit {
        // No tile event should be emitted if there was a node clicked.
        return;
    }

    let Some(world_pos) = world_interaction.get_cursor_world_pos() else {
        // Ignore out of bounds etc.
        return;
    };

    let event = match Joint::from_world_pos(world_pos) {
        Ok(Joint { tile, side }) => TileClickEvent {
            coord: tile,
            side: Some(side),
            button: MouseButton::Left,
        },
        Err(tile) => TileClickEvent {
            coord: tile,
            side: None,
            button: MouseButton::Left,
        },
    };
    tile_event_writer.write(event);
}

fn update_interaction_status(
    mut nodes: Query<(
        Entity,
        &GlobalTransform,
        &InteractionNode,
        &mut InteractionStatus,
    )>,
    mut world_interaction: WorldInteractionQuery,
) {
    let cursor_pos = world_interaction.get_cursor_world_pos();

    let mut nearest = None;
    for (id, position, node, mut status) in nodes.iter_mut() {
        let position = position.translation();

        // signed square distance, weird metric, but negative if inside, positive if outside.
        let signed_distance =
            cursor_pos.map(|p| p.distance_squared(position.xy()) - node.radius.powi(2));

        let Some(signed_distance) = signed_distance else {
            status.set_if_neq(InteractionStatus::None);
            continue;
        };

        if signed_distance < 0.0 {
            // inside.
            // Can still double trigger if was and is NearestHit.
            status.set_if_neq(InteractionStatus::Hit);

            match nearest {
                None => nearest = Some((id, signed_distance)),
                Some((_, least_dist)) if signed_distance < least_dist => {
                    nearest = Some((id, signed_distance))
                }
                _ => (),
            };
        } else {
            // outside
            status.set_if_neq(InteractionStatus::None);
        }
    }

    if let Some((id, _)) = nearest {
        let (_, _, _, mut status) = nodes
            .get_mut(id)
            .expect("id was assigned by the same query");

        *status = InteractionStatus::NearestHit;
    }
}

fn emit_train_events(
    trigger: On<NodeClickEvent>,
    mut commands: Commands,
    bumpers: Query<(&ChildOf, &BumperNode)>,
    vehicles: Query<(&TrainIndex, &VehicleOf)>,
    trains: Query<(Entity, &Trail, &Vehicles)>,
) {
    let ev = trigger.event();
    if !ev.primary {
        // only trigger from one side, action should be symmetric.
        return;
    }
    let Ok((bump_parent, bump_dir)) = bumpers.get(ev.node) else {
        return;
    };
    let Ok((index, vehicle_of)) = vehicles.get(bump_parent.parent()) else {
        error!("BumperNode should always be attached to a vehicle!");
        return;
    };
    let Ok((train, _trail, _train_vehicles)) = trains.get(vehicle_of.train()) else {
        error!("Vehicles should always be attached to a train!");
        return;
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
                bump_parent.parent()
            );
        }
        if _train_vehicles.len() != _trail.length as usize {
            error!("Train {train:?}'s length in trail does not match number of vehicles!");
        }
    }

    trace!("Train {train:?} clicked at {bumper_index}");
    commands.trigger(TrainClickEvent {
        train,
        bumper_index,
        bumper_entity: ev.node,
    });
}

fn draw_interaction_nodes(
    nodes: Query<(&GlobalTransform, &InteractionNode, &InteractionStatus)>,
    mut gizmos: Gizmos,
) {
    for (position, node, status) in &nodes {
        let position = position.translation();

        let color = match status {
            InteractionStatus::None => palettes::basic::GRAY,
            InteractionStatus::Hit => palettes::basic::YELLOW,
            InteractionStatus::NearestHit => palettes::basic::RED,
        };

        gizmos.circle_2d(Isometry2d::from_translation(position.xy()), node.radius, color);
    }
}

/// A query for the purpose of calling [`Self::get_cursor_world_pos`].
///
/// TODO:
/// There might be an advantage to converting this to an leafwing dual axis action.
/// With that the cursor position might get persisted more often...
#[derive(SystemParam)]
struct WorldInteractionQuery<'w, 's> {
    /// Windows to get the cursor position
    windows: Query<'w, 's, &'static Window>,
    /// Camera to convert it to a world position. Must be exactly 1 such camera.
    cam: Query<'w, 's, (&'static GlobalTransform, &'static Camera), With<WorldViewCam>>,
    /// Query to ignore interactions over bevy_ui buttons
    other_buttons: Query<'w, 's, &'static Interaction>,
}

impl<'w, 's> WorldInteractionQuery<'w, 's> {
    /// Returns `None` if the cursor is outside the viewport, the viewport cannot be computed,
    /// the viewport cannot be mapped to the world or the cursor is above a UI.
    fn get_cursor_world_pos(&mut self) -> Option<Vec2> {
        let on_bevy_ui = self
            .other_buttons
            .iter()
            .any(|&interact| interact == Interaction::Pressed || interact == Interaction::Hovered);
        if on_bevy_ui {
            return None;
        }

        let (pos, cam) = self.cam.single().ok()?;
        let Ok(window) = self.windows.single() else {
            warn!("Cannot deal with multiple windows!");
            return None;
        };

        cam.viewport_to_world_2d(pos, window.cursor_position()?).ok()
    }
}

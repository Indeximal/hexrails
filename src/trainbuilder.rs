use bevy::{ecs::system::RunSystemOnce, prelude::*};
use bevy_rapier2d::{
    plugin::RapierContext,
    prelude::{ActiveCollisionTypes, ActiveEvents, Collider, CollisionGroups, Group, Sensor},
};
use petgraph::EdgeDirection;

use crate::{
    interact::{InteractionNode, InteractionStatus, TileClickEvent, TrainClickEvent},
    ok_or_return,
    railroad::RailGraph,
    sprites::{SpriteAtlases, VehicleSprite},
    tilemap::*,
    trains::*,
    ui::InteractingState,
};

pub struct TrainBuildingPlugin;

impl Plugin for TrainBuildingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                train_builder,
                append_vehicle_system,
                uncoupling_system,
                coupling_system,
            ),
        );
    }
}

const VEHICLE_GROUP: Group = Group::GROUP_1;
const BUMPER_GROUP: Group = Group::GROUP_2;

impl Trail {
    /// Returns the index of the wagon currently nearest to `face` or `length` if at the end.
    /// Returns none if the face is not on the path or not near any wagon.
    ///
    /// Currently unused, but maybe useful in the future...
    #[allow(dead_code)]
    fn index_for_tile(&self, face: Joint) -> Option<u16> {
        if self.path_progress - self.length as f32 <= 1. {
            // no space for a new wagon
            return None;
        }
        let path_index = self.path.iter().position(|&f| f == face)? as f32;
        let index = self.path_progress - path_index;
        if index < 0. {
            None
        } else if index >= self.length as f32 + 1. {
            None
        } else {
            Some(index.floor() as u16)
        }
    }

    /// Return the distance between the end of the active segment of `self`
    /// to the beginning of `other`, but only if its magnitude is less than 1.0.
    fn gap_to(&self, other: &Trail) -> Option<f32> {
        let (head1, head2, head_fract) = other.point_on_trail(0.0).ok()?;
        let (tail1, tail2, tail_fract) = self.point_on_trail(self.length as f32).ok()?;
        // Cases:
        if head1 == tail1 && head2 == tail2 {
            // In same track
            Some(tail_fract - head_fract)
        } else if head2 == tail1 {
            // In following tracks
            Some((1.0 - head_fract) + tail_fract)
        } else if head1 == tail2 {
            // Weird case, when gap is negative, following tracks but other order
            Some(-(head_fract + (1.0 - tail_fract)))
        } else {
            None
        }
    }

    /// Appends `back` to `front`.
    ///
    /// If they do not overlap, this returns `Err`.
    /// If something goes wrong, e.g. if they overlap too much or invariants are broken,
    /// this also returns and error and logs more info.
    fn clone_from_parts(front: &Trail, back: &Trail) -> Result<Self, ()> {
        // init vec with capacity, push back with trimmed front, push front with trimmed back.
        let front_part = front.trim_back();
        let back_part = back.trim_front();
        let mut path = Vec::from(back_part);
        // Combine, but skip overlap of 1, 2 or 3 joints.
        let Some(overlap_index) = front_part.iter().position(|j| Some(j) == path.last()) else {
            return Err(());
        };
        path.extend(front_part.iter().skip(overlap_index + 1));

        // compute progress (by search, which isn't optimal, but okay)
        let Ok((f_start, _, progress_frac)) = front.point_on_trail(0.0) else {
            error!("clone_from_parts couldn't get front!");
            return Err(());
        };
        let Some(progress_int) = path.iter().position(|j| *j == f_start) else {
            error!("clone_from_parts couldn't find joining joint!");
            return Err(());
        };

        let combined = Self {
            path,
            path_progress: progress_int as f32 + progress_frac,
            length: front.length + back.length,
        };
        if !combined.check_invariant() {
            error!("clone_from_parts constructed an invalid trail!");
            return Err(());
        }
        Ok(combined)
    }
}

/// This system tries to place a new train on click
fn train_builder(
    mut commands: Commands,
    atlas: Res<SpriteAtlases>,
    mut click_event: EventReader<TileClickEvent>,
    rail_graph: Res<RailGraph>,
    state: Res<State<InteractingState>>,
) {
    let graph = rail_graph.as_ref();
    let InteractingState::PlaceTrains(wagon_type) = state.get() else {
        // Events are irrelevant
        click_event.clear();
        return;
    };

    for ev in click_event.read() {
        let Some(side) = ev.side else {
            // for now ignore clicks in the center: might be ambigous
            continue;
        };
        let face = Joint {
            tile: ev.coord,
            side,
        };
        let neighbor_count = graph
            .graph
            .neighbors_directed(face, EdgeDirection::Outgoing)
            .count();
        if neighbor_count == 0 {
            // skip if there is no rail in the graph at this position
            continue;
        }

        create_new_train(&mut commands, &atlas, face, &graph, *wagon_type);
    }
}

/// Temporary dev testing system to add vehicles to the end of trains.
///
/// Removed support for inserting at beginning or middle since it isn't planned feature.
///
/// FIXME: this is slightly fucked if you have a train without power, as you cannot extend
/// the path without driving right now.
fn append_vehicle_system(
    state: Res<State<InteractingState>>,
    mut trigger: EventReader<TrainClickEvent>,
    trains: Query<&Trail>,
    mut commands: Commands,
    atlas: Res<SpriteAtlases>,
) {
    let InteractingState::PlaceTrains(wagon_type) = state.get() else {
        // Events are irrelevant
        trigger.clear();
        return;
    };

    for ev in trigger.read() {
        let train_id = ev.train;
        let Ok(trail) = trains.get(train_id) else {
            warn!("Mismatch train query");
            continue;
        };
        if ev.bumper_index != trail.length {
            // Can only append at the end
            continue;
        }
        if trail.length as f32 + 1. > trail.path_progress {
            warn!("Cannot append vehicle to train {train_id:?} since the trail is too short");
            continue;
        }

        debug!("Appending a vehicle to train {train_id:?}");

        let new_wagon = spawn_wagon(
            &mut commands,
            &atlas,
            *wagon_type,
            VehicleStats::default_for_type(*wagon_type),
            ev.bumper_index,
        );
        commands.entity(train_id).add_child(new_wagon);
        commands.add(move |world: &mut World| {
            let mut train_q = world.query::<&mut Trail>();
            let Ok(mut trail) = train_q.get_mut(world, train_id) else {
                warn!("appending to probably despawned train ?!");
                return;
            };
            trail.length += 1;
            if !trail.check_invariant() {
                // This could be due to a bad interleaving...
                error!("adding vehicle broke invariant even though it was checked!");
            }
        });
    }
}

fn coupling_system(
    mut commands: Commands,
    mut trigger: EventReader<TrainClickEvent>,
    rapier_context: Res<RapierContext>,
    trains: Query<&Trail, With<TrainMarker>>,
    vehicles: Query<&Parent, With<VehicleType>>,
    bumpers: Query<(&Parent, &BumperNode)>,
) {
    fn couple_trains(
        In((t1, d1, t2, d2)): In<(Entity, BumperNode, Entity, BumperNode)>,
        mut commands: Commands,
        trains: Query<&Trail, With<TrainMarker>>,
        child_query: Query<&Children>,
    ) {
        let trail1 = ok_or_return!(trains.get(t1), "not a train");
        let trail2 = ok_or_return!(trains.get(t2), "not a train");

        // Cases:
        // All cases are combined in 3 steps: figure out which one is the front train,
        // the one which will survive and has the back appened to it.
        // Then reverse one of the trains, given by `reverse`, and add the value from
        // front length to all indices of the back train.
        let ((front_id, front), (back_id, back), reverse) = match (d1, d2) {
            // Front / back -> reindex back
            (BumperNode::Front, BumperNode::Back) => ((t2, trail2), (t1, trail1), None),
            (BumperNode::Back, BumperNode::Front) => ((t1, trail1), (t2, trail2), None),
            // Front / front -> reverse one, reindex other
            (BumperNode::Front, BumperNode::Front) => ((t1, trail1), (t2, trail2), Some(t1)),
            // Back / back -> reverse & reindex one
            (BumperNode::Back, BumperNode::Back) => ((t1, trail1), (t2, trail2), Some(t2)),
        };
        let front_len = front.length;

        // Construct a new trail anyway, with maybe too much cloning
        // TODO: Check the gap here aswell
        // let gap = some_or_return!(trail2.gap_to(trail1));
        let Ok(new_trail) = (match reverse {
            None => Trail::clone_from_parts(front, back),
            Some(reverse) if reverse == front_id => {
                let mut front = front.clone();
                front.reverse();
                Trail::clone_from_parts(&front, back)
            }
            Some(reverse) if reverse == back_id => {
                let mut back = back.clone();
                back.reverse();
                Trail::clone_from_parts(front, &back)
            }
            Some(_) => unreachable!("Was set to one of these values above"),
        }) else {
            warn!("Trails do not align");
            return;
        };

        debug!("Coupling train {back_id:?} to {front_id:?}");

        // Queue all the commands. I rely on them being executed in order.
        if let Some(reverse) = reverse {
            // Reverse train indices of one of the trains
            commands
                .add(move |world: &mut World| world.run_system_once_with(reverse, reverse_train));
        }
        commands.add(move |world: &mut World| {
            world.run_system_once_with((back_id, front_len as i16), reindex_train)
        });
        commands
            .entity(front_id)
            // Overrite old trail component
            .insert(new_trail)
            // And append all newly gained vehicles
            .push_children(ok_or_return!(
                child_query.get(back_id),
                "back has no children?"
            ));
        // No recursive needed, children have just been moved
        commands.entity(back_id).despawn();
    }

    for ev in trigger.read() {
        let Ok(trail) = trains.get(ev.train) else {
            error!("Train not found");
            continue;
        };

        if !(ev.bumper_index == 0 || ev.bumper_index == trail.length) {
            // cannot couple non-end vehicle
            continue;
        }

        // this should ideally be exactly one pair
        let mut pairs_iter = rapier_context
            .intersection_pairs_with(ev.bumper_entity)
            .filter(|(b1, b2, hit)| *hit && b1 != b2);
        let Some((b1, b2, _)) = pairs_iter.next() else {
            // nothing to couple to
            continue;
        };
        if let Some((b3, b4, _)) = pairs_iter.next() {
            warn!("Conflicting coupling: {b1:?}-{b2:?} vs {b3:?}-{b4:?}");
            continue;
        }

        let (Ok((v1, &d1)), Ok((v2, &d2))) = (bumpers.get(b1), bumpers.get(b2)) else {
            error!("BumperNode should always be attached to a vehicle!");
            continue;
        };
        let (Ok(t1), Ok(t2)) = (vehicles.get(v1.get()), vehicles.get(v2.get())) else {
            error!("Vehicle should always be attached to a train!");
            continue;
        };
        let (t1, t2) = (t1.get(), t2.get());
        if t1 == t2 {
            warn!("Trying to self-couple: {t1:?}");
            continue;
        }

        commands.add(move |world: &mut World| {
            // This actually leads to a two deep `run_system_once_with` invocation,
            // I hope this is fine.
            // TODO: for better performance, use a registered system.
            world.run_system_once_with((t1, d1, t2, d2), couple_trains);
        })
    }
}

fn uncoupling_system(
    mut commands: Commands,
    mut trigger: EventReader<TrainClickEvent>,
    vehicles: Query<(Entity, &TrainIndex)>,
    trains: Query<(Entity, &Trail, &Children)>,
) {
    for ev in trigger.read() {
        let Ok((train, trail, train_children)) = trains.get(ev.train) else {
            warn!("Train click event for not-query-matching entity (despawned or malformed)");
            continue;
        };

        let front_length = ev.bumper_index;
        let back_length = trail.length - ev.bumper_index;

        if front_length == 0 || back_length == 0 {
            // Already at an end of the train
            continue;
        }

        debug!("Uncoupling {back_length} vehicles from the train");

        // Changes that should happen simultaneoulsy
        // (otherwise breaks invariants and leads to weird bugs):
        // - Spawn new trainbundle, clone and adjust trail
        // - reparent uncoupled vehicles (automatically removed when inserting)
        // - update trainbundle: changed trail length
        // - change TrainIndex on uncoupled vehicles

        // This is unsorted tho
        let to_reparent = train_children
            .iter()
            .filter_map(|&e| vehicles.get(e).ok())
            .filter(|(_, idx)| idx.position >= ev.bumper_index)
            .map(|(e, _)| e)
            .collect::<Vec<_>>();

        let mut back_trail = Trail {
            // This shouldn't break any trail invariants...
            path: trail.path.clone(),
            path_progress: trail.path_progress - front_length as f32,
            length: back_length,
        };
        back_trail.remove_lead();

        let new_train_id = commands
            .spawn(TrainBundle::new(
                back_trail, 55.0, // approx 200kmh
            ))
            .push_children(&to_reparent)
            .id();
        commands.add(move |world: &mut World| {
            // The front trail has to be shortened by back_length,
            // while the vehicles in to_reindex have to have front_length subtracted.
            world.run_system_once_with((new_train_id, -(front_length as i16)), reindex_train);
            world.run_system_once_with((train, front_length), set_train_length);
        })
    }
}

/// A mutating part of the `uncoupling_system`, since I want to apply them
/// at a controlled time.
fn set_train_length(
    In((train_id, new_len)): In<(Entity, u16)>,
    mut train: Query<&mut Trail, With<TrainMarker>>,
) {
    let Ok(mut t) = train.get_mut(train_id) else {
        error!("set_train_length called with non train entity");
        return;
    };
    t.length = new_len;
}

/// Helper system to add an offset to all vehicle indices of a train.
///
/// WARNING: this will temporarlily break the train invariant,
/// this is only a part of the coupling/uncoupling process
fn reindex_train(
    In((train_id, diff)): In<(Entity, i16)>,
    train: Query<&Children, With<TrainMarker>>,
    mut vehicles: Query<&mut TrainIndex, With<VehicleType>>,
) {
    let Ok(children) = train.get(train_id) else {
        error!("reindex_train called with non-train entity");
        return;
    };

    // Somehow doesn't allow a for loop
    let mut iter = vehicles.iter_many_mut(children.iter());
    while let Some(mut idx) = iter.fetch_next() {
        idx.position = idx.position.saturating_add_signed(diff);
    }
}

/// Creates a new train with a single wagon.
/// Precondition: `face` is in the `rail_graph` and has a neighbor.
fn create_new_train(
    commands: &mut Commands,
    atlas: &SpriteAtlases,
    face: Joint,
    rail_graph: &RailGraph,
    wagon_type: VehicleType,
) {
    info!("Creating train at @{:?}", face.tile);

    let next_face = rail_graph
        .graph
        .neighbors_directed(face, EdgeDirection::Outgoing)
        .next()
        .expect("Broke precondition: `face is in the graph and has a neighbor`!");

    let first_wagon = spawn_wagon(
        commands,
        atlas,
        wagon_type,
        VehicleStats::default_for_type(wagon_type),
        0,
    );

    commands
        .spawn(TrainBundle::new(
            Trail {
                path: vec![face, next_face],
                path_progress: 1.0,
                length: 1,
            },
            55.0, // approx 200kmh
        ))
        .add_child(first_wagon);
}

/// Helper to spawn a wagon sprite
pub fn spawn_wagon(
    commands: &mut Commands,
    atlas: &SpriteAtlases,
    wagon_type: VehicleType,
    wagon_stats: VehicleStats,
    insert_index: u16,
) -> Entity {
    let sprite = atlas.vehicle_sprite(match wagon_type {
        VehicleType::Locomotive => VehicleSprite::PurpleBullet,
        VehicleType::Wagon => VehicleSprite::GreyBox,
    });

    fn spawn_bumper(commands: &mut Commands, translation: f32, uncouple_dir: BumperNode) -> Entity {
        commands
            .spawn(TransformBundle::from_transform(
                Transform::from_translation(Vec3::X * translation),
            ))
            .insert(InteractionNode {
                radius: TILE_WIDTH / 4.,
            })
            .insert(InteractionStatus::default())
            .insert(uncouple_dir)
            .insert(Collider::ball(TILE_WIDTH / 16.))
            .insert(Sensor)
            .insert(ActiveCollisionTypes::STATIC_STATIC)
            .insert(CollisionGroups::new(BUMPER_GROUP, BUMPER_GROUP))
            .id()
    }
    let front_bumper = spawn_bumper(commands, -TILE_WIDTH / 2., BumperNode::Front);
    let back_bumper = spawn_bumper(commands, TILE_WIDTH / 2., BumperNode::Back);

    commands
        .spawn(VehicleBundle {
            index: TrainIndex {
                position: insert_index,
            },
            tyype: wagon_type,
            stats: wagon_stats,
            name: Name::new("Wagon"),
            visuals: sprite,
        })
        .insert(Collider::cuboid(
            0.9 * TILE_WIDTH / 2.,
            0.5 * TILE_WIDTH / 2.,
        ))
        .insert(Sensor)
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(ActiveCollisionTypes::STATIC_STATIC)
        .insert(CollisionGroups::new(VEHICLE_GROUP, VEHICLE_GROUP))
        .add_child(front_bumper)
        .add_child(back_bumper)
        .id()
}

use bevy::{ecs::system::SystemId, prelude::*};
use bevy_rapier2d::prelude::{
    ActiveCollisionTypes, ActiveEvents, Collider, CollisionEvent, CollisionGroups, Group, Sensor,
};
use petgraph::EdgeDirection;

use crate::{
    interact::{InteractionNode, InteractionStatus, TileClickEvent, TrainClickEvent},
    railroad::RailGraph,
    sprites::{SpriteAtlases, VehicleSprite},
    tilemap::*,
    trains::*,
    ui::InteractingState,
};

pub struct TrainBuildingPlugin;

impl Plugin for TrainBuildingPlugin {
    fn build(&self, app: &mut App) {
        let label = app.world.register_system(reindex_system_command);
        app.add_systems(
            Update,
            (
                train_builder,
                uncoupling_system,
                append_vehicle_system,
                apply_coupleable_component,
            ),
        )
        .insert_resource(ReindexSystemLabel(label));
    }
}

#[derive(Resource)]
struct ReindexSystemLabel(SystemId<(Vec<Entity>, Entity, u16, u16)>);

/// Attached to a train whenever it bumps into another one, given by the inner id.
#[derive(Component)]
pub struct CoupleableTo(Entity);

const VEHICLE_GROUP: Group = Group::GROUP_1;
const BUMPER_GROUP: Group = Group::GROUP_2;

impl Trail {
    /// Returns the index of the wagon currently nearest to `face` or `length` if at the end.
    /// Returns none if the face is not on the path or not near any wagon.
    ///
    /// Currently unused, but maybe useful in the future...
    #[allow(dead_code)]
    pub fn index_for_tile(&self, face: Joint) -> Option<u16> {
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

// Fixme: reversing can fuck this up I think
fn apply_coupleable_component(
    mut commands: Commands,
    mut evs: EventReader<CollisionEvent>,
    bumpers: Query<&Parent, With<BumperNode>>,
    vehicles: Query<&Parent, With<VehicleType>>,
) {
    for collision_event in evs.read() {
        let (a, b, started) = match collision_event {
            CollisionEvent::Started(a, b, _) => (a, b, true),
            CollisionEvent::Stopped(a, b, _) => (a, b, false),
        };
        let (Ok(v1), Ok(v2)) = (bumpers.get(*a), bumpers.get(*b)) else {
            // Wasn't two bumpers
            continue;
        };
        let (Ok(t1), Ok(t2)) = (vehicles.get(v1.get()), vehicles.get(v2.get())) else {
            error!("BumperNode should always be attached to a vehicle!");
            continue;
        };
        let (t1, t2) = (t1.get(), t2.get());

        if t1 == t2 {
            continue;
        }

        if started {
            debug!("Trains {t1:?} and {t2:?} are now coupleable");
        } else {
            debug!("Trains {t1:?} and {t2:?} are not coupleable anymore");
        }

        if let Some(mut cmd) = commands.get_entity(t1) {
            if started {
                cmd.insert(CoupleableTo(t2));
            } else {
                cmd.remove::<CoupleableTo>();
            }
        }
        if let Some(mut cmd) = commands.get_entity(t2) {
            if started {
                cmd.insert(CoupleableTo(t1));
            } else {
                cmd.remove::<CoupleableTo>();
            }
        }
    }
}

fn coupling_system(
    mut trigger: EventReader<TrainClickEvent>,
    trains: Query<(&CoupleableTo, &Trail), With<TrainMarker>>,
) {
    for ev in trigger.read() {
        let Ok((coupleable, trail)) = trains.get(ev.train) else {
            // is not couplable probably
            continue;
        };
        // TODO: was the correct side pressed?
    }
}

fn uncoupling_system(
    mut commands: Commands,
    reindex_command: Res<ReindexSystemLabel>,
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
        back_trail.trim_front();

        let reindex_command = reindex_command.0.clone();
        commands
            .spawn(TrainBundle::new(
                back_trail, 55.0, // approx 200kmh
            ))
            .push_children(&to_reparent);
        commands.add(move |world: &mut World| {
            if let Err(e) = world.run_system_with_input(
                reindex_command,
                (to_reparent, train, front_length, back_length),
            ) {
                error!("couldn't run reindex_system_command: {e:?}");
            }
        })
    }
}

/// The mutating part of the `uncoupling_system`, since I want to apply them
/// at a controlled time.
///
/// The front trail has to be shortened by back_length,
/// while the vehicles in to_reindex have to have front_length subtracted.
fn reindex_system_command(
    In((to_reindex, to_shorten, front_len, back_len)): In<(Vec<Entity>, Entity, u16, u16)>,
    mut train: Query<&mut Trail>,
    mut vehicles: Query<&mut TrainIndex>,
) {
    let Ok(mut t) = train.get_mut(to_shorten) else {
        return;
    };
    t.length -= back_len;

    // Somehow doesn't allow a for loop
    let mut iter = vehicles.iter_many_mut(&to_reindex);
    while let Some(mut idx) = iter.fetch_next() {
        idx.position -= front_len;
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
            .insert(ActiveEvents::COLLISION_EVENTS)
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

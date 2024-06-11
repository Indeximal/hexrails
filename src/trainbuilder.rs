use bevy::{ecs::system::SystemId, prelude::*};
use petgraph::EdgeDirection;

use crate::{
    interact::{InteractionNode, InteractionStatus, TileClickEvent},
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
        app.add_systems(Update, (train_builder, uncoupling_system))
            .insert_resource(ReindexSystemLabel(label));
    }
}

#[derive(Resource)]
struct ReindexSystemLabel(SystemId<(Vec<Entity>, Entity, u16, u16)>);

impl Trail {
    /// Returns the index of the wagon currently nearest to `face` or `length` if at the end.
    /// Returns none if the face is not on the path or not near any wagon.
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

/// This system tries to place a train wagon or a new train on click
fn train_builder(
    mut commands: Commands,
    atlas: Res<SpriteAtlases>,
    mut click_event: EventReader<TileClickEvent>,
    rail_graph: Res<RailGraph>,
    state: Res<State<InteractingState>>,
    mut trains: Query<(Entity, &Children, &mut Trail)>,
    wagons: Query<&mut TrainIndex>,
) {
    let graph = rail_graph.as_ref();
    let wagon_type = match state.get() {
        InteractingState::PlaceTrains(v) => v.clone(),
        _ => {
            // Events are irrelevant
            click_event.clear();
            return;
        }
    };
    for ev in click_event.read() {
        if ev.side.is_none() {
            // for now ignore clicks in the center: might be ambigous
            continue;
        }
        // todo: also consider the opposite tile face
        let face = Joint {
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
                    VehicleStats::default_for_type(wagon_type),
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
            error!("Vehicles should always be attached to a train!");
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

        let reindex_command = reindex_command.0.clone();
        commands
            .spawn(TrainBundle::new(
                Trail {
                    // This shouldn't break any trail invariants...
                    path: trail.path.clone(),
                    path_progress: trail.path_progress - front_length as f32,
                    length: back_length,
                },
                55.0, // approx 200kmh
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

/// Helper to insert a wagon into an existing train and move all other wagons accordingly
fn insert_wagon(
    commands: &mut Commands,
    atlas: &SpriteAtlases,
    sibling_ids: &Children,
    mut sibling_query: Query<&mut TrainIndex>,
    parent: Entity,
    wagon_type: VehicleType,
    wagon_stats: VehicleStats,
    insert_index: u16,
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
        .add_child(front_bumper)
        .add_child(back_bumper)
        .id()
}

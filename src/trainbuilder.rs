use bevy::prelude::*;
use petgraph::EdgeDirection;

use crate::{
    railroad::RailGraph,
    sprites::{SpriteAtlases, VehicleSprite},
    tilemap::*,
    trains::*,
    ui::InteractingState,
};

pub struct TrainBuildingPlugin;

impl Plugin for TrainBuildingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, train_builder);
    }
}

impl Train {
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
    mut trains: Query<(Entity, &Children, &mut Train)>,
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
            Train {
                path: vec![face, next_face],
                path_progress: 1.0,
                length: 1,
            },
            55.0, // approx 200kmh
        ))
        .insert(VisibilityBundle::default())
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

// Helper to spawn a wagon sprite
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

    commands
        .spawn(sprite)
        .insert(TrainIndex {
            position: insert_index,
        })
        .insert(wagon_type)
        .insert(wagon_stats)
        .insert(Name::new("Wagon"))
        .id()
}

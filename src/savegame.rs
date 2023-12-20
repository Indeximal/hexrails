use std::{error::Error, fs};

use bevy::{ecs::system::CommandQueue, prelude::*};
use petgraph::graphmap::DiGraphMap;
use serde::{Deserialize, Serialize};

use crate::assets::SpriteAtlases;
use crate::railroad::{spawn_rail, NetworkRoot, RailGraph, Track};
use crate::trainbuilder::*;
use crate::trains::*;

const SAVEGAME_PATH: &str = "savegame/stupid.json";
const CURRENT_SAVEGAME_VERSION: u32 = 6;

pub struct LoadSavePlugin;

impl Plugin for LoadSavePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, initial_load_system)
            .add_systems(Update, save_system);
    }
}

/// This is a struct holding all the data that will get saved.
/// In order to avoid many clones this struct houses references, in order
/// to load the data, the equivalent `LoadedGame` struct is used.
///
/// *Make sure to keep these two structs the same!*
#[derive(Serialize)]
struct SavedGame<'a> {
    version: u32,
    network: &'a RailGraph,
    trains: Vec<SavedTrain<'a>>,
}

#[derive(Deserialize)]
struct LoadedGame {
    version: u32,
    network: RailGraph,
    trains: Vec<LoadedTrain>,
}

impl Default for LoadedGame {
    fn default() -> Self {
        Self {
            version: CURRENT_SAVEGAME_VERSION,
            network: RailGraph {
                graph: DiGraphMap::new(),
            },
            trains: Vec::new(),
        }
    }
}

#[derive(Serialize)]
/// contains the path and position of the whole train and all its wagons
struct SavedTrain<'a> {
    controller: &'a TrainHead,
    velocity: &'a Velocity,
    wagons: Vec<SavedWagon<'a>>,
}

#[derive(Deserialize)]
struct LoadedTrain {
    controller: TrainHead,
    velocity: Velocity,
    wagons: Vec<LoadedWagon>,
}

/// contains the components of a individual wagon (or locomotive)
#[derive(Serialize)]
struct SavedWagon<'a> {
    wagon_type: &'a TrainUnitType,
    stats: &'a WagonStats,
}

#[derive(Deserialize)]
struct LoadedWagon {
    wagon_type: TrainUnitType,
    stats: WagonStats,
}

/// System to listen to keypresses and load/save the game accordingly
fn save_system(mut world: &mut World) {
    let key_input = world.resource::<Input<KeyCode>>();
    if key_input.just_pressed(KeyCode::F6) {
        save_game(&mut world);
    } else if key_input.just_pressed(KeyCode::F5) {
        clean_game(&mut world);
        load_game(&mut world);
    }
}

fn initial_load_system(world: &mut World) {
    load_game(world);
}

/// Helper to save the game
fn save_game(world: &mut World) {
    let mut trains_query = world.query::<(Entity, &Children, &TrainHead, &Velocity)>();
    let mut wagons_query = world.query::<(&TrainUnit, &TrainUnitType, &WagonStats)>();

    let mut trains = Vec::new();
    for (_, children, head, velocity) in trains_query.iter(world) {
        // Thanks a lot to https://stackoverflow.com/a/72605922/19331219
        let mut wagons = (0..children.len()).map(|_| None).collect::<Vec<_>>();
        for &child in children.iter() {
            if let Ok((unit_id, unit_type, unit_stats)) = wagons_query.get(world, child) {
                let wagon = SavedWagon {
                    wagon_type: unit_type,
                    stats: unit_stats,
                };
                wagons[unit_id.position as usize] = Some(wagon);
            }
        }
        let wagons = wagons.into_iter().map(Option::unwrap).collect();

        let train = SavedTrain {
            controller: head,
            velocity: velocity,
            wagons: wagons,
        };
        trains.push(train);
    }

    let graph = world.resource::<RailGraph>();
    let savegame = SavedGame {
        version: CURRENT_SAVEGAME_VERSION,
        network: graph,
        trains: trains,
    };
    let savegame_data = serde_json::to_string(&savegame).expect("Couldn't serialize savegame");
    fs::write(SAVEGAME_PATH, savegame_data).expect("Couldn't write to file");
    info!("Saved game state");
}

/// This helper function takes the same query as save and instead despawns relevant entities
fn clean_game(world: &mut World) {
    let mut rail_root = world.query_filtered::<Entity, With<NetworkRoot>>();
    // This iter then collect then iter is possible because, no two matching entites are in hierachical relation.
    for entity in rail_root.iter(world).collect::<Vec<Entity>>().iter() {
        world.entity_mut(entity.clone()).despawn_recursive();
    }

    let mut trains = world.query_filtered::<Entity, With<TrainHead>>();
    for entity in trains.iter_mut(world).collect::<Vec<Entity>>().iter() {
        world.entity_mut(entity.clone()).despawn_recursive();
    }
}

fn load_savegame_file() -> Result<LoadedGame, Box<dyn Error>> {
    let savegame_data = fs::read_to_string(SAVEGAME_PATH)?;
    let savegame: LoadedGame = serde_json::from_str(savegame_data.as_str())?;
    info!("Loaded savegame v{}", savegame.version);
    // Todo: implement this, problem is, usually the from_str fails anyway, so this is unnesssery.
    // if savegame.version != CURRENT_SAVEGAME_VERSION {
    //     Err(format!(
    //         "Savegame is not compatible: expected {}, was {}!",
    //         CURRENT_SAVEGAME_VERSION, savegame.version
    //     ))
    // }
    Ok(savegame)
}

/// Helper to spawn the dynamic game state from a savegame. Requires the state to be cleaned first.
fn load_game(world: &mut World) {
    let atlases = world.resource::<SpriteAtlases>();

    let savegame = load_savegame_file().unwrap_or_else(|err| {
        info!("Creating new world, because: {}", err);
        LoadedGame::default()
    });

    let mut command_queue = CommandQueue::default();
    let mut commands = Commands::new(&mut command_queue, world);

    // Trains
    for train in savegame.trains {
        let mut wagons = Vec::new();
        for (index, wagon) in train.wagons.into_iter().enumerate() {
            wagons.push(spawn_wagon(
                &mut commands,
                atlases,
                wagon.wagon_type,
                wagon.stats,
                index as u32,
            ));
        }

        commands
            .spawn(TrainBundle {
                controller: train.controller,
                velocity: train.velocity,
                name: Name::new("Loaded Train"),
                local_transform: Transform::default(),
                global_transform: GlobalTransform::default(),
                visiblity: Default::default(),
            })
            .push_children(&wagons);
    }

    // Rails
    let rail_root = commands
        .spawn(SpatialBundle::default())
        .insert(NetworkRoot)
        .insert(Name::new("Rail Network"))
        .id();
    for (start, end, _edge) in savegame.network.graph.all_edges() {
        let Some(tt) = Track::from_joints(start, end) else {
            error!("Broken Graph: edge which does not represent a track {start:?}->{end:?}");
            return;
        };
        if tt.is_canonical_orientation() {
            spawn_rail(&mut commands, atlases, rail_root, tt);
        }
    }

    command_queue.apply(world);
    world.insert_resource(savegame.network);
}

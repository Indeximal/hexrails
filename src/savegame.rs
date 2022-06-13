use std::{error::Error, fs};

use bevy::prelude::*;
use petgraph::graphmap::DiGraphMap;
use serde::{Deserialize, Serialize};

use crate::{
    railroad::{spawn_rail, RailAtlas, RailGraph, RailNetworkRoot},
    trains::{spawn_wagon, TrainAtlas, TrainHead, TrainUnit, TrainUnitType, WagonStats},
};

const SAVEGAME_PATH: &str = "savegame/stupid.json";
const CURRENT_SAVEGAME_VERSION: u32 = 3;

pub struct LoadSavePlugin;

impl Plugin for LoadSavePlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system_to_stage(StartupStage::PostStartup, initial_load_system)
            .add_system(save_system);
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
    wagons: Vec<SavedWagon<'a>>,
}

#[derive(Deserialize)]
struct LoadedTrain {
    controller: TrainHead,
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
fn save_system(
    key_input: Res<Input<KeyCode>>,
    mut commands: Commands,
    graph_res: Res<RailGraph>,
    trains: Query<(Entity, &Children, &TrainHead)>,
    wagons: Query<(&TrainUnit, &TrainUnitType, &WagonStats)>,
    train_atlas: Res<TrainAtlas>,
    rail_atlas: Res<RailAtlas>,
    rail_root: Query<Entity, With<RailNetworkRoot>>,
) {
    let graph = graph_res.as_ref();
    if key_input.just_pressed(KeyCode::F6) {
        save_game(graph, trains, wagons);
    } else if key_input.just_pressed(KeyCode::F5) {
        clean_game(&mut commands, trains, rail_root.single());
        load_game(&mut commands, train_atlas.as_ref(), rail_atlas.as_ref());
    }
}

fn initial_load_system(
    mut commands: Commands,
    train_atlas: Res<TrainAtlas>,
    rail_atlas: Res<RailAtlas>,
) {
    load_game(&mut commands, train_atlas.as_ref(), rail_atlas.as_ref());
}

/// Helper to save the game
fn save_game(
    graph: &RailGraph,
    trains_query: Query<(Entity, &Children, &TrainHead)>,
    wagons_query: Query<(&TrainUnit, &TrainUnitType, &WagonStats)>,
) {
    info!("Saving game state");
    // Because of lifetimes I can't easily extract this into a function
    let mut trains = Vec::new();
    for (_, children, head) in trains_query.iter() {
        // Thanks a lot to https://stackoverflow.com/a/72605922/19331219
        let mut wagons = (0..children.len()).map(|_| None).collect::<Vec<_>>();
        for &child in children.iter() {
            if let Ok((unit_id, unit_type, unit_stats)) = wagons_query.get(child) {
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
            wagons: wagons,
        };
        trains.push(train);
    }

    let savegame = SavedGame {
        version: CURRENT_SAVEGAME_VERSION,
        network: graph,
        trains: trains,
    };
    let savegame_data = serde_json::to_string(&savegame).expect("Couldn't serialize savegame");
    fs::write(SAVEGAME_PATH, savegame_data).expect("Couldn't write to file");
}

/// This helper function takes the same query as save and instead despawns relevant entities
fn clean_game(
    commands: &mut Commands,
    trains: Query<(Entity, &Children, &TrainHead)>,
    rail_root: Entity,
) {
    commands.entity(rail_root).despawn_recursive();
    for (entity, _, _) in trains.iter() {
        commands.entity(entity).despawn_recursive();
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
fn load_game(commands: &mut Commands, train_atlas: &TrainAtlas, rail_atlas: &RailAtlas) {
    let savegame = load_savegame_file().unwrap_or_default();

    // Trains
    for train in savegame.trains {
        let mut wagons = Vec::new();
        for (index, wagon) in train.wagons.into_iter().enumerate() {
            wagons.push(spawn_wagon(
                commands,
                train_atlas,
                wagon.wagon_type,
                wagon.stats,
                index as u32,
            ));
        }

        // todo: use helper function
        commands
            .spawn()
            .insert_bundle(TransformBundle::default())
            .insert(Name::new("Train"))
            .insert(train.controller)
            .push_children(&wagons);
    }

    // Rails
    let rail_root = commands
        .spawn_bundle(TransformBundle::default())
        .insert(RailNetworkRoot)
        .insert(Name::new("Rail Network"))
        .id();
    for (start, _, edge) in savegame.network.graph.all_edges() {
        if let Some(rail_type) = edge.display_type {
            spawn_rail(
                commands, rail_atlas, rail_root, start.tile, start.side, rail_type,
            );
        }
    }
    commands.insert_resource(savegame.network);
}

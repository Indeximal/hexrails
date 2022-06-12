use std::fs;

use bevy::prelude::*;
use petgraph::graphmap::DiGraphMap;
use serde::{Deserialize, Serialize};

use crate::{
    railroad::RailGraph,
    trains::{TrainHead, TrainUnit, TrainUnitType},
};

const SAVEGAME_PATH: &str = "savegame/stupid.json";

pub struct LoadSavePlugin;

impl Plugin for LoadSavePlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(initial_load_system)
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
    network: &'a RailGraph,
    trains: Vec<SavedTrain<'a>>,
}

#[derive(Deserialize)]
struct LoadedGame {
    network: RailGraph,
    trains: Vec<LoadedTrain>,
}

impl Default for LoadedGame {
    fn default() -> Self {
        Self {
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
    wagons: Vec<SavedWagon>,
}

#[derive(Deserialize)]
struct LoadedTrain {
    controller: TrainHead,
    wagons: Vec<LoadedWagon>,
}

/// contains the components of a individual wagon (or locomotive)
#[derive(Serialize)]
struct SavedWagon {
    wagon_type: TrainUnitType,
}

#[derive(Deserialize)]
struct LoadedWagon {
    wagon_type: TrainUnitType,
}

fn save_system(
    key_input: Res<Input<KeyCode>>,
    mut commands: Commands,
    graph_res: Res<RailGraph>,
    trains: Query<(&Children, &TrainHead)>,
    wagons: Query<(&TrainUnitType, &TrainUnit)>,
) {
    let graph = graph_res.as_ref();
    if key_input.just_pressed(KeyCode::F6) {
        save_game(graph, trains, wagons);
    }
    if key_input.just_pressed(KeyCode::F5) {
        load_game(&mut commands);
    }
}

fn initial_load_system(mut commands: Commands) {
    load_game(&mut commands);
}

fn save_game(
    graph: &RailGraph,
    trains_query: Query<(&Children, &TrainHead)>,
    wagons_query: Query<(&TrainUnitType, &TrainUnit)>,
) {
    info!("Saving game state");
    // Because of lifetimes I can't easily extract this into a function
    let mut trains = Vec::new();
    for (children, head) in trains_query.iter() {
        // Wagons here are sorted after insertion into the vector, which is suboptimal, since
        // the index is technically known. But the borrow checker notices that this is not
        // particullarly nice. Problem is that `Option` needs Clone.
        let mut wagons_with_id = Vec::new();
        for &child in children.iter() {
            if let Ok((&unit_type, unit_id)) = wagons_query.get(child) {
                let wagon = SavedWagon {
                    wagon_type: unit_type,
                };
                wagons_with_id.push((wagon, unit_id.position));
            }
        }
        // This reverse sort and stack based map is needed because I don't want to make `SavedWagon` Clone.
        // Might have been a dump idea xD.
        wagons_with_id.sort_by_key(|(_, i)| -(*i as i32));
        let mut wagons = Vec::new();
        while !wagons_with_id.is_empty() {
            wagons.push(wagons_with_id.pop().unwrap().0);
        }

        let train = SavedTrain {
            controller: head,
            wagons: wagons,
        };
        trains.push(train);
    }

    let savegame = SavedGame {
        network: graph,
        trains: trains,
    };
    let savegame_data = serde_json::to_string(&savegame).expect("Couldn't serialize savegame");
    fs::write(SAVEGAME_PATH, savegame_data).expect("Couldn't write to file");
}

fn load_game(commands: &mut Commands) {
    let savegame_data = fs::read_to_string(SAVEGAME_PATH);
    let savegame = match savegame_data {
        Ok(save_str) => {
            serde_json::from_str(save_str.as_str()).expect("Coundn't deserialize savegame")
        }
        Err(_) => LoadedGame::default(),
    };

    // replacing previous RailGraph resource
    commands.insert_resource(savegame.network);
    // Todo: spawn the sprites aswell
}

use std::fs;

use bevy::{
    input::{keyboard::KeyboardInput, ElementState},
    prelude::*,
};
use petgraph::graphmap::DiGraphMap;

use crate::railroad::RailGraph;

const SAVEGAME_PATH: &str = "savegame/stupid.json";

pub struct LoadSavePlugin;

impl Plugin for LoadSavePlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(initial_load_system)
            .add_system(save_system);
    }
}

fn save_system(
    mut key_events: EventReader<KeyboardInput>,
    mut commands: Commands,
    graph_res: Res<RailGraph>,
) {
    let graph = graph_res.as_ref();
    for ev in key_events.iter() {
        if ev.state == ElementState::Pressed {
            match ev.key_code {
                Some(KeyCode::O) => save_game(graph),
                Some(KeyCode::L) => load_game(&mut commands),
                _ => {}
            };
        }
    }
}

fn initial_load_system(mut commands: Commands) {
    load_game(&mut commands);
}

fn save_game(graph: &RailGraph) {
    info!("Saving game state");
    let out = serde_json::to_string(graph).expect("Couldn't serialize rail graph");
    fs::write(SAVEGAME_PATH, out).expect("Couldn't write to file");
}

fn load_game(commands: &mut Commands) {
    let graph_json = fs::read_to_string(SAVEGAME_PATH);
    let graph = match graph_json {
        Ok(save_str) => {
            serde_json::from_str(save_str.as_str()).expect("Coundn't deserialize graph")
        }
        Err(_) => RailGraph {
            graph: DiGraphMap::new(),
        },
    };

    commands.insert_resource(graph);
}

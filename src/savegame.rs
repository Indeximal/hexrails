use bevy::{
    input::{keyboard::KeyboardInput, ElementState},
    prelude::*,
};

use crate::railroad::RailGraph;

pub struct LoadSavePlugin;

impl Plugin for LoadSavePlugin {
    fn build(&self, app: &mut App) {
        app.add_system(save_system);
    }
}

fn save_system(mut key_events: EventReader<KeyboardInput>, graph_res: Res<RailGraph>) {
    let graph = graph_res.as_ref();
    for ev in key_events.iter() {
        if ev.state == ElementState::Pressed {
            match ev.key_code {
                Some(KeyCode::O) => save_game(graph),
                Some(KeyCode::L) => load_game(),
                _ => {}
            }
        }
    }
}

fn save_game(graph: &RailGraph) {
    info!("Saving game state");
    let out = serde_json::to_string(graph);
    println!("{:?}", out);
}

fn load_game() {}

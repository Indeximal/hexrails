use std::{error::Error, fs};

use bevy::{ecs::system::CommandQueue, prelude::*};

use petgraph::graphmap::DiGraphMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::input::{Action, GameInput};
use crate::railroad::{spawn_rail, NetworkRoot, RailGraph, Track};
use crate::sprites::SpriteAtlases;
use crate::trainbuilder::*;
use crate::trains::*;

const SAVEGAME_PATH: &str = "savegame/stupid.json";
const CURRENT_SAVEGAME_VERSION: u32 = 6;

pub struct LoadSavePlugin;

impl Plugin for LoadSavePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, initial_load_system)
            .add_systems(Last, save_system);
    }
}

/// This is a struct holding all the data that will get saved.
///
/// Actual data is wrapped in [`SerDeserCell`] so no clones need to be performed.
#[derive(Serialize, Deserialize)]
struct SaveGame<'a> {
    version: u32,
    network: SerDeserCell<'a, RailGraph>,
    trains: Vec<SaveTrain<'a>>,
}

#[derive(Serialize, Deserialize)]
/// contains the path and position of the whole train and all its wagons
struct SaveTrain<'a> {
    train: SerDeserCell<'a, Trail>,
    velocity: SerDeserCell<'a, Velocity>,
    wagons: Vec<SaveWagon<'a>>,
}

/// contains the components of a individual wagon (or locomotive)
#[derive(Serialize, Deserialize)]
struct SaveWagon<'a> {
    wagon_type: SerDeserCell<'a, VehicleType>,
    stats: SerDeserCell<'a, VehicleStats>,
}

/// This is a new game.
impl<'a> Default for SaveGame<'a> {
    fn default() -> Self {
        Self {
            version: CURRENT_SAVEGAME_VERSION,
            network: SerDeserCell::Deser(RailGraph {
                graph: DiGraphMap::new(),
            }),
            trains: Vec::new(),
        }
    }
}
impl<'a> SaveGame<'a> {
    fn from_disk() -> Self {
        fn load_savegame_file() -> Result<SaveGame<'static>, Box<dyn Error>> {
            let savegame_data = fs::read_to_string(SAVEGAME_PATH)?;
            let savegame: SaveGame = serde_json::from_str(savegame_data.as_str())?;
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

        load_savegame_file().unwrap_or_else(|err| {
            info!("Creating new world, because: {}", err);
            SaveGame::default()
        })
    }

    fn from_world(world: &'a mut World) -> Self {
        let mut trains_query = world.query::<(Entity, &Children, &Trail, &Velocity)>();
        let mut wagons_query = world.query::<(&TrainIndex, &VehicleType, &VehicleStats)>();

        let mut trains = Vec::new();
        for (_, children, head, velocity) in trains_query.iter(world) {
            // Thanks a lot to https://stackoverflow.com/a/72605922/19331219
            let mut wagons = (0..children.len()).map(|_| None).collect::<Vec<_>>();
            for &child in children.iter() {
                if let Ok((unit_id, unit_type, unit_stats)) = wagons_query.get(world, child) {
                    let wagon = SaveWagon {
                        wagon_type: SerDeserCell::Ser(&unit_type),
                        stats: SerDeserCell::Ser(&unit_stats),
                    };
                    wagons[unit_id.position as usize] = Some(wagon);
                }
            }
            let wagons = wagons.into_iter().map(Option::unwrap).collect();

            let train = SaveTrain {
                train: SerDeserCell::Ser(&head),
                velocity: SerDeserCell::Ser(&velocity),
                wagons: wagons,
            };
            trains.push(train);
        }

        let graph = world.resource::<RailGraph>();
        SaveGame {
            version: CURRENT_SAVEGAME_VERSION,
            network: SerDeserCell::Ser(&graph),
            trains: trains,
        }
    }
}

/// System to listen to keypresses and load/save the game accordingly
fn save_system(world: &mut World) {
    let key_input = world.resource::<GameInput>();
    if key_input.just_pressed(&Action::Save) {
        save_game(world);
    } else if key_input.just_pressed(&Action::Load) {
        clean_game(world);
        load_game(world, SaveGame::from_disk());
    } else if key_input.just_pressed(&Action::LoadNew) {
        clean_game(world);
        load_game(world, SaveGame::default());
    }
}

fn initial_load_system(world: &mut World) {
    load_game(world, SaveGame::from_disk());
}

/// Helper to save the game
fn save_game(world: &mut World) {
    let savegame = SaveGame::from_world(world);
    let savegame_data = serde_json::to_string(&savegame).expect("Couldn't serialize savegame");
    fs::write(SAVEGAME_PATH, savegame_data).expect("Couldn't write to file");
    info!("Saved game state");
}

/// This helper function despawns relevant entities which constitute a savegame state
fn clean_game(world: &mut World) {
    let mut rail_root = world.query_filtered::<Entity, With<NetworkRoot>>();
    // This iter then collect then iter is possible because, no two matching entites are in hierachical relation.
    for entity in rail_root.iter(world).collect::<Vec<Entity>>().iter() {
        world.entity_mut(entity.clone()).despawn_recursive();
    }

    let mut trains = world.query_filtered::<Entity, With<Trail>>();
    for entity in trains.iter_mut(world).collect::<Vec<Entity>>().iter() {
        world.entity_mut(entity.clone()).despawn_recursive();
    }
}

/// Helper to spawn the dynamic game state from a savegame. Requires the state to be cleaned first.
fn load_game(world: &mut World, savegame: SaveGame) {
    let atlases = world.resource::<SpriteAtlases>();
    let mut command_queue = CommandQueue::default();
    let mut commands = Commands::new(&mut command_queue, world);

    // Trains
    for train in savegame.trains {
        let trail = train.train.get();
        if !trail.check_invariant() {
            error!("savegame contains broken trail, skipping loading this train");
            continue;
        }
        debug!("Loading train on: {trail:?}");

        let mut wagons = Vec::new();
        for (index, wagon) in train.wagons.into_iter().enumerate() {
            wagons.push(spawn_wagon(
                &mut commands,
                atlases,
                wagon.wagon_type.get(),
                wagon.stats.get(),
                index as u16,
            ));
        }

        commands
            .spawn(TrainBundle {
                path: trail,
                velocity: train.velocity.get(),
                controller: Default::default(),
                name: Name::new("Train"),
                spatial: Default::default(),
                marker: TrainMarker,
            })
            .push_children(&wagons);
    }

    // Rails
    let rail_root = commands
        .spawn(SpatialBundle::default())
        .insert(NetworkRoot)
        .insert(Name::new("Rail Network"))
        .id();
    let network = savegame.network.get();
    for (start, end, _edge) in network.graph.all_edges() {
        let Some(tt) = Track::from_joints(start, end) else {
            error!("Broken Graph: edge which does not represent a track {start:?}->{end:?}");
            return;
        };
        if tt.is_canonical_orientation() {
            spawn_rail(&mut commands, atlases, rail_root, tt);
        }
    }

    command_queue.apply(world);
    world.insert_resource(network);
}

/// In order to avoid many clones, this enum provides a Cow similar construct,
/// but one where the data doesn't need to be clone and can only be inserted
/// or fetched.
///
/// For a deserialized [`SerDeserCell`] it is garanteed that `get()` will not panic.
enum SerDeserCell<'a, T>
where
    T: 'a,
{
    /// Borrowed data for serialization
    Ser(&'a T),
    /// Owned data for deserialization
    Deser(T),
}

impl<'a, T> SerDeserCell<'a, T> {
    fn get(self) -> T {
        match self {
            SerDeserCell::Ser(_) => panic!("get should only be called for owned data"),
            SerDeserCell::Deser(x) => x,
        }
    }
}

impl<'a, T> Serialize for SerDeserCell<'a, T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            SerDeserCell::Ser(x) => x.serialize(serializer),
            SerDeserCell::Deser(x) => x.serialize(serializer),
        }
    }
}

impl<'a, 'de, T> Deserialize<'de> for SerDeserCell<'a, T>
where
    T: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self::Deser(Deserialize::deserialize(deserializer)?))
    }
}

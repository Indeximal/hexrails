use bevy::{ecs::schedule::ShouldRun, prelude::*};
use petgraph::EdgeDirection;

use crate::{railroad::RailGraph, tilemap::*, trains::*, ui::InteractingState};

const Z_LAYER_TRAINS: f32 = 0.3;

pub struct TrainBuildingPlugin;

impl Plugin for TrainBuildingPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system_to_stage(StartupStage::PreStartup, load_texture_atlas)
            .add_system_set(
                SystemSet::new()
                    .with_run_criteria(train_builder_condition)
                    .with_system(train_builder),
            );
    }
}

impl TrainHead {
    /// Returns the index of the wagon currently nearest to `face` or `length` if at the end.
    /// Returns none if the face is not on the path or not near any wagon.
    pub fn index_for_tile(&self, face: TileFace) -> Option<u32> {
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
            Some(index.floor() as u32)
        }
    }
}

impl TrainUnitType {
    fn into_texture_atlas_index(&self) -> usize {
        match self {
            Self::Locomotive => 1,
            Self::Wagon => 0,
        }
    }
}

fn train_builder_condition(state: Res<State<InteractingState>>) -> ShouldRun {
    match state.current() {
        InteractingState::PlaceTrains(_) => ShouldRun::Yes,
        _ => ShouldRun::No,
    }
}

/// This system tries to place a train wagon or a new train on click
fn train_builder(
    mut commands: Commands,
    atlas: Res<TrainAtlas>,
    mut click_event: EventReader<TileClickEvent>,
    rail_graph: Res<RailGraph>,
    state: Res<State<InteractingState>>,
    mut trains: Query<(Entity, &Children, &mut TrainHead)>,
    wagons: Query<&mut TrainUnit>,
) {
    let graph = rail_graph.as_ref();
    let wagon_type = match state.current() {
        InteractingState::PlaceTrains(v) => v.clone(),
        _ => unreachable!(
            "The run condition should insure that the train builder is only run in the PlaceTrains state!"
        ),
    };
    for ev in click_event.iter() {
        if ev.side.is_none() {
            // for now ignore clicks in the center: might be ambigous
            continue;
        }
        // todo: also consider the opposite tile face
        let face = TileFace {
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
                    WagonStats::default_for_type(wagon_type),
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
    atlas: &TrainAtlas,
    face: TileFace,
    rail_graph: &RailGraph,
    wagon_type: TrainUnitType,
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
        WagonStats::default_for_type(wagon_type),
        0,
    );

    commands
        .spawn_bundle(TrainBundle::new(
            TrainHead {
                path: vec![face, next_face],
                path_progress: 1.0,
                length: 1,
            },
            20. / 60.,
        ))
        .insert_bundle(VisibilityBundle::default())
        .add_child(first_wagon);
}

/// Helper to insert a wagon into an existing train and move all other wagons accordingly
fn insert_wagon(
    commands: &mut Commands,
    atlas: &TrainAtlas,
    sibling_ids: &Children,
    mut sibling_query: Query<&mut TrainUnit>,
    parent: Entity,
    wagon_type: TrainUnitType,
    wagon_stats: WagonStats,
    insert_index: u32,
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
    atlas: &TrainAtlas,
    wagon_type: TrainUnitType,
    wagon_stats: WagonStats,
    insert_index: u32,
) -> Entity {
    let mut sprite = TextureAtlasSprite::new(wagon_type.into_texture_atlas_index());
    sprite.custom_size = Some(Vec2::splat(TILE_SCALE));

    commands
        .spawn_bundle(SpriteSheetBundle {
            sprite: sprite,
            texture_atlas: atlas.0.clone(),
            transform: Transform::from_translation(Vec3::Z * Z_LAYER_TRAINS),
            ..Default::default()
        })
        .insert(TrainUnit {
            position: insert_index,
        })
        .insert(wagon_type)
        .insert(wagon_stats)
        .insert(Name::new("Wagon"))
        .id()
}

pub struct TrainAtlas(Handle<TextureAtlas>);

/// System to load the sprite sheet
fn load_texture_atlas(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas: ResMut<Assets<TextureAtlas>>,
) {
    let image = asset_server.load("TrainAtlas.png");
    let atlas = TextureAtlas::from_grid_with_padding(
        image,
        Vec2::new(TILE_SIZE, TILE_SIZE),
        1,
        2,
        Vec2::splat(2.0),
        Vec2::splat(1.0), // There is padding at the very edges (?)
    );
    let atlas_handle = texture_atlas.add(atlas);
    commands.insert_resource(TrainAtlas(atlas_handle));
}

use crate::{tilemap::*, ui::InteractingState};
use bevy::{ecs::schedule::ShouldRun, prelude::*};
use petgraph::graphmap::DiGraphMap;
use serde::{Deserialize, Serialize};

const Z_LAYER_RAILS: f32 = 0.2;

pub struct RailRoadPlugin;

impl Plugin for RailRoadPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system_to_stage(StartupStage::PreStartup, load_texture_atlas)
            .add_system_set(
                SystemSet::new()
                    .with_run_criteria(rail_builder_condition)
                    .with_system(rail_builder),
            );
    }
}

#[derive(Component)]
pub struct RailNetworkRoot;

#[derive(Serialize, Deserialize)]
pub struct RailGraph {
    pub graph: DiGraphMap<TileFace, RailEdge>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RailEdge {
    /// Type of the rail
    pub rail_type: RailType,
    pub should_render: bool,
}

#[derive(Component)]
pub struct RailTile {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RailType {
    Straight,
    CurvedLeft,
    CurvedRight,
}

impl RailType {
    pub fn mirror(&self) -> Self {
        match self {
            RailType::Straight => RailType::Straight,
            RailType::CurvedLeft => RailType::CurvedRight,
            RailType::CurvedRight => RailType::CurvedLeft,
        }
    }
}

impl TileFace {
    pub fn next_face_with(&self, rail: RailType) -> TileFace {
        match rail {
            RailType::Straight => TileFace {
                tile: self.tile.neighbor(self.side.opposite()),
                side: self.side,
            },
            RailType::CurvedLeft => TileFace {
                tile: self.tile.neighbor(self.side.curve_left()),
                side: self.side.curve_left().opposite(),
            },
            RailType::CurvedRight => TileFace {
                tile: self.tile.neighbor(self.side.curve_right()),
                side: self.side.curve_right().opposite(),
            },
        }
    }
}

fn rail_builder_condition(state: Res<State<InteractingState>>) -> ShouldRun {
    match state.current() {
        InteractingState::PlaceRails(_) => ShouldRun::Yes,
        _ => ShouldRun::No,
    }
}

/// This system tries to build rails both in the graph and with sprites when the mouse is clicked.
fn rail_builder(
    mut commands: Commands,
    atlas_h: Res<RailAtlas>,
    mut click_event: EventReader<TileClickEvent>,
    mut rail_graph: ResMut<RailGraph>,
    root_query: Query<Entity, With<RailNetworkRoot>>,
    state: Res<State<InteractingState>>,
) {
    let rail_graph = rail_graph.as_mut();
    let root_entity = root_query.single();
    let rail_type = match state.current() {
        InteractingState::PlaceRails(v) => v.clone(),
        _ => unreachable!(
            "The run condition should insure that the rail builder is only run in the PlaceRails state!"
        ),
    };

    for evt in click_event.iter() {
        if let Some(side) = evt.side {
            match evt.button {
                MouseButton::Left => build_rail(
                    &mut commands,
                    &atlas_h,
                    root_entity,
                    rail_graph,
                    evt.coord,
                    side,
                    rail_type,
                ),
                _ => (),
            }
        }
    }
}

/// This helper function tries to build a single rail if it doesn't already exist
fn build_rail(
    commands: &mut Commands,
    atlas: &Res<RailAtlas>,
    root_entity: Entity,
    rail_graph: &mut RailGraph,
    position: TileCoordinate,
    start_side: TileSide,
    rail_type: RailType,
) {
    let start_face = TileFace {
        tile: position,
        side: start_side,
    };
    let end_face = start_face.next_face_with(rail_type);

    let edge1 = rail_graph.graph.add_edge(
        start_face,
        end_face,
        RailEdge {
            rail_type: rail_type,
            should_render: true,
        },
    );
    let edge2 = rail_graph.graph.add_edge(
        end_face.opposite(),
        start_face.opposite(),
        RailEdge {
            rail_type: rail_type.mirror(),
            should_render: false,
        },
    );
    // if neither edge previously existed:
    if edge1.is_none() && edge2.is_none() {
        info!("Rail built @{} -> {}", start_face.tile, end_face.tile);
        spawn_rail(
            commands,
            atlas,
            root_entity,
            position,
            start_side,
            rail_type,
        );
    }
}

/// This helper function spawns a rail sprite
pub fn spawn_rail(
    commands: &mut Commands,
    atlas: &RailAtlas,
    root_entity: Entity,
    position: TileCoordinate,
    start_side: TileSide,
    rail_type: RailType,
) {
    let index = match rail_type {
        RailType::Straight => 0,
        RailType::CurvedLeft => 1,
        RailType::CurvedRight => 1,
    };
    let flipped = match rail_type {
        RailType::Straight => false,
        RailType::CurvedLeft => true,
        RailType::CurvedRight => false,
    };
    let mut sprite = TextureAtlasSprite::new(index);
    sprite.custom_size = Some(Vec2::splat(TILE_SCALE));
    sprite.flip_y = flipped;

    let child = commands
        .spawn_bundle(SpriteSheetBundle {
            sprite: sprite,
            texture_atlas: atlas.0.clone(),
            transform: Transform {
                translation: position.into_world_pos().extend(Z_LAYER_RAILS),
                rotation: Quat::from_rotation_z(start_side.to_angle()),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(Name::new(format!("Rail {}", position)))
        .insert(RailTile {})
        .insert(position)
        .id();

    commands.entity(root_entity).add_child(child);
}

pub struct RailAtlas(Handle<TextureAtlas>);

/// This system loads the sprite atlas for the rails
fn load_texture_atlas(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas: ResMut<Assets<TextureAtlas>>,
) {
    let image = asset_server.load("RailAtlas.png");
    let atlas = TextureAtlas::from_grid_with_padding(
        image,
        Vec2::new(TILE_SIZE, TILE_SIZE),
        1,
        2,
        Vec2::splat(1.0),
    );
    let atlas_handle = texture_atlas.add(atlas);
    commands.insert_resource(RailAtlas(atlas_handle));
}

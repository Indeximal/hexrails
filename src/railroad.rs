use crate::tilemap::*;
use bevy::prelude::*;
use petgraph::graphmap::DiGraphMap;
use serde::{Deserialize, Serialize};

const Z_LAYER_RAILS: f32 = 0.2;

pub struct RailRoadPlugin;

impl Plugin for RailRoadPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system_to_stage(StartupStage::PreStartup, load_texture_atlas)
            .add_startup_system(load_rail_graph)
            .add_system(rail_builder);
    }
}

#[derive(Serialize, Deserialize)]
pub struct RailGraph {
    pub graph: DiGraphMap<TileFace, ()>,
}

#[derive(Component)]
pub struct RailTile {}

#[derive(Debug, Clone, Copy)]
pub enum RailType {
    Straight,
    CurvedLeft,
    CurvedRight,
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

fn rail_builder(
    mut commands: Commands,
    atlas_h: Res<RailAtlas>,
    mut click_event: EventReader<TileClickEvent>,
    mut rail_graph: ResMut<RailGraph>,
) {
    let rail_graph = rail_graph.as_mut();
    for evt in click_event.iter() {
        if let Some(side) = evt.side {
            match evt.button {
                MouseButton::Left => build_rail(
                    &mut commands,
                    &atlas_h,
                    rail_graph,
                    evt.coord,
                    side,
                    RailType::Straight,
                ),
                MouseButton::Right => build_rail(
                    &mut commands,
                    &atlas_h,
                    rail_graph,
                    evt.coord,
                    side,
                    RailType::CurvedRight,
                ),
                MouseButton::Middle => build_rail(
                    &mut commands,
                    &atlas_h,
                    rail_graph,
                    evt.coord,
                    side,
                    RailType::CurvedLeft,
                ),
                _ => (),
            }
        }
    }
}

fn build_rail(
    commands: &mut Commands,
    atlas: &Res<RailAtlas>,
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

    let edge1 = rail_graph.graph.add_edge(start_face, end_face, ());
    let edge2 = rail_graph
        .graph
        .add_edge(end_face.opposite(), start_face.opposite(), ());
    // if neither edge previously existed:
    if edge1.is_none() && edge2.is_none() {
        info!("Rail built @{} -> {}", start_face.tile, end_face.tile);
        spawn_rail(commands, atlas, position, start_side, rail_type);
    }
}

fn spawn_rail(
    commands: &mut Commands,
    atlas: &Res<RailAtlas>,
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

    commands
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
        .insert(position);
}

struct RailAtlas(Handle<TextureAtlas>);

fn load_rail_graph(mut commands: Commands) {
    commands.insert_resource(RailGraph {
        graph: DiGraphMap::new(),
    })
}

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

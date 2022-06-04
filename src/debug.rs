use bevy::prelude::*;
use bevy_inspector_egui::{RegisterInspectable, WorldInspectorPlugin};
use bevy_prototype_debug_lines::{DebugLines, DebugLinesPlugin};

use crate::{
    railroad::RailGraph,
    tilemap::{TileCoordinate, TileFace, TILE_SCALE},
};

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        if cfg!(debug_assertions) {
            app.add_plugin(WorldInspectorPlugin::new())
                .add_plugin(DebugLinesPlugin::default())
                .register_inspectable::<TileCoordinate>()
                .add_system(draw_rail_graph);
        }
    }
}

fn face_position(face: TileFace) -> Vec3 {
    let origin = Vec2::from(face.tile);
    let angle = face.side.to_angle();
    let offset = Vec2::new(angle.cos(), angle.sin()) * TILE_SCALE / 2. * 0.6;
    (origin + offset).extend(999.)
}

fn draw_rail_graph(graph: Res<RailGraph>, mut lines: ResMut<DebugLines>) {
    for (from, to, _) in graph.graph.all_edges() {
        //println!("{:?}->{:?}", from.tile, to.tile);
        lines.line_colored(face_position(from), face_position(to), 0., Color::BLACK);
    }
}

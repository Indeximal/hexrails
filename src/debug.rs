use std::f32::consts::PI;

use bevy::prelude::*;
use bevy_inspector_egui::quick::WorldInspectorPlugin;

use crate::railroad::RailGraph;
use crate::tilemap::{Joint, TILE_SCALE};

pub struct DebugPlugin;
impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        if cfg!(debug_assertions) {
            app.add_plugins(WorldInspectorPlugin::new())
                .add_systems(Update, draw_rail_graph);
        }
    }
}

fn joint_position(face: Joint) -> Vec2 {
    let origin = face.tile.world_pos();
    // Add a angle offset for better visibility
    let angle = face.side.to_angle() + PI / 8.;
    let offset = Vec2::new(angle.cos(), angle.sin()) * TILE_SCALE / 2. * 0.8;
    origin + offset
}

fn draw_rail_graph(graph: Res<RailGraph>, mut gizmos: Gizmos) {
    for (from, to, _) in graph.graph.all_edges() {
        gizmos.line_2d(joint_position(from), joint_position(to), Color::BLUE);
    }
}

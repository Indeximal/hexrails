use std::f32::consts::PI;

use bevy::color::palettes;
use bevy::prelude::*;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_rapier2d::render::RapierDebugRenderPlugin;

use crate::interact::{NodeClickEvent, TileClickEvent};
use crate::railroad::RailGraph;
use crate::tilemap::{Joint, TILE_SCALE};
use crate::trains::{PlayerControlledTrain, Trail, Velocity};

pub struct DebugPlugin;
impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        if cfg!(debug_assertions) {
            app.add_plugins(WorldInspectorPlugin::new())
                .add_plugins(RapierDebugRenderPlugin::default())
                .register_type::<Velocity>()
                .add_systems(Update, (draw_rail_graph, draw_train_paths, log_collisions))
                .add_systems(Update, log_interaction_events);
        }
        // For framerate:
        // .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
        // .add_plugins(bevy::diagnostic::LogDiagnosticsPlugin::default())
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
        gizmos.line_2d(
            joint_position(from),
            joint_position(to),
            palettes::basic::BLUE,
        );
    }
}

fn draw_train_paths(trains: Query<(&Trail, Option<&PlayerControlledTrain>)>, mut gizmos: Gizmos) {
    for (train, player_control) in trains.iter() {
        for wnd in train.path.windows(2) {
            let from = wnd[0];
            let to = wnd[1];
            gizmos.line_2d(
                from.world_position(),
                to.world_position(),
                if player_control.is_some() {
                    palettes::basic::GREEN
                } else {
                    palettes::basic::OLIVE
                },
            );
        }
    }
}

fn log_collisions(mut collision_events: EventReader<bevy_rapier2d::prelude::CollisionEvent>) {
    for collision_event in collision_events.read() {
        trace!("CollisionEvent::{collision_event:?}");
    }
}

fn log_interaction_events(
    mut t_evs: EventReader<TileClickEvent>,
    mut n_evs: EventReader<NodeClickEvent>,
) {
    for ev in t_evs.read() {
        trace!("{ev:?}");
    }

    for ev in n_evs.read() {
        trace!("{ev:?}");
    }
}

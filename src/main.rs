use bevy::prelude::*;
use camera::MovingCameraPlugin;
use debug::DebugPlugin;
use railroad::RailRoadPlugin;
use savegame::LoadSavePlugin;
use terrain::TerrainPlugin;
use tilemap::TileMapPlugin;
use trainbuilder::TrainBuildingPlugin;
use trains::TrainPlugin;
use ui::UIOverlayPlugin;

mod camera;
mod debug;
mod railroad;
mod savegame;
mod terrain;
mod tilemap;
mod trainbuilder;
mod trains;
mod ui;

pub const BG_COLOR: Color = Color::rgb(0.7, 0.7, 0.7);
pub const ASPECT_RATIO: f32 = 16.0 / 9.0;

fn main() {
    let height = 900.0;
    App::new()
        .insert_resource(ClearColor(BG_COLOR))
        .insert_resource(WindowDescriptor {
            width: height * ASPECT_RATIO,
            height: height,
            title: "Hex Rails".to_string(),
            present_mode: bevy::window::PresentMode::Fifo,
            resizable: false,
            ..Default::default()
        })
        .add_startup_system(print_version)
        .add_plugins(DefaultPlugins)
        .add_plugin(DebugPlugin)
        .add_plugin(TerrainPlugin)
        .add_plugin(TileMapPlugin)
        .add_plugin(RailRoadPlugin)
        .add_plugin(MovingCameraPlugin)
        .add_plugin(LoadSavePlugin)
        .add_plugin(TrainPlugin)
        .add_plugin(UIOverlayPlugin)
        .add_plugin(TrainBuildingPlugin)
        .run();
}

fn print_version() {
    info!("Started game!");
}

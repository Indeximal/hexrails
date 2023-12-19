use bevy::prelude::*;

mod assets;
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
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (height * ASPECT_RATIO, height).into(),
                title: "Hex Rails".to_string(),
                present_mode: bevy::window::PresentMode::Fifo,
                resizable: false,
                ..Default::default()
            }),
            ..Default::default()
        }))
        .add_plugins(assets::AssetPlugin)
        .add_plugins(camera::MovingCameraPlugin)
        .add_plugins(debug::DebugPlugin)
        .add_plugins(railroad::RailRoadPlugin)
        .add_plugins(savegame::LoadSavePlugin)
        .add_plugins(terrain::TerrainPlugin)
        .add_plugins(tilemap::TileMapPlugin)
        .add_plugins(trainbuilder::TrainBuildingPlugin)
        .add_plugins(trains::TrainPlugin)
        .add_plugins(ui::UIOverlayPlugin)
        .run();
}

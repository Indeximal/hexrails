use bevy::prelude::*;
use camera::MovingCameraPlugin;
use terrain::TerrainPlugin;
use tilemap::TileMapPlugin;

mod camera;
mod terrain;
mod tilemap;

pub const BG_COLOR: Color = Color::rgb(0.7, 0.7, 0.7);
pub const ASPECT_RATIO: f32 = 16.0 / 9.0;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(HexRailsPlugin)
        .run();
}
pub struct HexRailsPlugin;

impl Plugin for HexRailsPlugin {
    fn build(&self, app: &mut App) {
        let height = 900.0;

        app.insert_resource(ClearColor(BG_COLOR))
            .insert_resource(WindowDescriptor {
                width: height * ASPECT_RATIO,
                height: height,
                title: "Hex Rails".to_string(),
                present_mode: bevy::window::PresentMode::Fifo,
                resizable: false,
                ..Default::default()
            })
            .add_startup_system(print_version)
            .add_plugin(TerrainPlugin)
            .add_plugin(TileMapPlugin)
            .add_plugin(MovingCameraPlugin);
    }
}

fn print_version() {
    println!("Build with Bevy!");
}

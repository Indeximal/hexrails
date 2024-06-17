use bevy::{log::LogPlugin, prelude::*};
use bevy_rapier2d::plugin::{NoUserData, RapierPhysicsPlugin};

mod camera;
mod collisions;
mod debug;
mod driving;
mod input;
mod interact;
mod railroad;
mod savegame;
mod sprites;
mod terrain;
mod tilemap;
mod trainbuilder;
mod trains;
mod ui;

pub const BG_COLOR: Color = Color::rgb(0.7, 0.7, 0.7);
pub const ASPECT_RATIO: f32 = 16.0 / 9.0;

fn main() {
    let height = 750.0;
    App::new()
        .insert_resource(ClearColor(BG_COLOR))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        resolution: (height * ASPECT_RATIO, height).into(),
                        title: "Hex Rails".to_string(),
                        present_mode: bevy::window::PresentMode::Fifo,
                        resizable: false,
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .set(LogPlugin {
                    level: bevy::log::Level::DEBUG,
                    filter: "info,wgpu=error,naga=warn,hexrails=debug,hexrails::debug=debug".into(),
                    ..Default::default()
                }),
        )
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
        .add_plugins(sprites::AssetPlugin)
        .add_plugins(camera::MovingCameraPlugin)
        .add_plugins(debug::DebugPlugin)
        .add_plugins(railroad::RailRoadPlugin)
        .add_plugins(savegame::LoadSavePlugin)
        .add_plugins(terrain::TerrainPlugin)
        .add_plugins(tilemap::TileMapPlugin)
        .add_plugins(trainbuilder::TrainBuildingPlugin)
        .add_plugins(trains::TrainPlugin)
        .add_plugins(ui::UIOverlayPlugin)
        .add_plugins(input::InputPlugin)
        .add_plugins(interact::InteractPlugin)
        .add_plugins(driving::ManualDrivingPlugin)
        .add_plugins(collisions::CollisionPlugin)
        .run();
}

#[macro_export]
macro_rules! ok_or_tt {
    ($ttt: tt, $x:expr, $err:literal) => {{
        match $x {
            Ok(x) => x,
            Err(e) => {
                error!(concat!($err, ": {err:?}"), err = e);
                $ttt;
            }
        }
    }};
    ($ttt: tt, $x:expr, $err:literal) => {{
        let Ok(x) = $x else {
            $ttt;
        };
        x
    }};
}

#[macro_export]
macro_rules! some_or_tt {
    ($ttt: tt, $x:expr, $err:literal) => {{
        match $x {
            Some(x) => x,
            None => {
                error!($err);
                $ttt;
            }
        }
    }};
    ($ttt: tt, $x:expr) => {{
        let Some(x) = $x else {
            $ttt;
        };
        x
    }};
}

#[macro_export]
macro_rules! ok_or_return {
    ($x:expr, $err:literal) => {
        crate::ok_or_tt!(return, $x, $err)
    };
    ($x:expr) => {
        crate::ok_or_tt!(return, $x)
    };
}

#[macro_export]
macro_rules! some_or_return {
    ($x:expr, $err:literal) => {
        crate::some_or_tt!(return, $x, $err)
    };
    ($x:expr) => {
        crate::some_or_tt!(return, $x)
    };
}

#[macro_export]
macro_rules! ok_or_continue {
    ($x:expr, $err:literal) => {
        crate::ok_or_tt!(continue, $x, $err)
    };
    ($x:expr) => {
        crate::ok_or_tt!(continue, $x)
    };
}

#[macro_export]
macro_rules! some_or_continue {
    ($x:expr, $err:literal) => {
        crate::some_or_tt!(continue, $x, $err)
    };
    ($x:expr) => {
        crate::some_or_tt!(continue, $x)
    };
}

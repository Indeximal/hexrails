use std::f32::consts::PI;

use bevy::{input::mouse::MouseButtonInput, prelude::*};

use crate::camera::current_cursor_world_pos;

pub const TILE_SCALE: f32 = 0.5;
pub const TILE_SIZE: f32 = 128.;
pub const SQRT3_HALF: f32 = 0.866025404;

pub struct TileMapPlugin;
impl Plugin for TileMapPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<TileClickEvent>()
            .add_system(mouse_button_events);
    }
}

pub struct TileClickEvent {
    pub coord: TileCoordinate,
    pub side: TileSide,
    pub button: MouseButton,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileSide {
    Center,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

impl TileSide {
    pub fn from_world_pos(world_pos: Vec2) -> TileSide {
        let tc = TileCoordinate::from(world_pos);
        let tc_center = Vec2::from(tc);
        let diff = world_pos - tc_center;
        if diff.length_squared() <= (TILE_SCALE / 2.) * (TILE_SCALE / 2.) * 0.1 {
            return TileSide::Center;
        }
        let angle = diff.angle_between(Vec2::X);
        if angle.abs() <= PI / 6. {
            TileSide::East
        } else if angle.abs() >= PI * 5. / 6. {
            TileSide::West
        } else if angle <= -PI / 2. {
            TileSide::NorthWest
        } else if angle >= PI / 2. {
            TileSide::SouthWest
        } else if angle >= 0. {
            TileSide::SouthEast
        } else {
            TileSide::NorthEast
        }
    }

    pub fn to_angle(&self) -> f32 {
        match self {
            TileSide::Center => 0.,
            TileSide::East => 0.,
            TileSide::NorthEast => PI * 1. / 3.,
            TileSide::NorthWest => PI * 2. / 3.,
            TileSide::West => PI,
            TileSide::SouthWest => PI * 4. / 3.,
            TileSide::SouthEast => PI * 5. / 3.,
        }
    }

    pub fn opposite(&self) -> TileSide {
        match self {
            TileSide::Center => TileSide::Center,
            TileSide::East => TileSide::West,
            TileSide::NorthEast => TileSide::SouthWest,
            TileSide::NorthWest => TileSide::SouthEast,
            TileSide::West => TileSide::East,
            TileSide::SouthWest => TileSide::NorthEast,
            TileSide::SouthEast => TileSide::NorthWest,
        }
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct TileCoordinate(pub (i32, i32));

impl TileCoordinate {
    fn neighbor(&self, side: TileSide) -> TileCoordinate {
        match side {
            TileSide::Center => TileCoordinate((self.0 .0, self.0 .1)),
            TileSide::East => TileCoordinate((self.0 .0 + 1, self.0 .1)),
            TileSide::West => TileCoordinate((self.0 .0 - 1, self.0 .1)),
            TileSide::NorthEast => TileCoordinate((self.0 .0, self.0 .1 + 1)),
            TileSide::NorthWest => TileCoordinate((self.0 .0 - 1, self.0 .1 + 1)),
            TileSide::SouthEast => TileCoordinate((self.0 .0 + 1, self.0 .1 - 1)),
            TileSide::SouthWest => TileCoordinate((self.0 .0, self.0 .1 - 1)),
        }
    }
}

impl From<TileCoordinate> for Vec2 {
    fn from(tc: TileCoordinate) -> Self {
        let (x, y) = tc.0;
        let yy = y as f32 * TILE_SCALE;
        Vec2::new(
            x as f32 * TILE_SCALE * SQRT3_HALF + SQRT3_HALF / 2. * yy,
            yy * 0.75,
        )
    }
}

impl From<Vec2> for TileCoordinate {
    // This does not currently respect hexagonal boundries
    fn from(world_pos: Vec2) -> Self {
        let yy = world_pos.y / 0.75;
        let x = (world_pos.x - SQRT3_HALF / 2. * yy) / (TILE_SCALE * SQRT3_HALF);
        TileCoordinate((x.round() as i32, (yy / TILE_SCALE).round() as i32))
    }
}

fn mouse_button_events(
    mut mousebtn_evr: EventReader<MouseButtonInput>,
    mut click_event: EventWriter<TileClickEvent>,
    windows: Res<Windows>,
    cam: Query<(&Transform, &OrthographicProjection), With<Camera>>,
) {
    use bevy::input::ElementState;

    let (pos, cam) = cam.single();
    let window = match windows.get_primary() {
        Some(w) => w,
        None => {
            return;
        }
    };
    let world_pos = match current_cursor_world_pos(pos, cam, window) {
        Some(v) => v,
        None => {
            return;
        }
    };

    for ev in mousebtn_evr.iter() {
        match ev.state {
            ElementState::Pressed => {}
            ElementState::Released => click_event.send(TileClickEvent {
                coord: TileCoordinate::from(world_pos),
                side: TileSide::from_world_pos(world_pos),
                button: ev.button,
            }),
        }
    }
}

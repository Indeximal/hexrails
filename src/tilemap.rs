use core::fmt;
use std::f32::consts::PI;

use bevy::prelude::*;
// use bevy_inspector_egui::{bevy_egui::EguiContext, Inspectable};
use serde::{Deserialize, Serialize};

use crate::camera::{current_cursor_world_pos, FlyCamera2d};

pub const TILE_SCALE: f32 = 0.5;
pub const TILE_SIZE: f32 = 128.;
pub const SQRT3_HALF: f32 = 0.866025404;

pub struct TileMapPlugin;
impl Plugin for TileMapPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<TileClickEvent>()
            .add_systems(Update, mouse_button_events);
    }
}

#[derive(Event)]
pub struct TileClickEvent {
    pub coord: TileCoordinate,
    pub side: Option<TileSide>,
    pub button: MouseButton,
}

/// These are the six direction of the hexagonal grid. The value of each direction
/// are equivalent to sixths of a counterclockwise turn starting at the positive x direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TileSide(u8);

impl TileSide {
    pub const EAST: TileSide = TileSide(0);
    pub const NORTH_EAST: TileSide = TileSide(1);
    pub const NORTH_WEST: TileSide = TileSide(2);
    pub const WEST: TileSide = TileSide(3);
    pub const SOUTH_WEST: TileSide = TileSide(4);
    pub const SOUTH_EAST: TileSide = TileSide(5);

    // Todo: make this generic over more number types
    pub fn from_sixth_turns(turns: i8) -> TileSide {
        TileSide((turns % 6) as u8)
    }

    pub fn from_world_pos(world_pos: Vec2) -> Option<TileSide> {
        let tc = TileCoordinate::from_world_pos(world_pos);
        let tc_center = tc.into_world_pos();
        let diff = world_pos - tc_center;
        if diff.length_squared() <= (TILE_SCALE / 2.) * (TILE_SCALE / 2.) * 0.1 {
            return None;
        }
        // angle_between gives a clockwise angle from -PI to PI
        let angle = -diff.angle_between(Vec2::X);
        Some(TileSide::from_sixth_turns(
            ((angle + 2. * PI) / (PI / 3.)).round() as i8,
        ))
    }

    pub fn to_angle(&self) -> f32 {
        self.0 as f32 * PI / 3.
    }

    pub fn opposite(&self) -> TileSide {
        TileSide::from_sixth_turns(self.0 as i8 + 3)
    }

    pub fn curve_right(&self) -> TileSide {
        TileSide::from_sixth_turns(self.0 as i8 + 2)
    }

    pub fn curve_left(&self) -> TileSide {
        TileSide::from_sixth_turns(self.0 as i8 + 4)
    }
}

#[derive(
    Component, Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct TileCoordinate(pub i32, pub i32);

/// This is the face of the tile at coordinate `tile` facing *from* `side`.
/// e.g (0, 0) EAST is the right part of the origin tile, but when used as an
/// coordinate for a rail this rail is going east to west (or south/north-west).
#[derive(Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TileFace {
    pub tile: TileCoordinate,
    pub side: TileSide,
}

impl TileFace {
    /// Returns the TileFace representing the same grid edge, but in the opposite direction
    pub fn opposite(&self) -> TileFace {
        TileFace {
            tile: self.tile.neighbor(self.side),
            side: self.side.opposite(),
        }
    }

    /// Returns the world position of the center of the grid edge that this face represents
    pub fn into_world_position(self) -> Vec2 {
        let origin = self.tile.into_world_pos();
        let angle = self.side.to_angle();
        let offset = Vec2::new(angle.cos(), angle.sin()) * TILE_SCALE / 2. * SQRT3_HALF;
        origin + offset
    }
}

/// This is needed for Egui Inspector
impl Default for TileFace {
    fn default() -> Self {
        Self {
            tile: TileCoordinate(0, 0),
            side: TileSide::EAST,
        }
    }
}

impl fmt::Display for TileCoordinate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.0, self.1)
        // write!(
        //     f,
        //     "{} {}, {} {}",
        //     self.0.abs(),
        //     if self.0 >= 0 { "E" } else { "W" },
        //     self.1.abs(),
        //     if self.1 >= 0 { "NE" } else { "SW" }
        // )
    }
}

impl TileCoordinate {
    pub fn neighbor(&self, side: TileSide) -> TileCoordinate {
        match side {
            TileSide::EAST => TileCoordinate(self.0 + 1, self.1),
            TileSide::WEST => TileCoordinate(self.0 - 1, self.1),
            TileSide::NORTH_EAST => TileCoordinate(self.0, self.1 + 1),
            TileSide::NORTH_WEST => TileCoordinate(self.0 - 1, self.1 + 1),
            TileSide::SOUTH_EAST => TileCoordinate(self.0 + 1, self.1 - 1),
            TileSide::SOUTH_WEST => TileCoordinate(self.0, self.1 - 1),
            _ => panic!("Tile side not in range 0..6"),
        }
    }

    pub fn into_world_pos(&self) -> Vec2 {
        let (x, y) = (self.0, self.1);
        let yy = y as f32 * TILE_SCALE;
        Vec2::new(
            x as f32 * TILE_SCALE * SQRT3_HALF + SQRT3_HALF / 2. * yy,
            yy * 0.75,
        )
    }

    fn nearer_tile(
        tile1: TileCoordinate,
        tile2: TileCoordinate,
        world_pos: Vec2,
    ) -> TileCoordinate {
        if tile1.into_world_pos().distance_squared(world_pos)
            < tile2.into_world_pos().distance_squared(world_pos)
        {
            tile1
        } else {
            tile2
        }
    }

    /// This returns the tile coordinate of tile the world_pos vector is in.
    pub fn from_world_pos(world_pos: Vec2) -> TileCoordinate {
        let yy = world_pos.y / 0.75;
        let x = (world_pos.x - SQRT3_HALF / 2. * yy) / (TILE_SCALE * SQRT3_HALF);
        let y = yy / TILE_SCALE;

        // After the linear transformation only four tiles are possible:
        // two that touch and two to either side of them called near and far here.
        let south_west_tile = TileCoordinate(x.floor() as i32, y.floor() as i32);
        let north_east_tile = TileCoordinate(x.ceil() as i32, y.ceil() as i32);
        let far_diagonal_tile = Self::nearer_tile(south_west_tile, north_east_tile, world_pos);

        let south_tile = TileCoordinate(x.ceil() as i32, y.floor() as i32);
        let north_tile = TileCoordinate(x.floor() as i32, y.ceil() as i32);
        let near_diagonal_tile = Self::nearer_tile(south_tile, north_tile, world_pos);

        Self::nearer_tile(far_diagonal_tile, near_diagonal_tile, world_pos)
    }
}

fn mouse_button_events(
    mousebtn: Res<Input<MouseButton>>,
    mut click_event: EventWriter<TileClickEvent>,
    windows: Query<&Window>,
    cam: Query<(&GlobalTransform, &Camera), With<FlyCamera2d>>,
    // mut egui_context: ResMut<EguiContext>,
    other_buttons: Query<&Interaction>,
) {
    // Skip if the mouse is above a inspector window or other gui
    // if egui_context.ctx_mut().wants_pointer_input() {
    //     return;
    // }
    // todo: fix, this doesn't work yet
    if other_buttons
        .iter()
        .any(|interact| *interact == Interaction::Pressed)
    {
        return;
    }

    let (pos, cam) = cam.single();
    let window = match windows.get_single().ok() {
        Some(w) => w,
        None => {
            return;
        }
    };
    let world_pos = match current_cursor_world_pos(cam, pos, window) {
        Some(v) => v,
        None => {
            return;
        }
    };

    if mousebtn.just_pressed(MouseButton::Left) {
        click_event.send(TileClickEvent {
            coord: TileCoordinate::from_world_pos(world_pos),
            side: TileSide::from_world_pos(world_pos),
            button: MouseButton::Left,
        })
    }
}

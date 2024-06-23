//! Defines the hexagonal tilemap used in the game
//!
//! A "Tile" is a single hexagon.
//! A "Joint" is a sixth of a hexagon, or equivalently an edge with a (perpendicular) orientation.
//! A "Track" is are two joints, which either form a single straight, single left-curving or
//! single right-curving section.

use bevy::prelude::*;
use core::fmt;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// The height (point to point) of the hexagons in world units.
pub const TILE_SCALE: f32 = 0.5;
/// The width (edge to edge) of the hexagons in world units.
pub const TILE_WIDTH: f32 = SQRT3_HALF * TILE_SCALE;
const SQRT3_HALF: f32 = 0.866025404;

pub struct TileMapPlugin;
impl Plugin for TileMapPlugin {
    fn build(&self, _app: &mut App) {
        // Nothing to do here right now. Maybe needs reflection/ type registry?
    }
}

/// A integer vector coordinate for a tile. First coordinate is towards east,
/// the second towards north-east.
#[derive(
    Component, Serialize, Deserialize, Default, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct Tile(pub i32, pub i32);

/// These are the six direction of the hexagonal grid. The value of each direction
/// are equivalent to sixths of a counterclockwise turn starting at the positive x direction.
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct Direction(u8);

/// This is the edge of the tile at coordinate `tile` in the `side` direction.
///
/// E.g (0, 0) EAST is the right part of the origin tile, but when used as an
/// coordinate for a track, this rail is going east to west (or SW/NW).
#[derive(
    Default, Debug, Hash, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct Joint {
    pub tile: Tile,
    pub side: Direction,
}

impl Tile {
    /// This returns the tile coordinate of tile the world_pos vector is in.
    pub fn from_world_pos(world_pos: Vec2) -> Self {
        let yy = world_pos.y / 0.75;
        let x = (world_pos.x - SQRT3_HALF / 2. * yy) / (TILE_SCALE * SQRT3_HALF);
        let y = yy / TILE_SCALE;

        // After the linear transformation only four tiles are possible:
        // two that touch and two to either side of them called near and far here.
        let south_west_tile = Tile(x.floor() as i32, y.floor() as i32);
        let north_east_tile = Tile(x.ceil() as i32, y.ceil() as i32);
        let far_diagonal_tile = Self::nearer_tile(south_west_tile, north_east_tile, world_pos);

        let south_tile = Tile(x.ceil() as i32, y.floor() as i32);
        let north_tile = Tile(x.floor() as i32, y.ceil() as i32);
        let near_diagonal_tile = Self::nearer_tile(south_tile, north_tile, world_pos);

        Self::nearer_tile(far_diagonal_tile, near_diagonal_tile, world_pos)
    }

    /// Returns the center of the tile in world units.
    pub fn world_pos(&self) -> Vec2 {
        let (x, y) = (self.0 as f32, self.1 as f32);
        TILE_SCALE * Vec2::new(x * SQRT3_HALF + SQRT3_HALF / 2. * y, y * 0.75)
    }

    /// Returns the neighboring tile in the direction `dir`
    pub fn neighbor_to(&self, dir: Direction) -> Tile {
        match dir {
            Direction::EAST => Tile(self.0 + 1, self.1),
            Direction::WEST => Tile(self.0 - 1, self.1),
            Direction::NORTH_EAST => Tile(self.0, self.1 + 1),
            Direction::NORTH_WEST => Tile(self.0 - 1, self.1 + 1),
            Direction::SOUTH_EAST => Tile(self.0 + 1, self.1 - 1),
            Direction::SOUTH_WEST => Tile(self.0, self.1 - 1),
            _ => panic!("Tile side not in range 0..6"),
        }
    }

    fn nearer_tile(tile1: Tile, tile2: Tile, world_pos: Vec2) -> Tile {
        if tile1.world_pos().distance_squared(world_pos)
            < tile2.world_pos().distance_squared(world_pos)
        {
            tile1
        } else {
            tile2
        }
    }
}

impl Direction {
    pub const EAST: Direction = Direction(0);
    pub const NORTH_EAST: Direction = Direction(1);
    pub const NORTH_WEST: Direction = Direction(2);
    pub const WEST: Direction = Direction(3);
    pub const SOUTH_WEST: Direction = Direction(4);
    pub const SOUTH_EAST: Direction = Direction(5);

    pub fn from_sixth_turns(turns: i8) -> Direction {
        // Todo: make this generic over more number types
        Direction((turns % 6) as u8)
    }

    /// Returns the angle in radians (counterclockwise, starting from 0 towards +X, to 2*Pi)
    pub fn to_angle(&self) -> f32 {
        self.0 as f32 * PI / 3.
    }

    fn opposite(&self) -> Direction {
        Direction::from_sixth_turns(self.0 as i8 + 3)
    }

    fn curve_right(&self) -> Direction {
        Direction::from_sixth_turns(self.0 as i8 + 2)
    }

    fn curve_left(&self) -> Direction {
        Direction::from_sixth_turns(self.0 as i8 + 4)
    }
}

impl Joint {
    /// Returns the TileFace representing the same edge, but in the opposite direction
    pub fn opposite(&self) -> Self {
        Self {
            tile: self.tile.neighbor_to(self.side),
            side: self.side.opposite(),
        }
    }

    pub fn next_straight(&self) -> Self {
        Self {
            tile: self.tile.neighbor_to(self.side.opposite()),
            side: self.side,
        }
    }
    pub fn next_left(&self) -> Self {
        Self {
            tile: self.tile.neighbor_to(self.side.curve_left()),
            side: self.side.curve_left().opposite(),
        }
    }
    pub fn next_right(&self) -> Self {
        Self {
            tile: self.tile.neighbor_to(self.side.curve_right()),
            side: self.side.curve_right().opposite(),
        }
    }

    /// Returns the world position of the center of the grid edge that this face represents
    pub fn world_position(self) -> Vec2 {
        let origin = self.tile.world_pos();
        let angle = self.side.to_angle();
        let offset = Vec2::new(angle.cos(), angle.sin()) * TILE_SCALE / 2. * SQRT3_HALF;
        origin + offset
    }

    /// Finds the Joint at world position, which is the tile with the sixth of the hexagon
    /// the mouse is in. If it might be ambiguous, then only the tile is returned.
    pub fn from_world_pos(world_pos: Vec2) -> Result<Self, Tile> {
        let tc = Tile::from_world_pos(world_pos);
        let tc_center = tc.world_pos();
        let diff = world_pos - tc_center;
        if diff.length_squared() <= (TILE_SCALE / 2.) * (TILE_SCALE / 2.) * 0.1 {
            return Err(tc);
        }
        // angle_between gives a clockwise angle from -PI to PI
        let angle = -diff.angle_between(Vec2::X);
        Ok(Self {
            tile: tc,
            side: Direction::from_sixth_turns(((angle + 2. * PI) / (PI / 3.)).round() as i8),
        })
    }
}

impl fmt::Debug for Tile {
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

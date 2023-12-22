use crate::sprites::RailSprite;
use crate::sprites::SpriteAtlases;
use crate::tilemap::Joint;
use crate::tilemap::TileClickEvent;
use crate::ui::InteractingState;

use bevy::prelude::*;
use petgraph::graphmap::DiGraphMap;
use serde::{Deserialize, Serialize};

pub struct RailRoadPlugin;
impl Plugin for RailRoadPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, rail_builder.run_if(in_rail_placing_state));
    }
}

#[derive(Component)]
pub struct NetworkRoot;

#[derive(Component)]
pub struct RailMarker;

#[derive(Serialize, Deserialize, Resource)]
pub struct RailGraph {
    /// The underlying directed graph of the rail network.
    ///
    /// A node is a [`Joint`] (an edge in the hex grid with an orientation)
    /// and is connected to other joints that can be reached with a track.
    /// This means every rail has two edges associated with it, one in either direction.
    ///
    /// # Invariants
    /// For all u -> v in G hold:
    /// - v is either 1 tile straight on, or 1 tile 60deg curved in either direction from u.
    /// - v.opposite() -> u.opposite() is also in G.
    pub graph: DiGraphMap<Joint, TrackProperties>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TrackProperties {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Track {
    pub joint: Joint,
    pub heading: TrackType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrackType {
    Straight,
    CurvedLeft,
    CurvedRight,
}

impl Track {
    pub fn from_joints(start: Joint, end: Joint) -> Option<Self> {
        if start.next_left() == end {
            Some(Self {
                joint: start,
                heading: TrackType::CurvedLeft,
            })
        } else if start.next_right() == end {
            Some(Self {
                joint: start,
                heading: TrackType::CurvedRight,
            })
        } else if start.next_straight() == end {
            Some(Self {
                joint: start,
                heading: TrackType::Straight,
            })
        } else {
            None
        }
    }

    pub fn end_joint(&self) -> Joint {
        match self.heading {
            TrackType::Straight => self.joint.next_straight(),
            TrackType::CurvedLeft => self.joint.next_left(),
            TrackType::CurvedRight => self.joint.next_right(),
        }
    }

    pub fn is_canonical_orientation(&self) -> bool {
        use crate::tilemap::Direction as Dir;
        match (self.joint.side, self.heading) {
            (Dir::EAST, _) => true,
            (Dir::NORTH_EAST, _) => true,
            (Dir::NORTH_WEST, TrackType::CurvedLeft) => false, // Special case: connects from E to NW, both canonical
            (Dir::NORTH_WEST, _) => true,
            (Dir::WEST, TrackType::CurvedRight) => true, // Special case: connects from W to SE, neither canonical
            (Dir::WEST, _) => false,
            (Dir::SOUTH_WEST, _) => false,
            (Dir::SOUTH_EAST, _) => false,
            _ => unreachable!("No 7th direction"),
        }
    }
}

impl RailGraph {
    /// Returns true if the track was added, false if it already existed.
    pub fn add_double_track(&mut self, track: Track) -> bool {
        let end_joint = track.end_joint();
        let prev_edge_1 = self
            .graph
            .add_edge(track.joint, end_joint, TrackProperties {});
        let prev_edge_2 = self.graph.add_edge(
            end_joint.opposite(),
            track.joint.opposite(),
            TrackProperties {},
        );
        debug!("Rail built @{} -> {}", track.joint.tile, end_joint.tile);

        // Either both or neither prev edge should have existed
        if prev_edge_1.is_none() != prev_edge_2.is_none() {
            error!("Invariant was broken: track in graph wasn't double track {track:?}");
        }
        prev_edge_1.is_none()
    }
}

fn in_rail_placing_state(state: Res<State<InteractingState>>) -> bool {
    match state.get() {
        InteractingState::PlaceRails(_) => true,
        _ => false,
    }
}

/// This system tries to build rails both in the graph and with sprites when the mouse is clicked.
fn rail_builder(
    mut commands: Commands,
    atlas: Res<SpriteAtlases>,
    mut click_event: EventReader<TileClickEvent>,
    mut rail_graph: ResMut<RailGraph>,
    root_query: Query<Entity, With<NetworkRoot>>,
    state: Res<State<InteractingState>>,
) {
    let rail_graph = rail_graph.as_mut();
    let root_entity = root_query.single();
    let rail_type = match state.get() {
        InteractingState::PlaceRails(v) => v.clone(),
        _ => unreachable!(
            "The run condition should insure that the rail builder is only run in the PlaceRails state!"
        ),
    };

    for evt in click_event.read() {
        if let Some(side) = evt.side {
            match evt.button {
                MouseButton::Left => {
                    let track = Track {
                        joint: Joint {
                            tile: evt.coord,
                            side,
                        },
                        heading: rail_type,
                    };
                    let is_new_track = rail_graph.add_double_track(track);
                    if is_new_track {
                        spawn_rail(&mut commands, &atlas, root_entity, track);
                    }
                }
                _ => (),
            }
        }
    }
}

/// This helper function spawns a rail sprite.
pub fn spawn_rail(
    commands: &mut Commands,
    atlas: &SpriteAtlases,
    root_entity: Entity,
    track: Track,
) {
    let flipped = match track.heading {
        TrackType::Straight => false,
        TrackType::CurvedLeft => true,
        TrackType::CurvedRight => false,
    };
    let mut sprite = atlas.rail_sprite(match track.heading {
        TrackType::Straight => RailSprite::Straight,
        TrackType::CurvedLeft => RailSprite::CurvedRight,
        TrackType::CurvedRight => RailSprite::CurvedRight,
    });
    sprite.sprite.flip_y = flipped;
    sprite.transform.rotate_z(track.joint.side.to_angle());
    sprite.transform.translation += track.joint.tile.world_pos().extend(0.);

    let child = commands
        .spawn(sprite)
        .insert(Name::new(format!("Rail {}", track.joint.tile)))
        .insert(RailMarker)
        .insert(track.joint.tile)
        .id();

    commands.entity(root_entity).add_child(child);
}

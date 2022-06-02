use crate::tilemap::*;
use bevy::prelude::*;

const Z_LAYER_RAILS: f32 = 200.;

pub struct RailRoadPlugin;

impl Plugin for RailRoadPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system_to_stage(StartupStage::PreStartup, load_texture_atlas)
            .add_system(rail_builder);
    }
}

#[derive(Component)]
struct RailTile {}

#[derive(Debug)]
enum RailType {
    Straight,
    CurvedLeft,
    CurvedRight,
}

fn rail_builder(
    mut commands: Commands,
    atlas_h: Res<RailAtlas>,
    mut click_event: EventReader<TileClickEvent>,
) {
    for evt in click_event.iter() {
        match evt.button {
            MouseButton::Left => spawn_rail(
                &mut commands,
                &atlas_h,
                evt.coord,
                evt.side,
                RailType::Straight,
            ),
            MouseButton::Right => spawn_rail(
                &mut commands,
                &atlas_h,
                evt.coord,
                evt.side,
                RailType::CurvedRight,
            ),
            _ => (),
        }
    }
}

fn spawn_rail(
    commands: &mut Commands,
    atlas: &Res<RailAtlas>,
    position: TileCoordinate,
    start_side: TileSide,
    rail_type: RailType,
) {
    let index = match rail_type {
        RailType::Straight => 0,
        RailType::CurvedLeft => 1,
        RailType::CurvedRight => 1,
    };
    let scale_x = match rail_type {
        RailType::Straight => 1.,
        RailType::CurvedLeft => -1.,
        RailType::CurvedRight => 1.,
    };
    let mut sprite = TextureAtlasSprite::new(index);
    sprite.custom_size = Some(Vec2::splat(TILE_SCALE));

    commands
        .spawn_bundle(SpriteSheetBundle {
            sprite: sprite,
            texture_atlas: atlas.0.clone(),
            transform: Transform {
                translation: Vec3::from((position.into(), Z_LAYER_RAILS)),
                rotation: Quat::from_rotation_z(start_side.to_angle()),
                scale: Vec3::from((scale_x, 1., 1.)),
            },
            ..Default::default()
        })
        .insert(RailTile {})
        .insert(position);
}

struct RailAtlas(Handle<TextureAtlas>);

fn load_texture_atlas(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas: ResMut<Assets<TextureAtlas>>,
) {
    let image = asset_server.load("RailAtlas.png");
    let atlas = TextureAtlas::from_grid(image, Vec2::new(TILE_SIZE, TILE_SIZE), 1, 2);
    let atlas_handle = texture_atlas.add(atlas);
    commands.insert_resource(RailAtlas(atlas_handle));
}

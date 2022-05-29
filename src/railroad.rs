use crate::tilemap::*;
use bevy::prelude::*;

pub struct RailRoadPlugin;

impl Plugin for RailRoadPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system_to_stage(StartupStage::PreStartup, load_texture_atlas)
            .add_system(rail_builder);
    }
}

#[derive(Component)]
struct RailTile {}

fn rail_builder(
    mut commands: Commands,
    atlas_h: Res<RailAtlas>,
    mut click_event: EventReader<TileClickEvent>,
) {
    for evt in click_event.iter() {
        spawn_rail(&mut commands, &atlas_h, evt.coord, evt.side);
    }
}

fn spawn_rail(
    commands: &mut Commands,
    atlas: &Res<RailAtlas>,
    position: TileCoordinate,
    start_side: TileSide,
) {
    let index = 0;
    let mut sprite = TextureAtlasSprite::new(index);
    sprite.custom_size = Some(Vec2::splat(TILE_SCALE));

    commands
        .spawn_bundle(SpriteSheetBundle {
            sprite: sprite,
            texture_atlas: atlas.0.clone(),
            transform: Transform {
                translation: Vec3::from((position.into(), 200.)),
                rotation: Quat::from_rotation_z(start_side.to_angle()),
                ..Default::default()
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
    let atlas = TextureAtlas::from_grid(image, Vec2::new(TILE_SIZE, TILE_SIZE), 1, 1);
    let atlas_handle = texture_atlas.add(atlas);
    commands.insert_resource(RailAtlas(atlas_handle));
}

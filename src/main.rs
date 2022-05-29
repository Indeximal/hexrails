use bevy::{prelude::*, render::camera::ScalingMode};

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
            .add_startup_system_to_stage(StartupStage::PreStartup, load_sprite)
            .add_startup_system(print_version)
            .add_startup_system(spawn_camera)
            .add_startup_system(spawn_tiles);
    }
}

#[derive(Component)]
struct TileCoordinate((i32, i32));

impl TileCoordinate {
    fn east(&self) -> TileCoordinate {
        TileCoordinate((self.0 .0 + 1, self.0 .1))
    }

    fn west(&self) -> TileCoordinate {
        TileCoordinate((self.0 .0 - 1, self.0 .1))
    }

    fn north_east(&self) -> TileCoordinate {
        TileCoordinate((self.0 .0, self.0 .1 + 1))
    }

    fn north_west(&self) -> TileCoordinate {
        TileCoordinate((self.0 .0 - 1, self.0 .1 + 1))
    }

    fn south_east(&self) -> TileCoordinate {
        TileCoordinate((self.0 .0 + 1, self.0 .1 - 1))
    }

    fn south_west(&self) -> TileCoordinate {
        TileCoordinate((self.0 .0, self.0 .1 - 1))
    }
}

#[derive(Component)]
struct Tile;

fn spawn_tiles(mut commands: Commands, atlas_h: Res<AtlasHandle>) {
    let mut sprite = TextureAtlasSprite::new(0);
    sprite.custom_size = Some(Vec2::splat(0.5));
    commands
        .spawn_bundle(SpriteSheetBundle {
            sprite: sprite,
            texture_atlas: atlas_h.0.clone(),
            transform: Transform {
                translation: Vec3::new(0.0, 0.0, 100.0),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(Name::new("idk"));

    commands.spawn().insert(Tile).insert(TileCoordinate((0, 0)));
    commands
        .spawn()
        .insert(Tile)
        .insert(TileCoordinate((0, 0)).east());
    commands
        .spawn()
        .insert(Tile)
        .insert(TileCoordinate((0, 0)).west());
}

fn print_version() {
    println!("Build with Bevy!");
}

fn spawn_camera(mut commands: Commands) {
    let mut camera = OrthographicCameraBundle::new_2d();
    camera.orthographic_projection.top = 1.0;
    camera.orthographic_projection.bottom = -1.0;
    camera.orthographic_projection.left = ASPECT_RATIO * -1.0;
    camera.orthographic_projection.right = ASPECT_RATIO * 1.0;
    camera.orthographic_projection.scaling_mode = ScalingMode::FixedVertical;
    commands.spawn_bundle(camera);
}

struct AtlasHandle(Handle<TextureAtlas>);

fn load_sprite(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas: ResMut<Assets<TextureAtlas>>,
) {
    let image = asset_server.load("Grasstile.png");
    let atlas = TextureAtlas::from_grid(image, Vec2::new(444.0, 512.0), 1, 1);
    let atlas_handle = texture_atlas.add(atlas);
    commands.insert_resource(AtlasHandle(atlas_handle));
}

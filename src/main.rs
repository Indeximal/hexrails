use bevy::prelude::*;

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

fn add_tiles(mut commands: Commands) {
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

struct WeirdTimer(Timer);

fn all_coords(
    time: Res<Time>,
    mut timer: ResMut<WeirdTimer>,
    query: Query<&TileCoordinate, With<Tile>>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        for name in query.iter() {
            println!("hello {:?}!", name.0);
        }
    }
}

fn print_version() {
    println!("Build with Bevy!");
}

pub struct HexRailsPlugin;

impl Plugin for HexRailsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WeirdTimer(Timer::from_seconds(2.0, true)))
            .add_startup_system(print_version)
            .add_startup_system(add_tiles)
            .add_system(all_coords);
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(HexRailsPlugin)
        .run();
}

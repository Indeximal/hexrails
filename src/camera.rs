use bevy::{input::mouse::MouseWheel, prelude::*, render::camera::ScalingMode};
// use bevy_inspector_egui::bevy_egui::EguiContext;

use crate::ASPECT_RATIO;

pub struct MovingCameraPlugin;

impl Plugin for MovingCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(spawn_camera)
            .add_system(zoom_system)
            .add_system(camera_2d_movement_system);
    }
}

/// From: https://github.com/mcpar-land/bevy_fly_camera
/// A set of options for initializing a FlyCamera.
/// Attach this component to a [`Camera2dBundle`](https://docs.rs/bevy/0.4.0/bevy/prelude/struct.Camera2dBundle.html)
/// bundle to control it with your keyboard.
/// # Example
/// ```no_compile
/// fn setup(mut commands: Commands) {
///	  commands
///     .spawn(Camera2dBundle::default())
///     .with(FlyCamera2d::default());
/// }
#[derive(Component)]
pub struct FlyCamera2d {
    /// The speed the FlyCamera2d accelerates at.
    pub accel: f32,
    /// The maximum speed the FlyCamera can move at.
    pub max_speed: f32,
    /// The amount of deceleration to apply to the camera's motion.
    pub friction: f32,
    /// The current velocity of the FlyCamera2d. This value is always up-to-date, enforced by [FlyCameraPlugin](struct.FlyCameraPlugin.html)
    pub velocity: Vec2,

    pub zoom_speed: f32,
    /// keep above 1.0 to avoid having to deal with far clip plane issues
    pub min_zoom: f32,
    pub max_zoom: f32,

    /// Key used to move left. Defaults to <kbd>A</kbd>
    pub key_left: KeyCode,
    /// Key used to move right. Defaults to <kbd>D</kbd>
    pub key_right: KeyCode,
    /// Key used to move up. Defaults to <kbd>W</kbd>
    pub key_up: KeyCode,
    /// Key used to move forward. Defaults to <kbd>S</kbd>
    pub key_down: KeyCode,
    /// If `false`, disable keyboard control of the camera. Defaults to `true`
    pub enabled: bool,
}

fn spawn_camera(mut commands: Commands) {
    let mut camera = OrthographicCameraBundle::new_2d();

    camera.orthographic_projection.top = 1.0;
    camera.orthographic_projection.bottom = -1.0;
    camera.orthographic_projection.left = ASPECT_RATIO * -1.0;
    camera.orthographic_projection.right = ASPECT_RATIO * 1.0;
    camera.orthographic_projection.scale = 3.0;
    camera.orthographic_projection.scaling_mode = ScalingMode::FixedVertical;
    commands.spawn_bundle(camera).insert(FlyCamera2d::default());
}

impl Default for FlyCamera2d {
    fn default() -> Self {
        const MUL_2D: f32 = 0.2;
        Self {
            accel: 15.0 * MUL_2D,
            max_speed: 0.5 * MUL_2D,
            friction: 5.0 * MUL_2D,
            velocity: Vec2::ZERO,
            zoom_speed: 0.7,
            min_zoom: 1.0,
            max_zoom: 12.0,
            key_left: KeyCode::A,
            key_right: KeyCode::D,
            key_up: KeyCode::W,
            key_down: KeyCode::S,
            enabled: true,
        }
    }
}

pub fn current_cursor_world_pos(
    cam_pos: &Transform,
    proj: &OrthographicProjection,
    window: &Window,
) -> Option<Vec2> {
    let window_size = Vec2::new(window.width(), window.height());
    let window_pos = window.cursor_position()?;
    let mouse_normalized_screen_pos = (window_pos / window_size) * 2. - Vec2::ONE;
    let mouse_world_pos = cam_pos.translation.truncate()
        + mouse_normalized_screen_pos * Vec2::new(proj.right, proj.top) * proj.scale;
    Some(mouse_world_pos)
}

fn movement_axis(input: &Res<Input<KeyCode>>, plus: KeyCode, minus: KeyCode) -> f32 {
    let mut axis = 0.0;
    if input.pressed(plus) {
        axis += 1.0;
    }
    if input.pressed(minus) {
        axis -= 1.0;
    }
    axis
}

fn camera_2d_movement_system(
    time: Res<Time>,
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<(&mut FlyCamera2d, &mut Transform)>,
) {
    for (mut options, mut transform) in query.iter_mut() {
        let (axis_h, axis_v) = if options.enabled {
            (
                movement_axis(&keyboard_input, options.key_right, options.key_left),
                movement_axis(&keyboard_input, options.key_up, options.key_down),
            )
        } else {
            (0.0, 0.0)
        };

        let accel: Vec2 = (Vec2::X * axis_h) + (Vec2::Y * axis_v);
        let accel: Vec2 = if accel.length() != 0.0 {
            accel.normalize() * options.accel
        } else {
            Vec2::ZERO
        };

        let friction: Vec2 = if options.velocity.length() != 0.0 {
            options.velocity.normalize() * -1.0 * options.friction
        } else {
            Vec2::ZERO
        };

        options.velocity += accel * time.delta_seconds();

        // clamp within max speed
        if options.velocity.length() > options.max_speed {
            options.velocity = options.velocity.normalize() * options.max_speed;
        }

        let delta_friction = friction * time.delta_seconds();

        options.velocity =
            if (options.velocity + delta_friction).signum() != options.velocity.signum() {
                Vec2::ZERO
            } else {
                options.velocity + delta_friction
            };

        transform.translation += Vec3::new(options.velocity.x, options.velocity.y, 0.0);
    }
}

/// From https://github.com/bevyengine/bevy/issues/2580
fn zoom_system(
    mut whl: EventReader<MouseWheel>,
    mut cam: Query<(&FlyCamera2d, &mut Transform, &mut OrthographicProjection), With<Camera>>,
    windows: Res<Windows>,
    // mut egui_context: ResMut<EguiContext>,
) {
    // Skip zooming if the mouse is above a inspector window
    // if egui_context.ctx_mut().wants_pointer_input() {
    //     return;
    // }

    let delta_zoom: f32 = whl.iter().map(|e| e.y).sum();
    if delta_zoom == 0. {
        return;
    }

    let (options, mut pos, mut cam) = cam.single_mut();

    // Minor code duplication with the current_cursor_world_pos function
    let window = windows.get_primary().unwrap();
    let window_size = Vec2::new(window.width(), window.height());
    let mouse_normalized_screen_pos =
        (window.cursor_position().unwrap() / window_size) * 2. - Vec2::ONE;
    let mouse_world_pos = pos.translation.truncate()
        + mouse_normalized_screen_pos * Vec2::new(cam.right, cam.top) * cam.scale;

    cam.scale -= options.zoom_speed * delta_zoom * cam.scale;
    cam.scale = cam.scale.clamp(options.min_zoom, options.max_zoom);

    pos.translation = (mouse_world_pos
        - mouse_normalized_screen_pos * Vec2::new(cam.right, cam.top) * cam.scale)
        .extend(pos.translation.z);
}

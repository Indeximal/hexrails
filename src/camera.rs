//! This module provides a draggable, zoomable, keyboard moveable camera.
//!
//! Mostly copy pasted code + using [`bevy_pancam`] for the zoom and drag functionality.
//!
//! <kbd>W</kbd>, <kbd>A</kbd>, <kbd>S</kbd>, <kbd>D</kbd> or <kbd>MMB</kbd>/<kbd>RMB</kbd> to move the camera.
//! Scroll to zoom in/out.

use bevy::prelude::*;
use bevy::render::camera::ScalingMode;
use bevy_pancam::{PanCam, PanCamPlugin};
// use bevy_inspector_egui::bevy_egui::EguiContext;

use crate::ASPECT_RATIO;

pub struct MovingCameraPlugin;

impl Plugin for MovingCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PanCamPlugin)
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, camera_2d_movement_system);
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
    accel: f32,
    /// The maximum speed the FlyCamera can move at.
    max_speed: f32,
    /// The amount of deceleration to apply to the camera's motion.
    friction: f32,
    /// The current velocity of the FlyCamera2d. This value is always up-to-date, enforced by [FlyCameraPlugin](struct.FlyCameraPlugin.html)
    velocity: Vec2,

    /// Key used to move left. Defaults to <kbd>A</kbd>
    key_left: KeyCode,
    /// Key used to move right. Defaults to <kbd>D</kbd>
    key_right: KeyCode,
    /// Key used to move up. Defaults to <kbd>W</kbd>
    key_up: KeyCode,
    /// Key used to move forward. Defaults to <kbd>S</kbd>
    key_down: KeyCode,
    /// If `false`, disable keyboard control of the camera. Defaults to `true`
    enabled: bool,
}

fn spawn_camera(mut commands: Commands) {
    // allow Z layer between 0 and 1
    let mut camera = Camera2dBundle::new_with_far(2.0);

    camera.projection.scale = 3.0;
    camera.projection.scaling_mode = ScalingMode::Fixed {
        width: 2.0 * ASPECT_RATIO,
        height: 2.0,
    };
    commands
        .spawn(camera)
        .insert(FlyCamera2d::default())
        .insert(PanCam {
            grab_buttons: vec![MouseButton::Right, MouseButton::Middle],
            enabled: true,
            zoom_to_cursor: true,
            min_scale: 0.5,
            max_scale: Some(12.0),
            ..Default::default()
        });
}

impl Default for FlyCamera2d {
    fn default() -> Self {
        const MUL_2D: f32 = 0.2;
        Self {
            accel: 15.0 * MUL_2D,
            max_speed: 0.5 * MUL_2D,
            friction: 5.0 * MUL_2D,
            velocity: Vec2::ZERO,
            key_left: KeyCode::A,
            key_right: KeyCode::D,
            key_up: KeyCode::W,
            key_down: KeyCode::S,
            enabled: true,
        }
    }
}

pub fn current_cursor_world_pos(
    cam: &Camera,
    cam_pos: &GlobalTransform,
    window: &Window,
) -> Option<Vec2> {
    cam.viewport_to_world_2d(cam_pos, window.cursor_position()?)
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

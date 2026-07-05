//! This module defines states, actions in these states, and transitions between them.
//!
//! The main state is [`MenuState`].

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use crate::{railroad::TrackType, trains::VehicleType};

pub struct InputPlugin;
impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        MenuAction::init(app);
        SpawnAction::init(app);
        BuildAction::init(app);
        DriveAction::init(app);
        CameraAction::init(app);
    }
}

macro_rules! transition_system {
    (<$A:ty, $S:ty> $($action:expr => $state:expr,)+) => {
        |action: Single<&ActionState<$A>>, mut next_state: ResMut<NextState<$S>>| {
            $(
                if action.just_pressed(&$action) {
                    next_state.set($state);
                }
            )+
        }
    };
}

trait Subaction: Actionlike + bevy::reflect::GetTypeRegistration {
    fn make_input_map() -> InputMap<Self>;
    fn init(app: &mut App) {
        app.add_plugins(InputManagerPlugin::<Self>::default());
        app.world_mut().spawn(Self::make_input_map());
        Self::additional_init(app);
    }
    fn additional_init(_app: &mut App) {}
    /// Only enables this action set's [`ActionState`] while `superstate` is active,
    /// starting disabled.
    fn toggle_with<S: States>(app: &mut App, superstate: S) {
        let mut query = app.world_mut().query::<&mut ActionState<Self>>();
        for mut state in query.iter_mut(app.world_mut()) {
            state.disable();
        }
        app.add_systems(
            OnEnter(superstate.clone()),
            |mut q: Query<&mut ActionState<Self>>| {
                for mut state in &mut q {
                    state.enable();
                }
            },
        )
        .add_systems(OnExit(superstate), |mut q: Query<&mut ActionState<Self>>| {
            for mut state in &mut q {
                state.disable();
            }
        });
    }
}

// region -- Camera
pub type CameraInput = ActionState<CameraAction>;

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum CameraAction {
    #[actionlike(DualAxis)]
    Move,
    // FIXME: this isn't actually used, defined again in `spawn_camera`, but not easy to fix
    Pan,
}

impl Subaction for CameraAction {
    fn make_input_map() -> InputMap<Self> {
        InputMap::default()
            .with_dual_axis(Self::Move, VirtualDPad::wasd())
            .with(Self::Pan, MouseButton::Left)
    }
}
// endregion -- Camera

// region -- Menu
pub type MenuInput = ActionState<MenuAction>;

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum MenuAction {
    // States
    Drive,
    BuildTracks,
    SpawnVehicles,
    // Savegame
    Reload,
    NewGame,
    Save,
    // Debug
    Help,
    ToggleGizmos,
}

#[derive(States, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuState {
    #[default]
    Driving,
    Building,
    Spawning,
}

#[derive(States, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DebugGizmosState {
    #[default]
    Disabled,
    Enabled,
}

impl Subaction for MenuAction {
    fn make_input_map() -> InputMap<Self> {
        InputMap::default()
            .with(Self::Drive, KeyCode::Space)
            .with(Self::BuildTracks, KeyCode::KeyT)
            .with(Self::SpawnVehicles, KeyCode::KeyV)
            .with(Self::Reload, KeyCode::F5)
            .with(Self::NewGame, KeyCode::F7)
            .with(Self::Save, KeyCode::F6)
            .with(Self::Help, KeyCode::F1)
            .with(Self::ToggleGizmos, KeyCode::F2)
    }

    fn additional_init(app: &mut App) {
        app.init_state::<MenuState>()
            .add_systems(
                Update,
                transition_system!(
                    <MenuAction, MenuState>
                    MenuAction::Drive => MenuState::Driving,
                    MenuAction::BuildTracks => MenuState::Building,
                    MenuAction::SpawnVehicles => MenuState::Spawning,
                ),
            )
            .init_state::<DebugGizmosState>()
            .add_systems(
                Update,
                |action: Single<&ActionState<MenuAction>>,
                 state: Res<State<DebugGizmosState>>,
                 mut next_state: ResMut<NextState<DebugGizmosState>>| {
                    if action.just_pressed(&MenuAction::ToggleGizmos) {
                        match state.get() {
                            DebugGizmosState::Disabled => next_state.set(DebugGizmosState::Enabled),
                            DebugGizmosState::Enabled => next_state.set(DebugGizmosState::Disabled),
                        };
                    }
                },
            );
    }
}
// endregion -- Menu

// region - Driving
pub type DriveInput = ActionState<DriveAction>;

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum DriveAction {
    SelectTrain,
    // Actions
    Couple,
    Uncouple,
    Accelerate,
    Brake,
    Reverse,
    #[actionlike(Axis)]
    SwitchDirection,
}

impl Subaction for DriveAction {
    fn make_input_map() -> InputMap<Self> {
        InputMap::default()
            .with(Self::SelectTrain, MouseButton::Right)
            .with(Self::Couple, KeyCode::KeyC)
            .with(Self::Uncouple, KeyCode::KeyX)
            .with(Self::Accelerate, KeyCode::ArrowUp)
            .with(Self::Brake, KeyCode::ArrowDown)
            .with(Self::Reverse, KeyCode::KeyR)
            .with_axis(Self::SwitchDirection, VirtualAxis::horizontal_arrow_keys())
    }
    fn additional_init(app: &mut App) {
        Self::toggle_with(app, MenuState::Driving)
    }
}
// endregion -- Driving

// region -- Build
pub type BuildInput = ActionState<BuildAction>;

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum BuildAction {
    Build,
    // Substates
    SelectLeft,
    SelectStraight,
    SelectRight,
}

#[derive(States, Clone, PartialEq, Eq, Hash, Debug)]
pub enum BuildingState {
    LayTrack(TrackType),
}

impl Default for BuildingState {
    fn default() -> Self {
        Self::LayTrack(TrackType::Straight)
    }
}

impl Subaction for BuildAction {
    fn make_input_map() -> InputMap<Self> {
        InputMap::default()
            .with(Self::Build, MouseButton::Right)
            .with(Self::SelectLeft, KeyCode::Digit1)
            .with(Self::SelectStraight, KeyCode::Digit2)
            .with(Self::SelectRight, KeyCode::Digit3)
    }

    fn additional_init(app: &mut App) {
        app.init_state::<BuildingState>().add_systems(
            Update,
            transition_system!(
                <BuildAction, BuildingState>
                BuildAction::SelectLeft => BuildingState::LayTrack(TrackType::CurvedLeft),
                BuildAction::SelectStraight => BuildingState::LayTrack(TrackType::Straight),
                BuildAction::SelectRight => BuildingState::LayTrack(TrackType::CurvedRight),
            ),
        );
        Self::toggle_with(app, MenuState::Building);
    }
}
// endregion -- Build

// region -- Vehicle spawning
pub type SpawnInput = ActionState<SpawnAction>;

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum SpawnAction {
    Spawn,
    // Substates
    SelectEngine,
    SelectBoxcar,
    SelectDespawn,
    SelectRerail,
}

#[derive(States, Clone, PartialEq, Eq, Hash, Debug)]
pub enum SpawningState {
    SpawnVehicle(VehicleType),
    Despawn,
    Rerail,
}

impl Default for SpawningState {
    fn default() -> Self {
        Self::SpawnVehicle(VehicleType::Locomotive)
    }
}

impl Subaction for SpawnAction {
    fn make_input_map() -> InputMap<Self> {
        InputMap::default()
            .with(Self::Spawn, MouseButton::Right)
            .with(Self::SelectEngine, KeyCode::Digit1)
            .with(Self::SelectBoxcar, KeyCode::Digit2)
            .with(Self::SelectRerail, KeyCode::Digit3)
            .with(Self::SelectDespawn, KeyCode::Digit4)
    }

    fn additional_init(app: &mut App) {
        app.init_state::<SpawningState>().add_systems(
            Update,
            transition_system!(
                <SpawnAction, SpawningState>
                SpawnAction::SelectEngine => SpawningState::SpawnVehicle(VehicleType::Locomotive),
                SpawnAction::SelectBoxcar => SpawningState::SpawnVehicle(VehicleType::Wagon),
                SpawnAction::SelectDespawn => SpawningState::Despawn,
                SpawnAction::SelectRerail => SpawningState::Rerail,
            ),
        );
        Self::toggle_with(app, MenuState::Spawning);
    }
}
// endregion -- Vehicle spawning

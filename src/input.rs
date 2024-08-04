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
        |action: Res<ActionState<$A>>, mut next_state: ResMut<NextState<$S>>| {
            $(
                if action.just_pressed(&$action) {
                    next_state.set($state);
                }
            )+
        }
    };
}

trait Subaction: Actionlike {
    fn make_input_map() -> InputMap<Self>;
    fn init(app: &mut App) {
        app.add_plugins(InputManagerPlugin::<Self>::default())
            .init_resource::<ActionState<Self>>()
            .insert_resource(Self::make_input_map());
        Self::additional_init(app);
    }
    fn additional_init(_app: &mut App) {}
    fn toggle_with<S: States>(app: &mut App, superstate: S) {
        app.insert_resource(<ToggleActions<Self>>::DISABLED)
            .add_systems(
                OnEnter(superstate.clone()),
                |mut toggle: ResMut<ToggleActions<Self>>| {
                    toggle.enabled = true;
                },
            )
            .add_systems(
                OnExit(superstate),
                |mut toggle: ResMut<ToggleActions<Self>>| {
                    toggle.enabled = false;
                },
            );
    }
}

// region -- Camera
pub type CameraInput = ActionState<CameraAction>;

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum CameraAction {
    Move,
    // FIXME: this isn't actually used, defined again in `spawn_camera`, but not easy to fix
    Pan,
}

impl Subaction for CameraAction {
    fn make_input_map() -> InputMap<Self> {
        use InputKind::Mouse;
        use UserInput::{Single, VirtualDPad as VirtualDPadW};
        InputMap::new([
            (Self::Move, VirtualDPadW(VirtualDPad::wasd())),
            (Self::Pan, Single(Mouse(MouseButton::Left))),
        ])
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
        use InputKind::PhysicalKey;
        use UserInput::Single;
        InputMap::new([
            (Self::Drive, Single(PhysicalKey(KeyCode::Space))),
            (Self::BuildTracks, Single(PhysicalKey(KeyCode::KeyT))),
            (Self::SpawnVehicles, Single(PhysicalKey(KeyCode::KeyV))),
            (Self::Reload, Single(PhysicalKey(KeyCode::F5))),
            (Self::NewGame, Single(PhysicalKey(KeyCode::F7))),
            (Self::Save, Single(PhysicalKey(KeyCode::F6))),
            (Self::Help, Single(PhysicalKey(KeyCode::F1))),
            (Self::ToggleGizmos, Single(PhysicalKey(KeyCode::F2))),
        ])
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
                |action: Res<ActionState<MenuAction>>,
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
    SwitchDirection,
}

impl Subaction for DriveAction {
    fn make_input_map() -> InputMap<Self> {
        use InputKind::{Mouse, PhysicalKey};
        use UserInput::Single;
        InputMap::new([
            (Self::SelectTrain, Single(Mouse(MouseButton::Right))),
            (Self::Couple, Single(PhysicalKey(KeyCode::KeyC))),
            (Self::Uncouple, Single(PhysicalKey(KeyCode::KeyX))),
            (Self::Accelerate, Single(PhysicalKey(KeyCode::ArrowUp))),
            (Self::Brake, Single(PhysicalKey(KeyCode::ArrowDown))),
            (Self::Reverse, Single(PhysicalKey(KeyCode::KeyR))),
            (
                Self::SwitchDirection,
                UserInput::VirtualAxis(VirtualAxis::horizontal_arrow_keys()),
            ),
        ])
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
        use InputKind::{Mouse, PhysicalKey};
        use UserInput::Single;
        InputMap::new([
            (Self::Build, Single(Mouse(MouseButton::Right))),
            (Self::SelectLeft, Single(PhysicalKey(KeyCode::Digit1))),
            (Self::SelectStraight, Single(PhysicalKey(KeyCode::Digit2))),
            (Self::SelectRight, Single(PhysicalKey(KeyCode::Digit3))),
        ])
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
        use InputKind::{Mouse, PhysicalKey};
        use UserInput::Single;
        InputMap::new([
            (Self::Spawn, Single(Mouse(MouseButton::Right))),
            (Self::SelectEngine, Single(PhysicalKey(KeyCode::Digit1))),
            (Self::SelectBoxcar, Single(PhysicalKey(KeyCode::Digit2))),
            (Self::SelectRerail, Single(PhysicalKey(KeyCode::Digit3))),
            (Self::SelectDespawn, Single(PhysicalKey(KeyCode::Digit4))),
        ])
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

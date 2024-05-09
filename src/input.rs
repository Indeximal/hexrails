use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

pub type GameInput = ActionState<Action>;

pub struct InputPlugin;
impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<Action>::default())
            .init_resource::<ActionState<Action>>()
            .insert_resource(Action::make_input_map());
    }
}

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum Action {
    // Camera
    CameraMove,
    // Savegame
    Load,
    LoadNew,
    Save,
    // Building
    Build,
    // Train Control (axis handled in application logic)
    Accelerate,
    Brake,
    Reverse,
    SwitchDirection,
}

impl Action {
    fn make_input_map() -> InputMap<Action> {
        use InputKind::{Mouse, PhysicalKey};
        use UserInput::{Single, VirtualDPad as VirtualDPadW};
        InputMap::new([
            (Self::CameraMove, VirtualDPadW(VirtualDPad::wasd())),
            (Self::Load, Single(PhysicalKey(KeyCode::F5))),
            (Self::LoadNew, Single(PhysicalKey(KeyCode::F7))),
            (Self::Save, Single(PhysicalKey(KeyCode::F6))),
            (Self::Build, Single(Mouse(MouseButton::Left))),
            (Self::Accelerate, Single(PhysicalKey(KeyCode::ArrowUp))),
            (Self::Brake, Single(PhysicalKey(KeyCode::ArrowDown))),
            (Self::Reverse, Single(PhysicalKey(KeyCode::KeyR))),
            (
                Self::SwitchDirection,
                UserInput::VirtualAxis(VirtualAxis::horizontal_arrow_keys()),
            ),
        ])
    }
}

use bevy::{prelude::*, ui::FocusPolicy};

use crate::{railroad::RailType, trains::TrainUnitType};

pub struct UIOverlayPlugin;

impl Plugin for UIOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_state(InteractingState::None)
            .add_startup_system(build_ui)
            .add_system(button_system)
            .add_system(button_hightlighting);
    }
}

#[derive(Component)]
pub struct InteractingStateTarget(InteractingState);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InteractingState {
    None,
    PlaceRails(RailType),
    PlaceTrains(TrainUnitType),
    SelectTrain,
}

fn button_hightlighting(
    mut button_query: Query<(&mut BackgroundColor, &InteractingStateTarget), With<Button>>,
    state: Res<State<InteractingState>>,
) {
    // Todo: this seems bad, maybe change?
    if !state.is_changed() {
        return;
    }
    for (mut color, target_state) in button_query.iter_mut() {
        *color = if target_state.0 == *state.current() {
            Color::rgb(0.8, 0.8, 0.8)
        } else {
            Color::WHITE
        }
        .into();
    }
}

fn button_system(
    interaction_query: Query<
        (&Interaction, &InteractingStateTarget),
        (Changed<Interaction>, With<Button>),
    >,
    mut state: ResMut<State<InteractingState>>,
) {
    let current_state = state.as_ref().current().clone();
    for (interaction, target_state) in interaction_query.iter() {
        match *interaction {
            Interaction::Clicked => {
                if current_state == target_state.0 {
                    state
                        .set(InteractingState::None)
                        .expect("Coundn't clear interacting state!");
                } else {
                    state
                        .set(target_state.0)
                        .expect("Coundn't set interacting state!");
                }
            }
            _ => {}
        };
    }
}

/// System to create the UI on startup
fn build_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
    let button_style = Style {
        size: Size::new(Val::Percent(100.), Val::Percent(100.)),
        ..default()
    };

    commands
        .spawn(NodeBundle {
            style: Style {
                size: Size::new(Val::Percent(50.), Val::Percent(12.)),
                margin: UiRect {
                    left: Val::Auto,
                    right: Val::Auto,
                    top: Val::Auto,
                    bottom: Val::Px(20.),
                },
                justify_content: JustifyContent::SpaceAround,
                ..default()
            },
            ..default()
        })
        .insert(Name::new("UI"))
        .with_children(|parent| {
            for (target_state, icon) in vec![
                (
                    InteractingState::PlaceRails(RailType::Straight),
                    "ui/icon_rail_straight.png",
                ),
                (
                    InteractingState::PlaceRails(RailType::CurvedRight),
                    "ui/icon_rail_right.png",
                ),
                (
                    InteractingState::PlaceRails(RailType::CurvedLeft),
                    "ui/icon_rail_left.png",
                ),
                (
                    InteractingState::PlaceTrains(TrainUnitType::Locomotive),
                    "ui/icon_locomotive.png",
                ),
                (
                    InteractingState::PlaceTrains(TrainUnitType::Wagon),
                    "ui/icon_wagon.png",
                ),
                (InteractingState::SelectTrain, "ui/icon_drive.png"),
            ] {
                parent
                    .spawn(ButtonBundle {
                        style: button_style.clone(),
                        image: asset_server.load(icon).into(),
                        focus_policy: FocusPolicy::Block,
                        ..default()
                    })
                    .insert(Name::new(format!("Button {:?}", target_state)))
                    .insert(InteractingStateTarget(target_state));
            }
        });
}

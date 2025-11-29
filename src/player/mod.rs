use bevy::prelude::*;

use crate::animations_utils::LinkAnimationPlayerPluginFor;
use crate::assets::MyStates;
use crate::player::animations::*;
use crate::player::controller::*;

pub mod animations;
pub mod controller;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LinkAnimationPlayerPluginFor::<PlayerRoot>::default());
        app.add_observer(on_player_spawn);
        app.add_observer(on_animation_player_loaded);
        app.add_systems(
            Update,
            (apply_controls, rotate_character_to_camera).run_if(in_state(MyStates::Next)),
        );
        app.add_systems(
            Update,
            (
                take_controller_snapshot,
                update_animation_state,
                update_animation_weights,
                apply_animation_weights,
            )
                .chain()
                .run_if(in_state(MyStates::Next)),
        );
    }
}

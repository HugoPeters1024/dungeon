use bevy::prelude::*;

use crate::animations_utils::LinkAnimationsPluginFor;
use crate::assets::MyStates;
use crate::player::controller::*;

pub mod controller;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LinkAnimationsPluginFor::<PlayerRoot>::default());
        app.add_observer(on_player_spawn);
        app.add_systems(
            Update,
            (
                setup_animation,
                update_animation_weights,
                update_animation_state,
                apply_controls,
                rotate_character_to_camera,
            )
                .run_if(in_state(MyStates::Next)),
        );
    }
}

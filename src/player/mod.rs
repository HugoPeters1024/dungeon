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
        app.add_observer(put_in_hand);
        app.add_systems(
            Update,
            (rotate_character_to_camera).run_if(in_state(MyStates::Next)),
        );
        app.add_systems(
            Update,
            (
                controller_update_sensors,
                update_controller_state,
                pickup_stuff,
                apply_controls,
                animations_from_controller,
                apply_animation_weights,
            )
                .chain()
                .run_if(in_state(MyStates::Next)),
        );
        app.add_systems(
            Update,
            cleanup_pickup_particles.run_if(in_state(MyStates::Next)),
        );
    }
}

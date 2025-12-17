pub mod controller;

pub use controller::*;

use bevy::prelude::*;

/// Plugin for third-person camera system
pub struct ThirdPersonCameraPlugin;

impl Plugin for ThirdPersonCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                controller::handle_mouse_look,
                controller::update_camera_position,
            ),
        );
    }
}

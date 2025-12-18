pub mod controller;

pub use controller::*;

use bevy::prelude::*;

/// Plugin for third-person camera system
pub struct ThirdPersonCameraPlugin;

impl Plugin for ThirdPersonCameraPlugin {
    fn build(&self, app: &mut App) {
        // Mouse input should be handled in Update for responsiveness
        app.add_systems(Update, controller::handle_mouse_look);
        // Camera position updates should run in FixedUpdate to align with physics
        // This prevents jitter when jumping or on moving platforms
        app.add_systems(FixedUpdate, controller::update_camera_position);
    }
}

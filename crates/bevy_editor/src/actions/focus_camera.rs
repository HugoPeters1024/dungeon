use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::EditorCamera;

use super::{Action, UndoFn};

/// Focus the editor camera on a specific world position
#[derive(Clone, Debug)]
pub struct FocusCameraAction {
    pub old_position: Vec3,
    pub new_position: Vec3,
}

impl Action for FocusCameraAction {
    fn apply(&self, world: &mut World) -> UndoFn {
        let mut query = world.query_filtered::<&mut PanOrbitCamera, With<EditorCamera>>();
        for mut pan_orbit in query.iter_mut(world) {
            pan_orbit.target_focus = self.new_position;
        }

        let old_position = self.old_position;
        Box::new(move |world: &mut World| {
            let mut query = world.query_filtered::<&mut PanOrbitCamera, With<EditorCamera>>();
            for mut pan_orbit in query.iter_mut(world) {
                pan_orbit.target_focus = old_position;
            }
        })
    }

    fn name(&self) -> String {
        "focus camera".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_camera_action_name() {
        let action = FocusCameraAction {
            old_position: Vec3::ZERO,
            new_position: Vec3::ONE,
        };
        assert_eq!(action.name(), "focus camera");
    }
}

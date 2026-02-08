use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::EditorCamera;

use super::Action;

/// Focus the editor camera on a specific world position
#[derive(Clone, Debug)]
pub struct FocusCameraAction {
    pub old_position: Vec3,
    pub new_position: Vec3,
}

impl Action for FocusCameraAction {
    fn apply(&mut self, world: &mut World) {
        let mut query = world.query_filtered::<&mut PanOrbitCamera, With<EditorCamera>>();
        for mut pan_orbit in query.iter_mut(world) {
            pan_orbit.target_focus = self.new_position;
        }
    }

    fn revert(&mut self, world: &mut World) {
        let mut query = world.query_filtered::<&mut PanOrbitCamera, With<EditorCamera>>();
        for mut pan_orbit in query.iter_mut(world) {
            pan_orbit.target_focus = self.old_position;
        }
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

    #[test]
    fn test_focus_camera_applies_to_editor_camera() {
        let mut world = World::new();

        // Spawn an editor camera with PanOrbitCamera
        world.spawn((
            EditorCamera,
            PanOrbitCamera {
                target_focus: Vec3::ZERO,
                ..default()
            },
        ));

        let mut action = FocusCameraAction {
            old_position: Vec3::ZERO,
            new_position: Vec3::new(10.0, 20.0, 30.0),
        };

        action.apply(&mut world);

        let mut query = world.query_filtered::<&PanOrbitCamera, With<EditorCamera>>();
        let camera = query.single(&world).unwrap();
        assert_eq!(camera.target_focus, Vec3::new(10.0, 20.0, 30.0));
    }

    #[test]
    fn test_focus_camera_undo_restores_position() {
        let mut world = World::new();

        world.spawn((
            EditorCamera,
            PanOrbitCamera {
                target_focus: Vec3::new(5.0, 5.0, 5.0),
                ..default()
            },
        ));

        let mut action = FocusCameraAction {
            old_position: Vec3::new(5.0, 5.0, 5.0),
            new_position: Vec3::new(10.0, 20.0, 30.0),
        };

        action.apply(&mut world);

        // Verify it changed
        {
            let mut query = world.query_filtered::<&PanOrbitCamera, With<EditorCamera>>();
            let camera = query.single(&world).unwrap();
            assert_eq!(camera.target_focus, Vec3::new(10.0, 20.0, 30.0));
        }

        action.revert(&mut world);

        // Verify it restored
        let mut query = world.query_filtered::<&PanOrbitCamera, With<EditorCamera>>();
        let camera = query.single(&world).unwrap();
        assert_eq!(camera.target_focus, Vec3::new(5.0, 5.0, 5.0));
    }

    #[test]
    fn test_focus_camera_no_editor_camera_doesnt_panic() {
        let mut world = World::new();

        // No editor camera in the world
        let mut action = FocusCameraAction {
            old_position: Vec3::ZERO,
            new_position: Vec3::ONE,
        };

        action.apply(&mut world);
        action.revert(&mut world); // Should not panic
    }

    #[test]
    fn test_focus_camera_multiple_cameras() {
        let mut world = World::new();

        // Spawn multiple editor cameras
        world.spawn((
            EditorCamera,
            PanOrbitCamera {
                target_focus: Vec3::ZERO,
                ..default()
            },
        ));
        world.spawn((
            EditorCamera,
            PanOrbitCamera {
                target_focus: Vec3::ZERO,
                ..default()
            },
        ));

        let mut action = FocusCameraAction {
            old_position: Vec3::ZERO,
            new_position: Vec3::new(100.0, 0.0, 0.0),
        };

        action.apply(&mut world);

        // All editor cameras should be updated
        let mut query = world.query_filtered::<&PanOrbitCamera, With<EditorCamera>>();
        for camera in query.iter(&world) {
            assert_eq!(camera.target_focus, Vec3::new(100.0, 0.0, 0.0));
        }
    }
}

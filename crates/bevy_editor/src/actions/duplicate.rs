use bevy::prelude::*;

use super::{Action, UndoFn};

/// Duplicate an entity, offsetting it by the given normal vector
#[derive(Clone, Debug)]
pub struct DuplicateAction {
    pub entity: Entity,
    pub offset: Vec3,
}

impl Action for DuplicateAction {
    fn apply(&self, world: &mut World) -> UndoFn {
        let new_entity = world
            .entity_mut(self.entity)
            .clone_and_spawn_with_opt_out(|builder| {
                builder.linked_cloning(true);
            });

        if let Some(mut transform) = world.get_mut::<Transform>(new_entity) {
            transform.translation += self.offset;
        }

        Box::new(move |world: &mut World| {
            world.entity_mut(new_entity).despawn();
        })
    }

    fn name(&self) -> String {
        format!("duplicate {}", self.entity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duplicate_creates_new_entity() {
        let mut world = World::new();
        let original = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let action = DuplicateAction {
            entity: original,
            offset: Vec3::X,
        };

        let _undo = action.apply(&mut world);

        let count = world.query::<&Transform>().iter(&world).count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_duplicate_undo_removes_entity() {
        let mut world = World::new();
        let original = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let action = DuplicateAction {
            entity: original,
            offset: Vec3::X,
        };

        let undo = action.apply(&mut world);
        assert_eq!(world.query::<&Transform>().iter(&world).count(), 2);

        undo(&mut world);
        assert_eq!(world.query::<&Transform>().iter(&world).count(), 1);
    }

    #[test]
    fn test_duplicate_applies_offset() {
        let mut world = World::new();
        let original_pos = Vec3::new(1.0, 2.0, 3.0);
        let offset = Vec3::new(5.0, 0.0, 0.0);
        let original = world.spawn(Transform::from_translation(original_pos)).id();

        let action = DuplicateAction {
            entity: original,
            offset,
        };

        let _undo = action.apply(&mut world);

        // Find the new entity (not the original)
        let transforms: Vec<_> = world.query::<&Transform>().iter(&world).collect();
        assert_eq!(transforms.len(), 2);

        // One should be at original position, one at original + offset
        let positions: Vec<Vec3> = transforms.iter().map(|t| t.translation).collect();
        assert!(positions.contains(&original_pos));
        assert!(positions.contains(&(original_pos + offset)));
    }

    #[test]
    fn test_duplicate_name_contains_entity() {
        let action = DuplicateAction {
            entity: Entity::PLACEHOLDER,
            offset: Vec3::ZERO,
        };
        assert!(action.name().starts_with("duplicate "));
    }

    #[test]
    fn test_duplicate_with_zero_offset() {
        let mut world = World::new();
        let original_pos = Vec3::new(1.0, 2.0, 3.0);
        let original = world.spawn(Transform::from_translation(original_pos)).id();

        let action = DuplicateAction {
            entity: original,
            offset: Vec3::ZERO,
        };

        let _undo = action.apply(&mut world);

        // Both entities should be at the same position
        let transforms: Vec<_> = world.query::<&Transform>().iter(&world).collect();
        assert_eq!(transforms.len(), 2);
        assert!(transforms.iter().all(|t| t.translation == original_pos));
    }

    #[test]
    fn test_duplicate_preserves_original() {
        let mut world = World::new();
        let original_pos = Vec3::new(1.0, 2.0, 3.0);
        let original = world.spawn(Transform::from_translation(original_pos)).id();

        let action = DuplicateAction {
            entity: original,
            offset: Vec3::X * 10.0,
        };

        let _undo = action.apply(&mut world);

        // Original entity should still exist and be unchanged
        let original_transform = world.get::<Transform>(original).unwrap();
        assert_eq!(original_transform.translation, original_pos);
    }

    #[test]
    fn test_duplicate_undo_preserves_original() {
        let mut world = World::new();
        let original_pos = Vec3::new(1.0, 2.0, 3.0);
        let original = world.spawn(Transform::from_translation(original_pos)).id();

        let action = DuplicateAction {
            entity: original,
            offset: Vec3::X,
        };

        let undo = action.apply(&mut world);
        undo(&mut world);

        // Original should still exist
        let original_transform = world.get::<Transform>(original).unwrap();
        assert_eq!(original_transform.translation, original_pos);
    }
}

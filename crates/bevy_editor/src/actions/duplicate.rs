use bevy::prelude::*;

use super::Action;

/// Marker component for entities that have been "undone" (hidden but not despawned)
#[derive(Component)]
pub struct UndoneEntity;

/// Duplicate an entity, offsetting it by the given normal vector
#[derive(Clone, Debug)]
pub struct DuplicateAction {
    pub entity: Entity,
    pub offset: Vec3,
    /// The entity that was created (stored after first apply for redo)
    created_entity: Option<Entity>,
}

impl DuplicateAction {
    pub fn new(entity: Entity, offset: Vec3) -> Self {
        Self {
            entity,
            offset,
            created_entity: None,
        }
    }

    /// Get the created entity (if any)
    pub fn created_entity(&self) -> Option<Entity> {
        self.created_entity
    }
}

impl Action for DuplicateAction {
    fn apply(&mut self, world: &mut World) {
        if let Some(existing) = self.created_entity {
            // Redo: re-enable the existing entity
            if let Ok(mut entity_mut) = world.get_entity_mut(existing) {
                entity_mut.remove::<UndoneEntity>();
                entity_mut.insert(Visibility::Inherited);
            }
        } else {
            // First apply: create the entity
            let new_entity = world
                .entity_mut(self.entity)
                .clone_and_spawn_with_opt_out(|builder| {
                    builder.linked_cloning(true);
                });

            if let Some(mut transform) = world.get_mut::<Transform>(new_entity) {
                transform.translation += self.offset;
            }

            self.created_entity = Some(new_entity);
        }
    }

    fn revert(&mut self, world: &mut World) {
        if let Some(entity) = self.created_entity {
            // Hide the entity instead of despawning it
            if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                entity_mut.insert((UndoneEntity, Visibility::Hidden));
            }
        }
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

        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);

        let count = world.query::<&Transform>().iter(&world).count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_duplicate_undo_hides_entity() {
        let mut world = World::new();
        let original = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);
        assert_eq!(world.query::<&Transform>().iter(&world).count(), 2);

        action.revert(&mut world);
        // Entity still exists but is hidden
        assert_eq!(world.query::<&Transform>().iter(&world).count(), 2);
        let created = action.created_entity().unwrap();
        assert!(world.get::<UndoneEntity>(created).is_some());
        assert_eq!(*world.get::<Visibility>(created).unwrap(), Visibility::Hidden);
    }

    #[test]
    fn test_duplicate_redo_restores_entity() {
        let mut world = World::new();
        let original = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);
        let created = action.created_entity().unwrap();

        action.revert(&mut world);
        assert!(world.get::<UndoneEntity>(created).is_some());

        action.apply(&mut world);
        // Entity should be visible again with same ID
        assert_eq!(action.created_entity().unwrap(), created);
        assert!(world.get::<UndoneEntity>(created).is_none());
        assert_eq!(*world.get::<Visibility>(created).unwrap(), Visibility::Inherited);
    }

    #[test]
    fn test_duplicate_applies_offset() {
        let mut world = World::new();
        let original_pos = Vec3::new(1.0, 2.0, 3.0);
        let offset = Vec3::new(5.0, 0.0, 0.0);
        let original = world.spawn(Transform::from_translation(original_pos)).id();

        let mut action = DuplicateAction::new(original, offset);
        action.apply(&mut world);

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
        let action = DuplicateAction::new(Entity::PLACEHOLDER, Vec3::ZERO);
        assert!(action.name().starts_with("duplicate "));
    }

    #[test]
    fn test_duplicate_with_zero_offset() {
        let mut world = World::new();
        let original_pos = Vec3::new(1.0, 2.0, 3.0);
        let original = world.spawn(Transform::from_translation(original_pos)).id();

        let mut action = DuplicateAction::new(original, Vec3::ZERO);
        action.apply(&mut world);

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

        let mut action = DuplicateAction::new(original, Vec3::X * 10.0);
        action.apply(&mut world);

        // Original entity should still exist and be unchanged
        let original_transform = world.get::<Transform>(original).unwrap();
        assert_eq!(original_transform.translation, original_pos);
    }

    #[test]
    fn test_duplicate_undo_preserves_original() {
        let mut world = World::new();
        let original_pos = Vec3::new(1.0, 2.0, 3.0);
        let original = world.spawn(Transform::from_translation(original_pos)).id();

        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);
        action.revert(&mut world);

        // Original should still exist
        let original_transform = world.get::<Transform>(original).unwrap();
        assert_eq!(original_transform.translation, original_pos);
    }
}

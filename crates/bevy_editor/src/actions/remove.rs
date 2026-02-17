use bevy::prelude::*;

use super::Action;
use super::duplicate::UndoneEntity;

/// Remove an entity from the scene
#[derive(Clone, Debug)]
pub struct RemoveAction {
    pub entity: Entity,
    /// Whether the entity was already hidden before removal (for proper undo)
    was_hidden: bool,
}

impl RemoveAction {
    pub fn new(entity: Entity) -> Self {
        Self {
            entity,
            was_hidden: false,
        }
    }
}

impl Action for RemoveAction {
    fn apply(&mut self, world: &mut World) {
        if let Ok(mut entity_mut) = world.get_entity_mut(self.entity) {
            // Check if already hidden
            self.was_hidden = entity_mut.get::<UndoneEntity>().is_some();
            // Hide the entity instead of despawning it (allows undo)
            entity_mut.insert((UndoneEntity, Visibility::Hidden));
        }
    }

    fn revert(&mut self, world: &mut World) {
        if let Ok(mut entity_mut) = world.get_entity_mut(self.entity) {
            // Only restore if it wasn't already hidden before
            if !self.was_hidden {
                entity_mut.remove::<UndoneEntity>();
                entity_mut.insert(Visibility::Inherited);
            }
        }
    }

    fn name(&self) -> String {
        format!("remove {}", self.entity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_hides_entity() {
        let mut world = World::new();
        let entity = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = RemoveAction::new(entity);
        action.apply(&mut world);

        assert!(world.get::<UndoneEntity>(entity).is_some());
        assert_eq!(*world.get::<Visibility>(entity).unwrap(), Visibility::Hidden);
    }

    #[test]
    fn test_remove_undo_restores_entity() {
        let mut world = World::new();
        let entity = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = RemoveAction::new(entity);
        action.apply(&mut world);
        
        assert!(world.get::<UndoneEntity>(entity).is_some());

        action.revert(&mut world);
        
        assert!(world.get::<UndoneEntity>(entity).is_none());
        assert_eq!(*world.get::<Visibility>(entity).unwrap(), Visibility::Inherited);
    }

    #[test]
    fn test_remove_redo_hides_again() {
        let mut world = World::new();
        let entity = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = RemoveAction::new(entity);
        action.apply(&mut world);
        action.revert(&mut world);
        
        assert!(world.get::<UndoneEntity>(entity).is_none());
        
        action.apply(&mut world);
        
        assert!(world.get::<UndoneEntity>(entity).is_some());
        assert_eq!(*world.get::<Visibility>(entity).unwrap(), Visibility::Hidden);
    }

    #[test]
    fn test_remove_name_contains_entity() {
        let action = RemoveAction::new(Entity::PLACEHOLDER);
        assert!(action.name().starts_with("remove "));
    }

    #[test]
    fn test_remove_preserves_entity_data() {
        let mut world = World::new();
        let entity = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = RemoveAction::new(entity);
        action.apply(&mut world);
        action.revert(&mut world);

        // Entity should still have its original transform
        let transform = world.get::<Transform>(entity).unwrap();
        assert_eq!(transform.translation, Vec3::new(1.0, 2.0, 3.0));
    }
}

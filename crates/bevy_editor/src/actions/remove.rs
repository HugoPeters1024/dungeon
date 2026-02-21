use bevy::prelude::*;

use super::Action;
use super::{TrashRoot, move_to_trash, restore_from_trash};

/// Remove an entity from the scene
#[derive(Clone, Debug)]
pub struct RemoveAction {
    pub entity: Entity,
}

impl RemoveAction {
    pub fn new(entity: Entity) -> Self {
        Self { entity }
    }
}

impl Action for RemoveAction {
    fn apply(&mut self, world: &mut World) {
        world.resource_scope::<TrashRoot, ()>(|world, trash| {
            if let Ok(mut entity_mut) = world.get_entity_mut(self.entity) {
                move_to_trash(&mut entity_mut, trash.0);
            }
        });
    }

    fn revert(&mut self, world: &mut World) {
        if let Ok(mut entity_mut) = world.get_entity_mut(self.entity) {
            restore_from_trash(&mut entity_mut);
        }
    }

    fn name(&self) -> String {
        format!("remove {}", self.entity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::TrashRootMarker;

    fn setup_trash(world: &mut World) -> Entity {
        let trash = world.spawn(TrashRootMarker).id();
        world.insert_resource(TrashRoot(trash));
        trash
    }

    #[test]
    fn test_remove_moves_to_trash() {
        let mut world = World::new();
        let trash = setup_trash(&mut world);
        let entity = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = RemoveAction::new(entity);
        action.apply(&mut world);

        assert_eq!(world.get::<ChildOf>(entity).unwrap().parent(), trash);
    }

    #[test]
    fn test_remove_undo_restores_entity() {
        let mut world = World::new();
        setup_trash(&mut world);
        let entity = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = RemoveAction::new(entity);
        action.apply(&mut world);
        action.revert(&mut world);

        assert!(world.get::<ChildOf>(entity).is_none());
    }

    #[test]
    fn test_remove_undo_restores_previous_parent() {
        let mut world = World::new();
        setup_trash(&mut world);
        let parent = world.spawn_empty().id();
        let entity = world
            .spawn((Transform::from_xyz(1.0, 2.0, 3.0), ChildOf(parent)))
            .id();

        let mut action = RemoveAction::new(entity);
        action.apply(&mut world);
        action.revert(&mut world);

        assert_eq!(world.get::<ChildOf>(entity).unwrap().parent(), parent);
    }

    #[test]
    fn test_remove_redo_moves_to_trash_again() {
        let mut world = World::new();
        let trash = setup_trash(&mut world);
        let entity = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = RemoveAction::new(entity);
        action.apply(&mut world);
        action.revert(&mut world);

        assert!(world.get::<ChildOf>(entity).is_none());

        action.apply(&mut world);

        assert_eq!(world.get::<ChildOf>(entity).unwrap().parent(), trash);
    }

    #[test]
    fn test_remove_name_contains_entity() {
        let action = RemoveAction::new(Entity::PLACEHOLDER);
        assert!(action.name().starts_with("remove "));
    }

    #[test]
    fn test_remove_preserves_entity_data() {
        let mut world = World::new();
        setup_trash(&mut world);
        let entity = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = RemoveAction::new(entity);
        action.apply(&mut world);
        action.revert(&mut world);

        let transform = world.get::<Transform>(entity).unwrap();
        assert_eq!(transform.translation, Vec3::new(1.0, 2.0, 3.0));
    }
}

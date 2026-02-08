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

        // Should now have 2 entities with Transform
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
}

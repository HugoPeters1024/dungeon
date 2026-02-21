use bevy::prelude::*;

use crate::PrefabId;

use super::Action;
use super::{TrashRoot, move_to_trash, restore_from_trash};

/// Spawn a prefab at a given position
#[derive(Clone, Debug)]
pub struct SpawnPrefabAction {
    pub prefab_id: PrefabId,
    pub position: Vec3,
    /// The entity that was created (stored after first apply for redo)
    created_entity: Option<Entity>,
}

impl SpawnPrefabAction {
    pub fn new(prefab_id: PrefabId, position: Vec3) -> Self {
        Self {
            prefab_id,
            position,
            created_entity: None,
        }
    }

    /// Get the created entity (if any)
    pub fn created_entity(&self) -> Option<Entity> {
        self.created_entity
    }
}

impl Action for SpawnPrefabAction {
    fn apply(&mut self, world: &mut World) {
        if let Some(existing) = self.created_entity {
            if let Ok(mut entity_mut) = world.get_entity_mut(existing) {
                restore_from_trash(&mut entity_mut);
            }
        } else {
            let entity = world
                .spawn((
                    self.prefab_id.clone(),
                    Transform::from_translation(self.position),
                ))
                .id();

            self.created_entity = Some(entity);
        }
    }

    fn revert(&mut self, world: &mut World) {
        world.resource_scope::<TrashRoot, ()>(|world, trash| {
            if let Some(entity) = self.created_entity {
                if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                    move_to_trash(&mut entity_mut, trash.0);
                }
            }
        });
    }

    fn name(&self) -> String {
        format!("spawn {}", self.prefab_id.name())
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
    fn test_spawn_prefab_creates_entity() {
        let mut world = World::new();

        let mut action = SpawnPrefabAction::new(PrefabId::new("test"), Vec3::new(1.0, 2.0, 3.0));
        action.apply(&mut world);

        assert!(action.created_entity().is_some());
        let entity = action.created_entity().unwrap();

        let transform = world.get::<Transform>(entity).unwrap();
        assert_eq!(transform.translation, Vec3::new(1.0, 2.0, 3.0));

        let prefab_id = world.get::<PrefabId>(entity).unwrap();
        assert_eq!(prefab_id.name(), "test");
    }

    #[test]
    fn test_spawn_prefab_undo_moves_to_trash() {
        let mut world = World::new();
        let trash = setup_trash(&mut world);

        let mut action = SpawnPrefabAction::new(PrefabId::new("test"), Vec3::ZERO);
        action.apply(&mut world);

        let entity = action.created_entity().unwrap();
        assert!(world.get::<ChildOf>(entity).is_none());

        action.revert(&mut world);

        assert_eq!(world.get::<ChildOf>(entity).unwrap().parent(), trash);
    }

    #[test]
    fn test_spawn_prefab_redo_restores_entity() {
        let mut world = World::new();
        let trash = setup_trash(&mut world);

        let mut action = SpawnPrefabAction::new(PrefabId::new("test"), Vec3::ZERO);
        action.apply(&mut world);
        let entity = action.created_entity().unwrap();

        action.revert(&mut world);
        assert_eq!(world.get::<ChildOf>(entity).unwrap().parent(), trash);

        action.apply(&mut world);
        assert_eq!(action.created_entity().unwrap(), entity);
        assert!(world.get::<ChildOf>(entity).is_none());
    }

    #[test]
    fn test_spawn_prefab_name() {
        let action = SpawnPrefabAction::new(PrefabId::new("rock"), Vec3::ZERO);
        assert_eq!(action.name(), "spawn rock");
    }
}

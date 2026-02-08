use bevy::prelude::*;

use crate::{PrefabId, Selected};

use super::{Action, UndoFn};

/// Merge multiple entities into a new parent entity
/// Removes PrefabId from each entity and creates a new parent with combined PrefabId
#[derive(Clone, Debug)]
pub struct MergeAction {
    pub entities: Vec<Entity>,
}

impl Action for MergeAction {
    fn apply(&self, world: &mut World) -> UndoFn {
        // Collect PrefabId names and world transforms from all entities
        let mut prefab_names: Vec<String> = Vec::new();
        let mut original_prefab_ids: Vec<(Entity, String)> = Vec::new();
        let mut world_transforms: Vec<(Entity, Transform)> = Vec::new();

        for &entity in &self.entities {
            if let Some(prefab_id) = world.get::<PrefabId>(entity) {
                let name = prefab_id.name().to_string();
                prefab_names.push(name.clone());
                original_prefab_ids.push((entity, name));
            }
            if let Some(global_transform) = world.get::<GlobalTransform>(entity) {
                let (scale, rotation, translation) =
                    global_transform.to_scale_rotation_translation();
                world_transforms.push((
                    entity,
                    Transform {
                        translation,
                        rotation,
                        scale,
                    },
                ));
            }
        }

        let original_world_transforms = world_transforms.clone();

        // Remove PrefabId from all entities
        for &entity in &self.entities {
            world.entity_mut(entity).remove::<PrefabId>();
        }

        let combined_name = prefab_names.join("-");

        // Compute average transform for the parent
        let count = world_transforms.len() as f32;
        let parent_transform = if count > 0.0 {
            let avg_translation = world_transforms
                .iter()
                .map(|(_, t)| t.translation)
                .sum::<Vec3>()
                / count;
            let avg_scale = world_transforms.iter().map(|(_, t)| t.scale).sum::<Vec3>() / count;
            let avg_rotation = world_transforms
                .first()
                .map(|(_, t)| t.rotation)
                .unwrap_or(Quat::IDENTITY);
            Transform {
                translation: avg_translation,
                rotation: avg_rotation,
                scale: avg_scale,
            }
        } else {
            Transform::default()
        };

        let parent = world
            .spawn((
                PrefabId::new(combined_name),
                parent_transform,
                InheritedVisibility::default(),
            ))
            .add_children(&self.entities)
            .id();

        // Adjust each child's local transform
        let parent_inverse = parent_transform.compute_affine().inverse();
        for (entity, world_transform) in world_transforms {
            let local_pos = parent_inverse.transform_point3(world_transform.translation);
            let local_rotation = parent_transform.rotation.inverse() * world_transform.rotation;
            let local_scale = world_transform.scale / parent_transform.scale;

            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                transform.translation = local_pos;
                transform.rotation = local_rotation;
                transform.scale = local_scale;
            }
        }

        if let Some(mut selected) = world.get_resource_mut::<Selected>() {
            selected.set_single(parent);
        }

        let entities = self.entities.clone();
        Box::new(move |world: &mut World| {
            for &entity in &entities {
                world.entity_mut(entity).remove_parent_in_place();
            }

            for (entity, original_transform) in &original_world_transforms {
                if let Some(mut transform) = world.get_mut::<Transform>(*entity) {
                    transform.translation = original_transform.translation;
                    transform.rotation = original_transform.rotation;
                    transform.scale = original_transform.scale;
                }
            }

            for (entity, name) in &original_prefab_ids {
                world.entity_mut(*entity).insert(PrefabId::new(name));
            }

            world.entity_mut(parent).despawn();

            if let Some(mut selected) = world.get_resource_mut::<Selected>() {
                if let Some(&first) = entities.first() {
                    selected.entities = entities.clone();
                    selected.set_single(first);
                }
            }
        })
    }

    fn name(&self) -> String {
        format!("merge {} entities", self.entities.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_selected(world: &mut World) {
        // Insert a dummy Selected resource for tests
        world.insert_resource(Selected::new(Entity::PLACEHOLDER));
    }

    #[test]
    fn test_merge_action_name() {
        let action = MergeAction {
            entities: vec![Entity::PLACEHOLDER, Entity::PLACEHOLDER],
        };
        assert_eq!(action.name(), "merge 2 entities");
    }

    #[test]
    fn test_merge_action_name_single() {
        let action = MergeAction {
            entities: vec![Entity::PLACEHOLDER],
        };
        assert_eq!(action.name(), "merge 1 entities");
    }

    #[test]
    fn test_merge_creates_parent() {
        let mut world = World::new();
        setup_selected(&mut world);

        let e1 = world.spawn((
            Transform::from_xyz(0.0, 0.0, 0.0),
            GlobalTransform::from_xyz(0.0, 0.0, 0.0),
            PrefabId::new("entity1"),
        )).id();
        let e2 = world.spawn((
            Transform::from_xyz(2.0, 0.0, 0.0),
            GlobalTransform::from_xyz(2.0, 0.0, 0.0),
            PrefabId::new("entity2"),
        )).id();

        let action = MergeAction {
            entities: vec![e1, e2],
        };

        let _undo = action.apply(&mut world);

        // Should have created a new parent entity with PrefabId
        let prefab_count = world.query::<&PrefabId>().iter(&world).count();
        assert_eq!(prefab_count, 1); // Only the parent has PrefabId now
    }

    #[test]
    fn test_merge_removes_prefab_ids_from_children() {
        let mut world = World::new();
        setup_selected(&mut world);

        let e1 = world.spawn((
            Transform::from_xyz(0.0, 0.0, 0.0),
            GlobalTransform::from_xyz(0.0, 0.0, 0.0),
            PrefabId::new("entity1"),
        )).id();
        let e2 = world.spawn((
            Transform::from_xyz(2.0, 0.0, 0.0),
            GlobalTransform::from_xyz(2.0, 0.0, 0.0),
            PrefabId::new("entity2"),
        )).id();

        let action = MergeAction {
            entities: vec![e1, e2],
        };

        let _undo = action.apply(&mut world);

        // Original entities should no longer have PrefabId
        assert!(world.get::<PrefabId>(e1).is_none());
        assert!(world.get::<PrefabId>(e2).is_none());
    }

    #[test]
    fn test_merge_undo_restores_prefab_ids() {
        let mut world = World::new();
        setup_selected(&mut world);

        let e1 = world.spawn((
            Transform::from_xyz(0.0, 0.0, 0.0),
            GlobalTransform::from_xyz(0.0, 0.0, 0.0),
            PrefabId::new("entity1"),
        )).id();
        let e2 = world.spawn((
            Transform::from_xyz(2.0, 0.0, 0.0),
            GlobalTransform::from_xyz(2.0, 0.0, 0.0),
            PrefabId::new("entity2"),
        )).id();

        let action = MergeAction {
            entities: vec![e1, e2],
        };

        let undo = action.apply(&mut world);
        undo(&mut world);

        // Original entities should have their PrefabIds back
        assert_eq!(world.get::<PrefabId>(e1).unwrap().name(), "entity1");
        assert_eq!(world.get::<PrefabId>(e2).unwrap().name(), "entity2");
    }

    #[test]
    fn test_merge_undo_removes_parent() {
        let mut world = World::new();
        setup_selected(&mut world);

        let e1 = world.spawn((
            Transform::from_xyz(0.0, 0.0, 0.0),
            GlobalTransform::from_xyz(0.0, 0.0, 0.0),
            PrefabId::new("entity1"),
        )).id();
        let e2 = world.spawn((
            Transform::from_xyz(2.0, 0.0, 0.0),
            GlobalTransform::from_xyz(2.0, 0.0, 0.0),
            PrefabId::new("entity2"),
        )).id();

        let initial_entity_count = world.entities().len();

        let action = MergeAction {
            entities: vec![e1, e2],
        };

        let undo = action.apply(&mut world);
        
        // One more entity (the parent)
        assert_eq!(world.entities().len(), initial_entity_count + 1);

        undo(&mut world);

        // Back to original count
        assert_eq!(world.entities().len(), initial_entity_count);
    }

    #[test]
    fn test_merge_empty_entities() {
        let mut world = World::new();
        setup_selected(&mut world);

        let action = MergeAction {
            entities: vec![],
        };

        let undo = action.apply(&mut world);
        undo(&mut world); // Should not panic
    }

    #[test]
    fn test_merge_combined_prefab_name() {
        let mut world = World::new();
        setup_selected(&mut world);

        let e1 = world.spawn((
            Transform::from_xyz(0.0, 0.0, 0.0),
            GlobalTransform::from_xyz(0.0, 0.0, 0.0),
            PrefabId::new("rock"),
        )).id();
        let e2 = world.spawn((
            Transform::from_xyz(2.0, 0.0, 0.0),
            GlobalTransform::from_xyz(2.0, 0.0, 0.0),
            PrefabId::new("tree"),
        )).id();

        let action = MergeAction {
            entities: vec![e1, e2],
        };

        let _undo = action.apply(&mut world);

        // Find the parent's PrefabId
        let prefab_ids: Vec<_> = world.query::<&PrefabId>().iter(&world).collect();
        assert_eq!(prefab_ids.len(), 1);
        assert_eq!(prefab_ids[0].name(), "rock-tree");
    }
}

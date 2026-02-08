use bevy::prelude::*;

use crate::{PrefabId, Selected};

use super::Action;
use super::duplicate::UndoneEntity;

/// Merge multiple entities into a new parent entity
/// Removes PrefabId from each entity and creates a new parent with combined PrefabId
#[derive(Clone, Debug)]
pub struct MergeAction {
    pub entities: Vec<Entity>,
    /// State stored after first apply for undo/redo
    state: Option<MergeState>,
}

#[derive(Clone, Debug)]
struct MergeState {
    parent: Entity,
    original_prefab_ids: Vec<(Entity, String)>,
    original_world_transforms: Vec<(Entity, Transform)>,
    parent_transform: Transform,
}

impl MergeAction {
    pub fn new(entities: Vec<Entity>) -> Self {
        Self { entities, state: None }
    }
}

impl Action for MergeAction {
    fn apply(&mut self, world: &mut World) {
        if let Some(state) = &self.state {
            // Redo: re-enable the parent and re-parent children
            if let Ok(mut entity_mut) = world.get_entity_mut(state.parent) {
                entity_mut.remove::<UndoneEntity>();
                entity_mut.insert(Visibility::Inherited);
            }

            // Re-parent the children
            for &entity in &self.entities {
                world.entity_mut(entity).remove::<PrefabId>();
                world.entity_mut(entity).set_parent_in_place(state.parent);
            }

            // Recompute local transforms
            let parent_inverse = state.parent_transform.compute_affine().inverse();
            for (entity, world_transform) in &state.original_world_transforms {
                let local_pos = parent_inverse.transform_point3(world_transform.translation);
                let local_rotation = state.parent_transform.rotation.inverse() * world_transform.rotation;
                let local_scale = world_transform.scale / state.parent_transform.scale;

                if let Some(mut transform) = world.get_mut::<Transform>(*entity) {
                    transform.translation = local_pos;
                    transform.rotation = local_rotation;
                    transform.scale = local_scale;
                }
            }

            if let Some(mut selected) = world.get_resource_mut::<Selected>() {
                selected.set_single(state.parent);
            }
        } else {
            // First apply: create everything
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
                        Transform { translation, rotation, scale },
                    ));
                }
            }

            let original_world_transforms = world_transforms.clone();

            for &entity in &self.entities {
                world.entity_mut(entity).remove::<PrefabId>();
            }

            let combined_name = prefab_names.join("-");

            let count = world_transforms.len() as f32;
            let parent_transform = if count > 0.0 {
                let avg_translation = world_transforms.iter().map(|(_, t)| t.translation).sum::<Vec3>() / count;
                let avg_scale = world_transforms.iter().map(|(_, t)| t.scale).sum::<Vec3>() / count;
                let avg_rotation = world_transforms.first().map(|(_, t)| t.rotation).unwrap_or(Quat::IDENTITY);
                Transform { translation: avg_translation, rotation: avg_rotation, scale: avg_scale }
            } else {
                Transform::default()
            };

            let parent = world
                .spawn((
                    PrefabId::new(&combined_name),
                    parent_transform,
                    InheritedVisibility::default(),
                ))
                .add_children(&self.entities)
                .id();

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

            self.state = Some(MergeState {
                parent,
                original_prefab_ids,
                original_world_transforms,
                parent_transform,
            });
        }
    }

    fn revert(&mut self, world: &mut World) {
        let Some(state) = &self.state else { return };

        // Unparent children
        for &entity in &self.entities {
            world.entity_mut(entity).remove_parent_in_place();
        }

        // Restore original transforms
        for (entity, original_transform) in &state.original_world_transforms {
            if let Some(mut transform) = world.get_mut::<Transform>(*entity) {
                transform.translation = original_transform.translation;
                transform.rotation = original_transform.rotation;
                transform.scale = original_transform.scale;
            }
        }

        // Restore original PrefabIds
        for (entity, name) in &state.original_prefab_ids {
            world.entity_mut(*entity).insert(PrefabId::new(name));
        }

        // Hide the parent instead of despawning
        if let Ok(mut entity_mut) = world.get_entity_mut(state.parent) {
            entity_mut.insert((UndoneEntity, Visibility::Hidden));
        }

        // Select the original entities
        if let Some(mut selected) = world.get_resource_mut::<Selected>() {
            if let Some(&first) = self.entities.first() {
                selected.entities = self.entities.clone();
                selected.set_single(first);
            }
        }
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
        let action = MergeAction::new(vec![Entity::PLACEHOLDER, Entity::PLACEHOLDER]);
        assert_eq!(action.name(), "merge 2 entities");
    }

    #[test]
    fn test_merge_action_name_single() {
        let action = MergeAction::new(vec![Entity::PLACEHOLDER]);
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

        let mut action = MergeAction::new(vec![e1, e2]);
        action.apply(&mut world);

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

        let mut action = MergeAction::new(vec![e1, e2]);
        action.apply(&mut world);

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

        let mut action = MergeAction::new(vec![e1, e2]);
        action.apply(&mut world);
        action.revert(&mut world);

        // Original entities should have their PrefabIds back
        assert_eq!(world.get::<PrefabId>(e1).unwrap().name(), "entity1");
        assert_eq!(world.get::<PrefabId>(e2).unwrap().name(), "entity2");
    }

    #[test]
    fn test_merge_undo_hides_parent() {
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

        let mut action = MergeAction::new(vec![e1, e2]);
        action.apply(&mut world);

        // One more entity (the parent)
        assert_eq!(world.entities().len(), initial_entity_count + 1);

        action.revert(&mut world);

        // Parent still exists but is hidden (not despawned for redo support)
        assert_eq!(world.entities().len(), initial_entity_count + 1);
    }

    #[test]
    fn test_merge_empty_entities() {
        let mut world = World::new();
        setup_selected(&mut world);

        let mut action = MergeAction::new(vec![]);
        action.apply(&mut world);
        action.revert(&mut world); // Should not panic
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

        let mut action = MergeAction::new(vec![e1, e2]);
        action.apply(&mut world);

        // Find the parent's PrefabId
        let prefab_ids: Vec<_> = world.query::<&PrefabId>().iter(&world).collect();
        assert_eq!(prefab_ids.len(), 1);
        assert_eq!(prefab_ids[0].name(), "rock-tree");
    }
}

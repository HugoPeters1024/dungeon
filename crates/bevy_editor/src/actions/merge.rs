use bevy::prelude::*;

use crate::merged_aabb::MergedAabb;
use crate::{PrefabId, Selected};

use super::Action;
use super::{TrashRoot, move_to_trash, restore_from_trash};

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
        Self {
            entities,
            state: None,
        }
    }
}

/// Convert a child's world-space transform into the local transform it should
/// have under `parent_transform`, and write it onto the entity.
fn apply_local_under_parent(
    world: &mut World,
    entity: Entity,
    parent_transform: &Transform,
    world_transform: &Transform,
) {
    let parent_inverse = parent_transform.compute_affine().inverse();
    if let Some(mut transform) = world.get_mut::<Transform>(entity) {
        transform.translation = parent_inverse.transform_point3(world_transform.translation);
        transform.rotation = parent_transform.rotation.inverse() * world_transform.rotation;
        transform.scale = world_transform.scale / parent_transform.scale;
    }
}

impl Action for MergeAction {
    fn apply(&mut self, world: &mut World) {
        if let Some(state) = &self.state {
            if let Ok(mut entity_mut) = world.get_entity_mut(state.parent) {
                restore_from_trash(&mut entity_mut);
            }

            for &entity in &self.entities {
                world.entity_mut(entity).remove::<PrefabId>();
                world.entity_mut(entity).set_parent_in_place(state.parent);
            }

            for (entity, world_transform) in &state.original_world_transforms {
                apply_local_under_parent(world, *entity, &state.parent_transform, world_transform);
            }

            if let Some(mut selected) = world.get_resource_mut::<Selected>() {
                selected.set_single(state.parent);
            }
        } else {
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

            for &entity in &self.entities {
                world.entity_mut(entity).remove::<PrefabId>();
            }

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
                    parent_transform,
                    InheritedVisibility::default(),
                    MergedAabb::default(),
                ))
                .add_children(&self.entities)
                .id();

            for (entity, world_transform) in &world_transforms {
                apply_local_under_parent(world, *entity, &parent_transform, world_transform);
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

        for &entity in &self.entities {
            world.entity_mut(entity).remove_parent_in_place();
        }

        for (entity, original_transform) in &state.original_world_transforms {
            if let Some(mut transform) = world.get_mut::<Transform>(*entity) {
                transform.translation = original_transform.translation;
                transform.rotation = original_transform.rotation;
                transform.scale = original_transform.scale;
            }
        }

        for (entity, name) in &state.original_prefab_ids {
            world.entity_mut(*entity).insert(PrefabId::new(name));
        }

        world.resource_scope::<TrashRoot, ()>(|world, trash| {
            if let Ok(mut entity_mut) = world.get_entity_mut(state.parent) {
                move_to_trash(&mut entity_mut, trash.0);
            }
        });

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
    use crate::actions::TrashRootMarker;

    fn setup_world(world: &mut World) -> Entity {
        world.insert_resource(Selected::new(Entity::PLACEHOLDER));
        let trash = world.spawn(TrashRootMarker).id();
        world.insert_resource(TrashRoot(trash));
        trash
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
        setup_world(&mut world);

        let e1 = world
            .spawn((
                Transform::from_xyz(0.0, 0.0, 0.0),
                GlobalTransform::from_xyz(0.0, 0.0, 0.0),
                Name::new("entity1"),
            ))
            .id();
        let e2 = world
            .spawn((
                Transform::from_xyz(2.0, 0.0, 0.0),
                GlobalTransform::from_xyz(2.0, 0.0, 0.0),
                Name::new("entity2"),
            ))
            .id();

        let mut action = MergeAction::new(vec![e1, e2]);
        action.apply(&mut world);

        let mut q = world.query::<&ChildOf>();
        let parent_e1 = q.get(&world, e1);
        let parent_e2 = q.get(&world, e2);
        assert_eq!(parent_e1, parent_e2);
    }

    #[test]
    fn test_merge_removes_prefab_ids_from_children() {
        let mut world = World::new();
        setup_world(&mut world);

        let e1 = world
            .spawn((
                Transform::from_xyz(0.0, 0.0, 0.0),
                GlobalTransform::from_xyz(0.0, 0.0, 0.0),
                PrefabId::new("entity1"),
            ))
            .id();
        let e2 = world
            .spawn((
                Transform::from_xyz(2.0, 0.0, 0.0),
                GlobalTransform::from_xyz(2.0, 0.0, 0.0),
                PrefabId::new("entity2"),
            ))
            .id();

        let mut action = MergeAction::new(vec![e1, e2]);
        action.apply(&mut world);

        assert!(world.get::<PrefabId>(e1).is_none());
        assert!(world.get::<PrefabId>(e2).is_none());
    }

    #[test]
    fn test_merge_undo_restores_prefab_ids() {
        let mut world = World::new();
        setup_world(&mut world);

        let e1 = world
            .spawn((
                Transform::from_xyz(0.0, 0.0, 0.0),
                GlobalTransform::from_xyz(0.0, 0.0, 0.0),
                PrefabId::new("entity1"),
            ))
            .id();
        let e2 = world
            .spawn((
                Transform::from_xyz(2.0, 0.0, 0.0),
                GlobalTransform::from_xyz(2.0, 0.0, 0.0),
                PrefabId::new("entity2"),
            ))
            .id();

        let mut action = MergeAction::new(vec![e1, e2]);
        action.apply(&mut world);
        action.revert(&mut world);

        assert_eq!(world.get::<PrefabId>(e1).unwrap().name(), "entity1");
        assert_eq!(world.get::<PrefabId>(e2).unwrap().name(), "entity2");
    }

    #[test]
    fn test_merge_undo_trashes_parent() {
        let mut world = World::new();
        let trash = setup_world(&mut world);

        let e1 = world
            .spawn((
                Transform::from_xyz(0.0, 0.0, 0.0),
                GlobalTransform::from_xyz(0.0, 0.0, 0.0),
                PrefabId::new("entity1"),
            ))
            .id();
        let e2 = world
            .spawn((
                Transform::from_xyz(2.0, 0.0, 0.0),
                GlobalTransform::from_xyz(2.0, 0.0, 0.0),
                PrefabId::new("entity2"),
            ))
            .id();

        let mut action = MergeAction::new(vec![e1, e2]);
        action.apply(&mut world);

        let parent = action.state.as_ref().unwrap().parent;

        action.revert(&mut world);

        assert_eq!(world.get::<ChildOf>(parent).unwrap().parent(), trash);
    }

    #[test]
    fn test_merge_empty_entities() {
        let mut world = World::new();
        setup_world(&mut world);

        let mut action = MergeAction::new(vec![]);
        action.apply(&mut world);
        action.revert(&mut world);
    }
}

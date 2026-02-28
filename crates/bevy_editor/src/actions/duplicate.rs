use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::{camera::primitives::Aabb, platform::collections::HashSet};

use crate::actions::{TrashRoot, move_to_trash, restore_from_trash};

use super::Action;

/// Duplicate an entity, offsetting it in the given direction based on its AABB
/// so the duplicated object exactly touches the original.
#[derive(Clone, Debug)]
pub struct DuplicateAction {
    pub entity: Entity,
    /// The direction to offset the duplicate (typically a face normal)
    pub direction: Vec3,
    /// The computed offset (calculated on first apply based on AABB)
    computed_offset: Option<Vec3>,
    /// The entity that was created (stored after first apply for redo)
    created_entity: Option<Entity>,
}

impl DuplicateAction {
    pub fn new(entity: Entity, direction: Vec3) -> Self {
        Self {
            entity,
            direction,
            computed_offset: None,
            created_entity: None,
        }
    }

    /// Get the created entity (if any)
    pub fn created_entity(&self) -> Option<Entity> {
        self.created_entity
    }
}

/// Compute the world-space AABB for an entity and all its descendants
fn compute_world_aabb(world: &World, entity: Entity) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut has_aabb = false;

    let mut merge_aabb = |entity: Entity| {
        if let Some(global_transform) = world.get::<GlobalTransform>(entity) {
            if let Some(aabb) = world.get::<Aabb>(entity) {
                has_aabb = true;
                let center: Vec3 = aabb.center.into();
                let half: Vec3 = aabb.half_extents.into();
                for x in [-1.0, 1.0] {
                    for y in [-1.0, 1.0] {
                        for z in [-1.0, 1.0] {
                            let local_corner = center + half * Vec3::new(x, y, z);
                            let world_corner = global_transform.transform_point(local_corner);
                            min = min.min(world_corner);
                            max = max.max(world_corner);
                        }
                    }
                }
            }
        }
    };

    merge_aabb(entity);

    if let Some(children) = world.get::<Children>(entity) {
        fn merge_descendants(world: &World, entity: Entity, merge: &mut impl FnMut(Entity)) {
            if let Some(children) = world.get::<Children>(entity) {
                for child in children.iter() {
                    merge(child);
                    merge_descendants(world, child, merge);
                }
            }
        }
        for child in children.iter() {
            merge_aabb(child);
            merge_descendants(world, child, &mut merge_aabb);
        }
    }

    if has_aabb { Some((min, max)) } else { None }
}

fn compute_offset(world: &World, entity: Entity, direction: Vec3) -> Vec3 {
    if let Some((min, max)) = compute_world_aabb(world, entity) {
        let size = max - min;
        let dir = direction.normalize_or_zero();
        let distance = size.x * dir.x.abs() + size.y * dir.y.abs() + size.z * dir.z.abs();
        dir * distance
    } else {
        direction
    }
}

fn collect_subtree_entities(world: &World, root: Entity) -> Vec<Entity> {
    let mut result = Vec::new();
    let mut stack = vec![root];

    while let Some(entity) = stack.pop() {
        result.push(entity);
        if let Some(children) = world.get::<Children>(entity) {
            for child in children.iter().rev() {
                stack.push(child);
            }
        }
    }

    result
}

impl Action for DuplicateAction {
    fn apply(&mut self, world: &mut World) {
        if let Some(existing) = self.created_entity {
            if let Ok(mut entity_mut) = world.get_entity_mut(existing) {
                restore_from_trash(&mut entity_mut);
            }
        } else {
            let offset = *self
                .computed_offset
                .get_or_insert_with(|| compute_offset(world, self.entity, self.direction));

            let source_entities = collect_subtree_entities(world, self.entity);
            let source_set = source_entities.iter().copied().collect::<HashSet<_>>();
            let source_parents: Vec<(Entity, Option<Entity>)> = source_entities
                .iter()
                .map(|&entity| {
                    let parent = world.get::<ChildOf>(entity).map(|c| c.parent());
                    let parent_in_subtree =
                        parent.filter(|p| source_set.contains(p) || entity == self.entity);
                    (entity, parent_in_subtree)
                })
                .collect();

            let mut cloned_entities: HashMap<Entity, Entity> = HashMap::default();
            for &source_entity in &source_entities {
                let cloned = world
                    .entity_mut(source_entity)
                    .clone_and_spawn_with_opt_out(|builder| {
                        // Prevent recursive cloning through linked relationships.
                        builder.linked_cloning(false);
                        // Rebuild hierarchy explicitly for the selected subtree only.
                        builder.deny::<(ChildOf, Children)>();
                    });
                cloned_entities.insert(source_entity, cloned);
            }

            for (source_entity, maybe_parent) in source_parents {
                let Some(&cloned_entity) = cloned_entities.get(&source_entity) else {
                    continue;
                };
                if let Some(source_parent) = maybe_parent {
                    if let Some(&cloned_parent) = cloned_entities.get(&source_parent) {
                        world
                            .entity_mut(cloned_entity)
                            .insert(ChildOf(cloned_parent));
                    } else if source_entity == self.entity {
                        world
                            .entity_mut(cloned_entity)
                            .insert(ChildOf(source_parent));
                    }
                }
            }

            let new_entity = cloned_entities[&self.entity];
            if let Some(mut transform) = world.get_mut::<Transform>(new_entity) {
                transform.translation += offset;
            }

            self.created_entity = Some(new_entity);
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
        format!("duplicate {}", self.entity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{TrashRoot, TrashRootMarker};

    fn setup_trash(world: &mut World) -> Entity {
        let trash = world.spawn(TrashRootMarker).id();
        world.insert_resource(TrashRoot(trash));
        trash
    }

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
    fn test_duplicate_clones_all_components() {
        let mut world = World::new();
        let original = world
            .spawn((Name::new("thing"), Transform::from_xyz(1.0, 0.0, 0.0)))
            .id();

        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);

        let created = action.created_entity().unwrap();
        assert_ne!(created, original);
        assert!(world.get::<Name>(created).is_some());
        assert!(world.get::<Transform>(created).is_some());
    }

    #[test]
    fn test_duplicate_copies_transform_with_offset() {
        let mut world = World::new();
        let original_pos = Vec3::new(3.0, 0.0, 0.0);
        let direction = Vec3::new(2.0, 0.0, 0.0);
        let original = world.spawn(Transform::from_translation(original_pos)).id();

        let mut action = DuplicateAction::new(original, direction);
        action.apply(&mut world);

        let created = action.created_entity().unwrap();
        let new_transform = world.get::<Transform>(created).unwrap();
        assert_eq!(new_transform.translation, original_pos + direction);
    }

    #[test]
    fn test_duplicate_undo_moves_to_trash() {
        let mut world = World::new();
        let trash = setup_trash(&mut world);
        let original = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);

        action.revert(&mut world);
        let created = action.created_entity().unwrap();
        let child_of = world.get::<ChildOf>(created).unwrap();
        assert_eq!(child_of.parent(), trash);
    }

    #[test]
    fn test_duplicate_redo_restores_entity() {
        let mut world = World::new();
        let trash = setup_trash(&mut world);
        let original = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);
        let created = action.created_entity().unwrap();

        action.revert(&mut world);
        assert_eq!(world.get::<ChildOf>(created).unwrap().parent(), trash);

        action.apply(&mut world);
        assert_eq!(action.created_entity().unwrap(), created);
        assert!(world.get::<ChildOf>(created).is_none());
    }

    #[test]
    fn test_duplicate_redo_restores_previous_parent() {
        let mut world = World::new();
        setup_trash(&mut world);
        let parent = world.spawn_empty().id();
        let original = world
            .spawn((Transform::from_xyz(1.0, 2.0, 3.0), ChildOf(parent)))
            .id();

        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);
        let created = action.created_entity().unwrap();

        action.revert(&mut world);
        action.apply(&mut world);
        assert_eq!(action.created_entity().unwrap(), created);
        // Clone inherited ChildOf(parent) from original, so redo restores that
        assert_eq!(world.get::<ChildOf>(created).unwrap().parent(), parent);
    }

    #[test]
    fn test_duplicate_with_aabb_offsets_by_size() {
        let mut world = World::new();
        let original_pos = Vec3::ZERO;

        let original = world
            .spawn((
                Transform::from_translation(original_pos),
                GlobalTransform::from_translation(original_pos),
                Aabb {
                    center: Vec3A::ZERO,
                    half_extents: Vec3A::ONE,
                },
            ))
            .id();

        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);

        let created = action.created_entity().unwrap();
        let new_transform = world.get::<Transform>(created).unwrap();
        assert_eq!(new_transform.translation, Vec3::new(2.0, 0.0, 0.0));
    }

    #[test]
    fn test_duplicate_with_aabb_diagonal_direction() {
        let mut world = World::new();
        let original_pos = Vec3::ZERO;

        let original = world
            .spawn((
                Transform::from_translation(original_pos),
                GlobalTransform::from_translation(original_pos),
                Aabb {
                    center: Vec3A::ZERO,
                    half_extents: Vec3A::new(2.0, 1.0, 3.0),
                },
            ))
            .id();

        let mut action = DuplicateAction::new(original, Vec3::Y);
        action.apply(&mut world);

        let created = action.created_entity().unwrap();
        let new_transform = world.get::<Transform>(created).unwrap();
        assert_eq!(new_transform.translation, Vec3::new(0.0, 2.0, 0.0));
    }

    #[test]
    fn test_duplicate_without_aabb_uses_direction() {
        let mut world = World::new();
        let original_pos = Vec3::new(1.0, 2.0, 3.0);
        let direction = Vec3::new(5.0, 0.0, 0.0);
        let original = world.spawn(Transform::from_translation(original_pos)).id();

        let mut action = DuplicateAction::new(original, direction);
        action.apply(&mut world);

        let created = action.created_entity().unwrap();
        let new_transform = world.get::<Transform>(created).unwrap();
        assert_eq!(new_transform.translation, original_pos + direction);
    }

    #[test]
    fn test_duplicate_name_contains_entity() {
        let action = DuplicateAction::new(Entity::PLACEHOLDER, Vec3::ZERO);
        assert!(action.name().starts_with("duplicate "));
    }

    #[test]
    fn test_duplicate_with_zero_direction() {
        let mut world = World::new();
        let original_pos = Vec3::new(1.0, 2.0, 3.0);
        let original = world.spawn(Transform::from_translation(original_pos)).id();

        let mut action = DuplicateAction::new(original, Vec3::ZERO);
        action.apply(&mut world);

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

        let original_transform = world.get::<Transform>(original).unwrap();
        assert_eq!(original_transform.translation, original_pos);
    }

    #[test]
    fn test_duplicate_undo_preserves_original() {
        let mut world = World::new();
        setup_trash(&mut world);
        let original_pos = Vec3::new(1.0, 2.0, 3.0);
        let original = world.spawn(Transform::from_translation(original_pos)).id();

        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);
        action.revert(&mut world);

        let original_transform = world.get::<Transform>(original).unwrap();
        assert_eq!(original_transform.translation, original_pos);
    }

    #[test]
    fn test_duplicate_preserves_modified_child_transforms() {
        let mut world = World::new();

        let child_pos = Vec3::new(0.0, 5.0, 0.0);
        let parent = world
            .spawn(Transform::from_xyz(0.0, 0.0, 0.0))
            .with_children(|p| {
                p.spawn(Transform::from_translation(child_pos));
            })
            .id();

        let mut action = DuplicateAction::new(parent, Vec3::ZERO);
        action.apply(&mut world);

        let created = action.created_entity().unwrap();
        let cloned_children: Vec<Entity> = world.get::<Children>(created).unwrap().iter().collect();

        assert_eq!(cloned_children.len(), 1);
        let cloned_child_transform = world.get::<Transform>(cloned_children[0]).unwrap();
        assert_eq!(cloned_child_transform.translation, child_pos);
    }
}

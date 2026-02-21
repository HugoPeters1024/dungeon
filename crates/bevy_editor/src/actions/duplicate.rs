use bevy::camera::primitives::Aabb;
use bevy::prelude::*;

use crate::PrefabId;
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

            let original_transform = world
                .get::<Transform>(self.entity)
                .copied()
                .unwrap_or_default();

            let mut new_transform = original_transform;
            new_transform.translation += offset;

            let new_entity = if let Some(prefab_id) = world.get::<PrefabId>(self.entity) {
                let prefab_id = prefab_id.clone();
                world.spawn((prefab_id, new_transform)).id()
            } else {
                let e = world
                    .entity_mut(self.entity)
                    .clone_and_spawn_with_opt_out(|builder| {
                        builder.linked_cloning(true);
                    });
                if let Some(mut transform) = world.get_mut::<Transform>(e) {
                    transform.translation += offset;
                }
                e
            };

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
    fn test_duplicate_prefab_spawns_only_prefab_id() {
        let mut world = World::new();
        let prefab_id = PrefabId::new("test_prefab");
        let original = world
            .spawn((prefab_id.clone(), Transform::from_xyz(1.0, 0.0, 0.0)))
            .id();

        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);

        let created = action.created_entity().unwrap();
        assert_ne!(created, original);

        let new_prefab = world.get::<PrefabId>(created).unwrap();
        assert_eq!(new_prefab.name(), "test_prefab");

        assert!(world.get::<Children>(created).is_none());
    }

    #[test]
    fn test_duplicate_prefab_copies_transform_with_offset() {
        let mut world = World::new();
        let original_pos = Vec3::new(3.0, 0.0, 0.0);
        let direction = Vec3::new(2.0, 0.0, 0.0);
        let original = world
            .spawn((
                PrefabId::new("thing"),
                Transform::from_translation(original_pos),
            ))
            .id();

        let mut action = DuplicateAction::new(original, direction);
        action.apply(&mut world);

        let created = action.created_entity().unwrap();
        let new_transform = world.get::<Transform>(created).unwrap();
        assert_eq!(new_transform.translation, original_pos + direction);
    }

    #[test]
    fn test_duplicate_non_prefab_still_clones() {
        let mut world = World::new();
        let original = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);

        let created = action.created_entity().unwrap();
        assert!(world.get::<PrefabId>(created).is_none());
        assert!(world.get::<Transform>(created).is_some());
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
}

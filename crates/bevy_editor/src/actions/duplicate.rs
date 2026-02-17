use bevy::camera::primitives::Aabb;
use bevy::prelude::*;

use super::Action;

/// Marker component for entities that have been "undone" (hidden but not despawned)
#[derive(Component)]
pub struct UndoneEntity;

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

    // Helper to merge an entity's AABB into the world-space bounds
    let mut merge_aabb = |entity: Entity| {
        if let Some(global_transform) = world.get::<GlobalTransform>(entity) {
            if let Some(aabb) = world.get::<Aabb>(entity) {
                has_aabb = true;
                // Transform the 8 corners of the local AABB to world space
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

    // Merge the entity's AABB
    merge_aabb(entity);

    // Merge all descendants' AABBs
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
            // Recursively check descendants
            merge_descendants(world, child, &mut merge_aabb);
        }
    }

    if has_aabb { Some((min, max)) } else { None }
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
            // First apply: compute offset based on AABB and create the entity
            let offset = if let Some(offset) = self.computed_offset {
                offset
            } else {
                // Compute offset based on AABB
                let offset = if let Some((min, max)) = compute_world_aabb(world, self.entity) {
                    let size = max - min;
                    let direction = self.direction.normalize_or_zero();
                    // Project the size onto the direction to get the offset distance
                    // We need to move by the full extent in that direction (size * |direction component|)
                    let offset_distance = size.x * direction.x.abs()
                        + size.y * direction.y.abs()
                        + size.z * direction.z.abs();
                    direction * offset_distance
                } else {
                    // Fallback: use direction as-is if no AABB
                    self.direction
                };
                self.computed_offset = Some(offset);
                offset
            };

            let new_entity =
                world
                    .entity_mut(self.entity)
                    .clone_and_spawn_with_opt_out(|builder| {
                        builder.linked_cloning(true);
                    });

            if let Some(mut transform) = world.get_mut::<Transform>(new_entity) {
                transform.translation += offset;
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
        assert_eq!(
            *world.get::<Visibility>(created).unwrap(),
            Visibility::Hidden
        );
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
        assert_eq!(
            *world.get::<Visibility>(created).unwrap(),
            Visibility::Inherited
        );
    }

    #[test]
    fn test_duplicate_with_aabb_offsets_by_size() {
        let mut world = World::new();
        let original_pos = Vec3::ZERO;

        // Create entity with AABB (2x2x2 box centered at origin)
        let original = world
            .spawn((
                Transform::from_translation(original_pos),
                GlobalTransform::from_translation(original_pos),
                Aabb {
                    center: Vec3A::ZERO,
                    half_extents: Vec3A::ONE, // 2x2x2 box
                },
            ))
            .id();

        // Duplicate in +X direction
        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);

        let created = action.created_entity().unwrap();
        let new_transform = world.get::<Transform>(created).unwrap();

        // Should be offset by 2.0 (full width) in X direction
        assert_eq!(new_transform.translation, Vec3::new(2.0, 0.0, 0.0));
    }

    #[test]
    fn test_duplicate_with_aabb_diagonal_direction() {
        let mut world = World::new();
        let original_pos = Vec3::ZERO;

        // Create entity with non-uniform AABB (4x2x6 box)
        let original = world
            .spawn((
                Transform::from_translation(original_pos),
                GlobalTransform::from_translation(original_pos),
                Aabb {
                    center: Vec3A::ZERO,
                    half_extents: Vec3A::new(2.0, 1.0, 3.0), // 4x2x6 box
                },
            ))
            .id();

        // Duplicate in +Y direction
        let mut action = DuplicateAction::new(original, Vec3::Y);
        action.apply(&mut world);

        let created = action.created_entity().unwrap();
        let new_transform = world.get::<Transform>(created).unwrap();

        // Should be offset by 2.0 (full height) in Y direction
        assert_eq!(new_transform.translation, Vec3::new(0.0, 2.0, 0.0));
    }

    #[test]
    fn test_duplicate_without_aabb_uses_direction() {
        let mut world = World::new();
        let original_pos = Vec3::new(1.0, 2.0, 3.0);
        let direction = Vec3::new(5.0, 0.0, 0.0);
        let original = world.spawn(Transform::from_translation(original_pos)).id();

        // No AABB, so direction is used as-is
        let mut action = DuplicateAction::new(original, direction);
        action.apply(&mut world);

        let created = action.created_entity().unwrap();
        let new_transform = world.get::<Transform>(created).unwrap();

        // Should be offset by the direction directly
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

        // Both entities should be at the same position (zero direction = zero offset)
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

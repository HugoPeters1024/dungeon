use bevy::{camera::primitives::Aabb, prelude::*};

use crate::merged_aabb::MergedAabb;

use super::Action;
use super::{TrashRoot, move_to_trash, restore_from_trash};

#[derive(Clone, Debug)]
pub struct DuplicateAction {
    pub entity: Entity,
    pub direction: Vec3,
    offset: Option<Vec3>,
    created_entity: Option<Entity>,
}

impl DuplicateAction {
    pub fn new(entity: Entity, direction: Vec3) -> Self {
        Self {
            entity,
            direction,
            offset: None,
            created_entity: None,
        }
    }

    pub fn created_entity(&self) -> Option<Entity> {
        self.created_entity
    }
}

fn compute_offset(world: &World, entity: Entity, direction: Vec3) -> Vec3 {
    let dir = direction.normalize_or_zero();
    if dir == Vec3::ZERO {
        return Vec3::ZERO;
    }

    let half_extents: Option<Vec3> = world
        .get::<MergedAabb>(entity)
        .map(|m| m.half_extents.into())
        .or_else(|| world.get::<Aabb>(entity).map(|a| a.half_extents.into()));

    match half_extents {
        Some(h) => {
            let extent = h.x * dir.x.abs() + h.y * dir.y.abs() + h.z * dir.z.abs();
            dir * extent * 2.0
        }
        None => direction,
    }
}

fn clone_subtree(world: &mut World, root: Entity) -> Entity {
    let mut stack = vec![root];
    let mut old_to_new = Vec::new();

    // Clone each entity keeping its ChildOf intact so that component hooks
    // (e.g. avian3d's physics transform init) see the correct parent hierarchy
    // and don't corrupt Transform with world-space Position values.
    // Deny Children to prevent the clone from adopting the original's children.
    while let Some(entity) = stack.pop() {
        let new = world
            .entity_mut(entity)
            .clone_and_spawn_with_opt_out(|builder| {
                builder.linked_cloning(false);
                builder.deny::<Children>();
            });
        old_to_new.push((entity, new));

        if let Some(children) = world.get::<Children>(entity) {
            for child in children.iter().rev() {
                stack.push(child);
            }
        }
    }

    // Reparent cloned children under their cloned parents.
    for i in 1..old_to_new.len() {
        let (original, cloned) = old_to_new[i];
        let original_parent = world.get::<ChildOf>(original).map(|c| c.parent());
        if let Some(parent) = original_parent {
            let cloned_parent = old_to_new
                .iter()
                .find(|(orig, _)| *orig == parent)
                .map(|(_, new)| *new);
            if let Some(cloned_parent) = cloned_parent {
                world.entity_mut(cloned).insert(ChildOf(cloned_parent));
            }
        }
    }

    old_to_new[0].1
}

impl Action for DuplicateAction {
    fn apply(&mut self, world: &mut World) {
        if let Some(existing) = self.created_entity {
            if let Ok(mut entity_mut) = world.get_entity_mut(existing) {
                restore_from_trash(&mut entity_mut);
            }
            return;
        }

        let offset = *self
            .offset
            .get_or_insert_with(|| compute_offset(world, self.entity, self.direction));

        let new_root = clone_subtree(world, self.entity);

        if let Some(mut transform) = world.get_mut::<Transform>(new_root) {
            transform.translation += offset;
        }

        self.created_entity = Some(new_root);
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
    use bevy::camera::primitives::Aabb;

    fn setup_trash(world: &mut World) -> Entity {
        let trash = world.spawn(TrashRootMarker).id();
        world.insert_resource(TrashRoot(trash));
        trash
    }

    #[test]
    fn creates_new_entity() {
        let mut world = World::new();
        let original = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);

        assert_eq!(world.query::<&Transform>().iter(&world).count(), 2);
        assert!(action.created_entity().is_some());
        assert_ne!(action.created_entity().unwrap(), original);
    }

    #[test]
    fn clones_components() {
        let mut world = World::new();
        let original = world
            .spawn((Name::new("thing"), Transform::from_xyz(1.0, 0.0, 0.0)))
            .id();

        let mut action = DuplicateAction::new(original, Vec3::ZERO);
        action.apply(&mut world);

        let created = action.created_entity().unwrap();
        assert_eq!(world.get::<Name>(created).unwrap().as_str(), "thing");
        assert!(world.get::<Transform>(created).is_some());
    }

    #[test]
    fn offsets_by_aabb_size() {
        let mut world = World::new();
        let original = world
            .spawn((
                Transform::IDENTITY,
                GlobalTransform::IDENTITY,
                Aabb {
                    center: Vec3A::ZERO,
                    half_extents: Vec3A::splat(1.0),
                },
            ))
            .id();

        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);

        let pos = world
            .get::<Transform>(action.created_entity().unwrap())
            .unwrap()
            .translation;
        assert_eq!(pos, Vec3::new(2.0, 0.0, 0.0));
    }

    #[test]
    fn prefers_merged_aabb_over_aabb() {
        let mut world = World::new();
        let original = world
            .spawn((
                Transform::IDENTITY,
                GlobalTransform::IDENTITY,
                Aabb {
                    center: Vec3A::ZERO,
                    half_extents: Vec3A::splat(1.0),
                },
                MergedAabb(Aabb {
                    center: Vec3A::ZERO,
                    half_extents: Vec3A::splat(3.0),
                }),
            ))
            .id();

        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);

        let pos = world
            .get::<Transform>(action.created_entity().unwrap())
            .unwrap()
            .translation;
        assert_eq!(pos, Vec3::new(6.0, 0.0, 0.0));
    }

    #[test]
    fn falls_back_to_raw_direction_without_aabb() {
        let mut world = World::new();
        let direction = Vec3::new(5.0, 0.0, 0.0);
        let original = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = DuplicateAction::new(original, direction);
        action.apply(&mut world);

        let pos = world
            .get::<Transform>(action.created_entity().unwrap())
            .unwrap()
            .translation;
        assert_eq!(pos, Vec3::new(1.0, 2.0, 3.0) + direction);
    }

    #[test]
    fn zero_direction_places_at_same_position() {
        let mut world = World::new();
        let original = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = DuplicateAction::new(original, Vec3::ZERO);
        action.apply(&mut world);

        let pos = world
            .get::<Transform>(action.created_entity().unwrap())
            .unwrap()
            .translation;
        assert_eq!(pos, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn preserves_original() {
        let mut world = World::new();
        let original = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = DuplicateAction::new(original, Vec3::X * 10.0);
        action.apply(&mut world);

        assert_eq!(
            world.get::<Transform>(original).unwrap().translation,
            Vec3::new(1.0, 2.0, 3.0)
        );
    }

    #[test]
    fn undo_moves_to_trash() {
        let mut world = World::new();
        let trash = setup_trash(&mut world);
        let original = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);
        action.revert(&mut world);

        let created = action.created_entity().unwrap();
        assert_eq!(world.get::<ChildOf>(created).unwrap().parent(), trash);
    }

    #[test]
    fn redo_restores_same_entity() {
        let mut world = World::new();
        setup_trash(&mut world);
        let original = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);
        let created = action.created_entity().unwrap();

        action.revert(&mut world);
        action.apply(&mut world);

        assert_eq!(action.created_entity().unwrap(), created);
        assert!(world.get::<ChildOf>(created).is_none());
    }

    #[test]
    fn redo_restores_parent_relationship() {
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

        assert_eq!(world.get::<ChildOf>(created).unwrap().parent(), parent);
    }

    #[test]
    fn undo_preserves_original() {
        let mut world = World::new();
        setup_trash(&mut world);
        let original = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

        let mut action = DuplicateAction::new(original, Vec3::X);
        action.apply(&mut world);
        action.revert(&mut world);

        assert_eq!(
            world.get::<Transform>(original).unwrap().translation,
            Vec3::new(1.0, 2.0, 3.0)
        );
    }

    #[test]
    fn clones_children_with_correct_hierarchy() {
        let mut world = World::new();
        let child_pos = Vec3::new(0.0, 5.0, 0.0);
        let parent = world
            .spawn(Transform::IDENTITY)
            .with_children(|p| {
                p.spawn(Transform::from_translation(child_pos));
            })
            .id();

        let mut action = DuplicateAction::new(parent, Vec3::ZERO);
        action.apply(&mut world);

        let created = action.created_entity().unwrap();
        let children: Vec<Entity> = world.get::<Children>(created).unwrap().iter().collect();
        assert_eq!(children.len(), 1);
        assert_eq!(
            world.get::<Transform>(children[0]).unwrap().translation,
            child_pos
        );
    }

    #[test]
    fn cloned_children_are_not_children_of_original() {
        let mut world = World::new();
        let parent = world
            .spawn(Transform::IDENTITY)
            .with_children(|p| {
                p.spawn(Transform::IDENTITY);
                p.spawn(Transform::IDENTITY);
            })
            .id();

        let mut action = DuplicateAction::new(parent, Vec3::ZERO);
        action.apply(&mut world);

        let created = action.created_entity().unwrap();
        let original_children: Vec<Entity> =
            world.get::<Children>(parent).unwrap().iter().collect();
        let cloned_children: Vec<Entity> =
            world.get::<Children>(created).unwrap().iter().collect();

        assert_eq!(original_children.len(), 2);
        assert_eq!(cloned_children.len(), 2);
        for child in &cloned_children {
            assert!(!original_children.contains(child));
        }
    }

    #[test]
    fn deep_hierarchy_is_fully_cloned() {
        let mut world = World::new();
        let root = world
            .spawn(Transform::IDENTITY)
            .with_children(|p| {
                p.spawn(Transform::IDENTITY).with_children(|p| {
                    p.spawn(Name::new("leaf"));
                });
            })
            .id();

        let mut action = DuplicateAction::new(root, Vec3::ZERO);
        action.apply(&mut world);

        let created = action.created_entity().unwrap();
        let child = world.get::<Children>(created).unwrap().iter().next().unwrap();
        let grandchild = world.get::<Children>(child).unwrap().iter().next().unwrap();
        assert_eq!(
            world.get::<Name>(grandchild).unwrap().as_str(),
            "leaf"
        );
    }
}

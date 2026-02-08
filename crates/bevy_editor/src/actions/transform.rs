use bevy::prelude::*;

use super::Action;

// === World-to-local transform utilities ===

fn world_position_to_local(world: &World, entity: Entity, world_position: Vec3) -> Vec3 {
    if let Some(child_of) = world.get::<ChildOf>(entity) {
        let parent = child_of.parent();
        if let Some(parent_global) = world.get::<GlobalTransform>(parent) {
            parent_global
                .affine()
                .inverse()
                .transform_point3(world_position)
        } else {
            world_position
        }
    } else {
        world_position
    }
}

fn world_scale_to_local(world: &World, entity: Entity, world_scale: Vec3) -> Vec3 {
    if let Some(child_of) = world.get::<ChildOf>(entity) {
        let parent = child_of.parent();
        if let Some(parent_global) = world.get::<GlobalTransform>(parent) {
            parent_global
                .affine()
                .inverse()
                .to_scale_rotation_translation()
                .0
                * world_scale
        } else {
            world_scale
        }
    } else {
        world_scale
    }
}

fn world_rotation_to_local(world: &World, entity: Entity, world_rotation: Quat) -> Quat {
    if let Some(child_of) = world.get::<ChildOf>(entity) {
        let parent = child_of.parent();
        if let Some(parent_global) = world.get::<GlobalTransform>(parent) {
            let (_, parent_rotation, _) = parent_global.to_scale_rotation_translation();
            parent_rotation.inverse() * world_rotation
        } else {
            world_rotation
        }
    } else {
        world_rotation
    }
}

/// The kind of transform change, used for naming
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformKind {
    Move,
    Scale,
    Rotate,
    Full,
}

impl TransformKind {
    fn verb(&self) -> &'static str {
        match self {
            TransformKind::Move => "move",
            TransformKind::Scale => "scale",
            TransformKind::Rotate => "rotate",
            TransformKind::Full => "transform",
        }
    }
}

/// A general transform action that can represent translation, rotation, scale, or any combination
#[derive(Clone, Debug)]
pub struct TransformAction {
    pub entity: Entity,
    pub old_transform: Transform,
    pub new_transform: Transform,
    kind: TransformKind,
}

impl TransformAction {
    /// Create a move action (translation only)
    pub fn move_entity(entity: Entity, old_position: Vec3, new_position: Vec3) -> Self {
        Self {
            entity,
            old_transform: Transform::from_translation(old_position),
            new_transform: Transform::from_translation(new_position),
            kind: TransformKind::Move,
        }
    }

    /// Create a scale action
    pub fn scale(entity: Entity, old_scale: Vec3, new_scale: Vec3) -> Self {
        Self {
            entity,
            old_transform: Transform::from_scale(old_scale),
            new_transform: Transform::from_scale(new_scale),
            kind: TransformKind::Scale,
        }
    }

    /// Create a rotation action
    pub fn rotate(entity: Entity, old_rotation: Quat, new_rotation: Quat) -> Self {
        Self {
            entity,
            old_transform: Transform::from_rotation(old_rotation),
            new_transform: Transform::from_rotation(new_rotation),
            kind: TransformKind::Rotate,
        }
    }

    /// Create a full transform action (translation, rotation, and scale)
    pub fn full(entity: Entity, old_transform: Transform, new_transform: Transform) -> Self {
        Self {
            entity,
            old_transform,
            new_transform,
            kind: TransformKind::Full,
        }
    }

    pub fn kind(&self) -> TransformKind {
        self.kind
    }
}

impl TransformAction {
    fn apply_transform(&self, world: &mut World, target: &Transform) {
        match self.kind {
            TransformKind::Move => {
                let local_position = world_position_to_local(world, self.entity, target.translation);
                if let Some(mut transform) = world.get_mut::<Transform>(self.entity) {
                    transform.translation = local_position;
                }
            }
            TransformKind::Scale => {
                let local_scale = world_scale_to_local(world, self.entity, target.scale);
                if let Some(mut transform) = world.get_mut::<Transform>(self.entity) {
                    transform.scale = local_scale;
                }
            }
            TransformKind::Rotate => {
                let local_rotation = world_rotation_to_local(world, self.entity, target.rotation);
                if let Some(mut transform) = world.get_mut::<Transform>(self.entity) {
                    transform.rotation = local_rotation;
                }
            }
            TransformKind::Full => {
                let local_translation = world_position_to_local(world, self.entity, target.translation);
                let local_rotation = world_rotation_to_local(world, self.entity, target.rotation);
                let local_scale = world_scale_to_local(world, self.entity, target.scale);
                if let Some(mut transform) = world.get_mut::<Transform>(self.entity) {
                    transform.translation = local_translation;
                    transform.rotation = local_rotation;
                    transform.scale = local_scale;
                }
            }
        }
    }
}

impl Action for TransformAction {
    fn apply(&mut self, world: &mut World) {
        self.apply_transform(world, &self.new_transform.clone());
    }

    fn revert(&mut self, world: &mut World) {
        self.apply_transform(world, &self.old_transform.clone());
    }

    fn name(&self) -> String {
        format!("{} {}", self.kind.verb(), self.entity)
    }
}

/// Transform multiple entities as a single action
#[derive(Clone, Debug)]
pub struct TransformSelectionAction {
    pub transforms: Vec<TransformAction>,
}

impl TransformSelectionAction {
    pub fn new(transforms: Vec<TransformAction>) -> Self {
        Self { transforms }
    }

    fn kind(&self) -> TransformKind {
        self.transforms.first().map(|t| t.kind).unwrap_or(TransformKind::Full)
    }
}

impl Action for TransformSelectionAction {
    fn apply(&mut self, world: &mut World) {
        for action in &mut self.transforms {
            action.apply(world);
        }
    }

    fn revert(&mut self, world: &mut World) {
        for action in &mut self.transforms {
            action.revert(world);
        }
    }

    fn name(&self) -> String {
        format!("{} selection ({})", self.kind().verb(), self.transforms.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    // === TransformAction::move_entity tests ===

    #[test]
    fn test_move_applies_new_position() {
        let mut world = World::new();
        let entity = world.spawn(Transform::from_xyz(0.0, 0.0, 0.0)).id();

        let mut action = TransformAction::move_entity(entity, Vec3::ZERO, Vec3::new(5.0, 5.0, 5.0));
        action.apply(&mut world);

        let transform = world.get::<Transform>(entity).unwrap();
        assert_eq!(transform.translation, Vec3::new(5.0, 5.0, 5.0));
    }

    #[test]
    fn test_move_undo_restores_position() {
        let mut world = World::new();
        let entity = world.spawn(Transform::from_xyz(0.0, 0.0, 0.0)).id();

        let mut action = TransformAction::move_entity(entity, Vec3::ZERO, Vec3::new(5.0, 5.0, 5.0));
        action.apply(&mut world);
        action.revert(&mut world);

        let transform = world.get::<Transform>(entity).unwrap();
        assert_eq!(transform.translation, Vec3::ZERO);
    }

    #[test]
    fn test_move_name() {
        let action = TransformAction::move_entity(Entity::PLACEHOLDER, Vec3::ZERO, Vec3::ONE);
        assert!(action.name().starts_with("move "));
    }

    #[test]
    fn test_move_preserves_other_components() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::splat(2.0)),
            ))
            .id();

        let mut action = TransformAction::move_entity(entity, Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0));
        action.apply(&mut world);

        let transform = world.get::<Transform>(entity).unwrap();
        assert_eq!(transform.translation, Vec3::new(5.0, 0.0, 0.0));
        assert_eq!(transform.scale, Vec3::splat(2.0)); // Scale unchanged
    }

    // === TransformAction::scale tests ===

    #[test]
    fn test_scale_applies_new_scale() {
        let mut world = World::new();
        let entity = world.spawn(Transform::from_scale(Vec3::ONE)).id();

        let mut action = TransformAction::scale(entity, Vec3::ONE, Vec3::splat(2.0));
        action.apply(&mut world);

        let transform = world.get::<Transform>(entity).unwrap();
        assert_eq!(transform.scale, Vec3::splat(2.0));
    }

    #[test]
    fn test_scale_undo_restores_scale() {
        let mut world = World::new();
        let entity = world.spawn(Transform::from_scale(Vec3::ONE)).id();

        let mut action = TransformAction::scale(entity, Vec3::ONE, Vec3::splat(2.0));
        action.apply(&mut world);
        action.revert(&mut world);

        let transform = world.get::<Transform>(entity).unwrap();
        assert_eq!(transform.scale, Vec3::ONE);
    }

    #[test]
    fn test_scale_name() {
        let action = TransformAction::scale(Entity::PLACEHOLDER, Vec3::ONE, Vec3::splat(2.0));
        assert!(action.name().starts_with("scale "));
    }

    #[test]
    fn test_scale_preserves_position() {
        let mut world = World::new();
        let entity = world
            .spawn(Transform::from_xyz(10.0, 20.0, 30.0).with_scale(Vec3::ONE))
            .id();

        let mut action = TransformAction::scale(entity, Vec3::ONE, Vec3::splat(3.0));
        action.apply(&mut world);

        let transform = world.get::<Transform>(entity).unwrap();
        assert_eq!(transform.scale, Vec3::splat(3.0));
        assert_eq!(transform.translation, Vec3::new(10.0, 20.0, 30.0)); // Position unchanged
    }

    #[test]
    fn test_scale_non_uniform() {
        let mut world = World::new();
        let entity = world.spawn(Transform::from_scale(Vec3::ONE)).id();

        let mut action = TransformAction::scale(entity, Vec3::ONE, Vec3::new(1.0, 2.0, 3.0));
        action.apply(&mut world);

        let transform = world.get::<Transform>(entity).unwrap();
        assert_eq!(transform.scale, Vec3::new(1.0, 2.0, 3.0));
    }

    // === TransformAction::rotate tests ===

    #[test]
    fn test_rotate_applies_new_rotation() {
        let mut world = World::new();
        let entity = world.spawn(Transform::from_rotation(Quat::IDENTITY)).id();

        let new_rotation = Quat::from_rotation_y(PI / 2.0);
        let mut action = TransformAction::rotate(entity, Quat::IDENTITY, new_rotation);
        action.apply(&mut world);

        let transform = world.get::<Transform>(entity).unwrap();
        assert!((transform.rotation.dot(new_rotation) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_rotate_undo_restores_rotation() {
        let mut world = World::new();
        let entity = world.spawn(Transform::from_rotation(Quat::IDENTITY)).id();

        let new_rotation = Quat::from_rotation_y(PI / 2.0);
        let mut action = TransformAction::rotate(entity, Quat::IDENTITY, new_rotation);
        action.apply(&mut world);
        action.revert(&mut world);

        let transform = world.get::<Transform>(entity).unwrap();
        assert!((transform.rotation.dot(Quat::IDENTITY) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_rotate_name() {
        let action = TransformAction::rotate(Entity::PLACEHOLDER, Quat::IDENTITY, Quat::IDENTITY);
        assert!(action.name().starts_with("rotate "));
    }

    // === TransformAction::full tests ===

    #[test]
    fn test_full_transform_applies_all() {
        let mut world = World::new();
        let entity = world.spawn(Transform::IDENTITY).id();

        let new_transform = Transform {
            translation: Vec3::new(1.0, 2.0, 3.0),
            rotation: Quat::from_rotation_z(PI / 4.0),
            scale: Vec3::splat(2.0),
        };

        let mut action = TransformAction::full(entity, Transform::IDENTITY, new_transform);
        action.apply(&mut world);

        let transform = world.get::<Transform>(entity).unwrap();
        assert_eq!(transform.translation, new_transform.translation);
        assert!((transform.rotation.dot(new_transform.rotation) - 1.0).abs() < 0.001);
        assert_eq!(transform.scale, new_transform.scale);
    }

    #[test]
    fn test_full_transform_undo_restores_all() {
        let mut world = World::new();
        let old_transform = Transform {
            translation: Vec3::new(10.0, 20.0, 30.0),
            rotation: Quat::from_rotation_x(PI / 6.0),
            scale: Vec3::splat(0.5),
        };
        let entity = world.spawn(old_transform).id();

        let new_transform = Transform {
            translation: Vec3::new(1.0, 2.0, 3.0),
            rotation: Quat::from_rotation_z(PI / 4.0),
            scale: Vec3::splat(2.0),
        };

        let mut action = TransformAction::full(entity, old_transform, new_transform);
        action.apply(&mut world);
        action.revert(&mut world);

        let transform = world.get::<Transform>(entity).unwrap();
        assert_eq!(transform.translation, old_transform.translation);
        assert!((transform.rotation.dot(old_transform.rotation) - 1.0).abs() < 0.001);
        assert_eq!(transform.scale, old_transform.scale);
    }

    #[test]
    fn test_full_transform_name() {
        let action = TransformAction::full(Entity::PLACEHOLDER, Transform::IDENTITY, Transform::IDENTITY);
        assert!(action.name().starts_with("transform "));
    }

    // === TransformSelectionAction tests ===

    #[test]
    fn test_selection_moves_multiple() {
        let mut world = World::new();
        let e1 = world.spawn(Transform::from_xyz(0.0, 0.0, 0.0)).id();
        let e2 = world.spawn(Transform::from_xyz(1.0, 1.0, 1.0)).id();

        let mut action = TransformSelectionAction::new(vec![
            TransformAction::move_entity(e1, Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)),
            TransformAction::move_entity(e2, Vec3::ONE, Vec3::new(11.0, 1.0, 1.0)),
        ]);

        action.apply(&mut world);

        assert_eq!(world.get::<Transform>(e1).unwrap().translation.x, 10.0);
        assert_eq!(world.get::<Transform>(e2).unwrap().translation.x, 11.0);
    }

    #[test]
    fn test_selection_undo_restores_all() {
        let mut world = World::new();
        let e1 = world.spawn(Transform::from_xyz(0.0, 0.0, 0.0)).id();
        let e2 = world.spawn(Transform::from_xyz(1.0, 1.0, 1.0)).id();

        let mut action = TransformSelectionAction::new(vec![
            TransformAction::move_entity(e1, Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)),
            TransformAction::move_entity(e2, Vec3::ONE, Vec3::new(11.0, 1.0, 1.0)),
        ]);

        action.apply(&mut world);
        action.revert(&mut world);

        assert_eq!(world.get::<Transform>(e1).unwrap().translation, Vec3::ZERO);
        assert_eq!(world.get::<Transform>(e2).unwrap().translation, Vec3::ONE);
    }

    #[test]
    fn test_selection_name_reflects_kind() {
        let action = TransformSelectionAction::new(vec![
            TransformAction::scale(Entity::PLACEHOLDER, Vec3::ONE, Vec3::splat(2.0)),
            TransformAction::scale(Entity::PLACEHOLDER, Vec3::ONE, Vec3::splat(3.0)),
        ]);
        assert!(action.name().starts_with("scale selection"));
    }

    #[test]
    fn test_selection_scales_multiple() {
        let mut world = World::new();
        let e1 = world.spawn(Transform::from_scale(Vec3::ONE)).id();
        let e2 = world.spawn(Transform::from_scale(Vec3::ONE)).id();

        let mut action = TransformSelectionAction::new(vec![
            TransformAction::scale(e1, Vec3::ONE, Vec3::splat(2.0)),
            TransformAction::scale(e2, Vec3::ONE, Vec3::splat(3.0)),
        ]);

        action.apply(&mut world);

        assert_eq!(world.get::<Transform>(e1).unwrap().scale, Vec3::splat(2.0));
        assert_eq!(world.get::<Transform>(e2).unwrap().scale, Vec3::splat(3.0));
    }

    #[test]
    fn test_empty_selection() {
        let mut world = World::new();
        let mut action = TransformSelectionAction::new(vec![]);

        action.apply(&mut world);
        action.revert(&mut world); // Should not panic
    }

    // === Kind tests ===

    #[test]
    fn test_kind_is_correct() {
        let move_action = TransformAction::move_entity(Entity::PLACEHOLDER, Vec3::ZERO, Vec3::ONE);
        assert_eq!(move_action.kind(), TransformKind::Move);

        let scale_action = TransformAction::scale(Entity::PLACEHOLDER, Vec3::ONE, Vec3::splat(2.0));
        assert_eq!(scale_action.kind(), TransformKind::Scale);

        let rotate_action = TransformAction::rotate(Entity::PLACEHOLDER, Quat::IDENTITY, Quat::IDENTITY);
        assert_eq!(rotate_action.kind(), TransformKind::Rotate);

        let full_action = TransformAction::full(Entity::PLACEHOLDER, Transform::IDENTITY, Transform::IDENTITY);
        assert_eq!(full_action.kind(), TransformKind::Full);
    }
}

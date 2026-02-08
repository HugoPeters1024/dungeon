use bevy::prelude::*;

use super::{Action, UndoFn, utils::world_scale_to_local};

/// Scale an entity (in world space)
#[derive(Clone, Debug)]
pub struct ScaleAction {
    pub entity: Entity,
    pub old_scale: Vec3,
    pub new_scale: Vec3,
}

impl Action for ScaleAction {
    fn apply(&self, world: &mut World) -> UndoFn {
        let local_scale = world_scale_to_local(world, self.entity, self.new_scale);

        if let Some(mut transform) = world.get_mut::<Transform>(self.entity) {
            transform.scale = local_scale;
        }

        let entity = self.entity;
        let old_scale = self.old_scale;
        Box::new(move |world: &mut World| {
            let local_scale = world_scale_to_local(world, entity, old_scale);
            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                transform.scale = local_scale;
            }
        })
    }

    fn name(&self) -> String {
        format!("scale {}", self.entity)
    }
}

/// Scale multiple entities as a single action
#[derive(Clone, Debug)]
pub struct ScaleSelectionAction {
    pub scales: Vec<ScaleAction>,
}

impl Action for ScaleSelectionAction {
    fn apply(&self, world: &mut World) -> UndoFn {
        let undo_fns: Vec<UndoFn> = self.scales.iter().map(|action| action.apply(world)).collect();

        Box::new(move |world: &mut World| {
            for undo_fn in undo_fns {
                undo_fn(world);
            }
        })
    }

    fn name(&self) -> String {
        format!("scale selection ({})", self.scales.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_action_applies_new_scale() {
        let mut world = World::new();
        let entity = world.spawn(Transform::from_scale(Vec3::ONE)).id();

        let action = ScaleAction {
            entity,
            old_scale: Vec3::ONE,
            new_scale: Vec3::splat(2.0),
        };

        let _undo = action.apply(&mut world);

        let transform = world.get::<Transform>(entity).unwrap();
        assert_eq!(transform.scale, Vec3::splat(2.0));
    }

    #[test]
    fn test_scale_action_undo_restores_scale() {
        let mut world = World::new();
        let entity = world.spawn(Transform::from_scale(Vec3::ONE)).id();

        let action = ScaleAction {
            entity,
            old_scale: Vec3::ONE,
            new_scale: Vec3::splat(2.0),
        };

        let undo = action.apply(&mut world);
        undo(&mut world);

        let transform = world.get::<Transform>(entity).unwrap();
        assert_eq!(transform.scale, Vec3::ONE);
    }
}

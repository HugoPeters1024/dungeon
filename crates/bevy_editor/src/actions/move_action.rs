use bevy::prelude::*;

use super::{Action, UndoFn, utils::world_position_to_local};

/// Move an entity from one position to another (in world space)
#[derive(Clone, Debug)]
pub struct MoveAction {
    pub entity: Entity,
    pub old_position: Vec3,
    pub new_position: Vec3,
}

impl Action for MoveAction {
    fn apply(&self, world: &mut World) -> UndoFn {
        let local_position = world_position_to_local(world, self.entity, self.new_position);

        if let Some(mut transform) = world.get_mut::<Transform>(self.entity) {
            transform.translation = local_position;
        }

        let entity = self.entity;
        let old_position = self.old_position;
        Box::new(move |world: &mut World| {
            let local_position = world_position_to_local(world, entity, old_position);
            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                transform.translation = local_position;
            }
        })
    }

    fn name(&self) -> String {
        format!("move {}", self.entity)
    }
}

/// Move multiple entities as a single action (world space)
#[derive(Clone, Debug)]
pub struct MoveSelectionAction {
    pub moves: Vec<MoveAction>,
}

impl Action for MoveSelectionAction {
    fn apply(&self, world: &mut World) -> UndoFn {
        let undo_fns: Vec<UndoFn> = self.moves.iter().map(|action| action.apply(world)).collect();

        Box::new(move |world: &mut World| {
            for undo_fn in undo_fns {
                undo_fn(world);
            }
        })
    }

    fn name(&self) -> String {
        format!("move selection ({})", self.moves.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_move_action_applies_new_position() {
        let mut world = World::new();
        let entity = world.spawn(Transform::from_xyz(0.0, 0.0, 0.0)).id();

        let action = MoveAction {
            entity,
            old_position: Vec3::ZERO,
            new_position: Vec3::new(5.0, 5.0, 5.0),
        };

        let _undo = action.apply(&mut world);

        let transform = world.get::<Transform>(entity).unwrap();
        assert_eq!(transform.translation, Vec3::new(5.0, 5.0, 5.0));
    }

    #[test]
    fn test_move_action_undo_restores_position() {
        let mut world = World::new();
        let entity = world.spawn(Transform::from_xyz(0.0, 0.0, 0.0)).id();

        let action = MoveAction {
            entity,
            old_position: Vec3::ZERO,
            new_position: Vec3::new(5.0, 5.0, 5.0),
        };

        let undo = action.apply(&mut world);
        undo(&mut world);

        let transform = world.get::<Transform>(entity).unwrap();
        assert_eq!(transform.translation, Vec3::ZERO);
    }

    #[test]
    fn test_move_selection_moves_multiple() {
        let mut world = World::new();
        let e1 = world.spawn(Transform::from_xyz(0.0, 0.0, 0.0)).id();
        let e2 = world.spawn(Transform::from_xyz(1.0, 1.0, 1.0)).id();

        let action = MoveSelectionAction {
            moves: vec![
                MoveAction {
                    entity: e1,
                    old_position: Vec3::ZERO,
                    new_position: Vec3::new(10.0, 0.0, 0.0),
                },
                MoveAction {
                    entity: e2,
                    old_position: Vec3::ONE,
                    new_position: Vec3::new(11.0, 1.0, 1.0),
                },
            ],
        };

        let _undo = action.apply(&mut world);

        assert_eq!(world.get::<Transform>(e1).unwrap().translation.x, 10.0);
        assert_eq!(world.get::<Transform>(e2).unwrap().translation.x, 11.0);
    }
}

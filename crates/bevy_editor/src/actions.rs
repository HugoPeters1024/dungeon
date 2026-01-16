use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::EditorCamera;

/// Represents an action that can be applied to the world.
/// Actions are queued and executed later, enabling undo/redo support.
#[derive(Clone, Debug)]
pub enum EditorAction {
    /// Duplicate an entity, offsetting it by the given normal vector
    Duplicate { entity: Entity, offset: Vec3 },
    /// Focus the editor camera on a specific world position
    FocusCameraOn {
        old_position: Vec3,
        new_position: Vec3,
    },
    /// Move an entity from one position to another (in world space)
    Move {
        entity: Entity,
        old_position: Vec3,
        new_position: Vec3,
    },
}

impl EditorAction {
    /// Apply this action to the world
    pub fn apply(&self, world: &mut World) {
        match self {
            EditorAction::Duplicate { entity, offset } => {
                Self::apply_duplicate(world, *entity, *offset);
            }
            EditorAction::FocusCameraOn { new_position, .. } => {
                Self::apply_focus_camera(world, *new_position);
            }
            EditorAction::Move {
                entity,
                new_position,
                ..
            } => {
                Self::apply_move(world, *entity, *new_position);
            }
        }
    }

    fn apply_duplicate(world: &mut World, entity: Entity, offset: Vec3) {
        // Clone the entity
        let new_entity = world.entity_mut(entity).clone_and_spawn();

        // Offset the new entity's transform
        if let Some(mut transform) = world.get_mut::<Transform>(new_entity) {
            transform.translation += offset;
        }
    }

    fn apply_focus_camera(world: &mut World, position: Vec3) {
        let mut query = world.query_filtered::<&mut PanOrbitCamera, With<EditorCamera>>();
        for mut pan_orbit in query.iter_mut(world) {
            pan_orbit.target_focus = position;
        }
    }

    fn apply_move(world: &mut World, entity: Entity, new_position: Vec3) {
        // Convert world position to local space, accounting for parent transform
        let local_position = if let Some(child_of) = world.get::<ChildOf>(entity) {
            let parent = child_of.parent();
            if let Some(parent_global) = world.get::<GlobalTransform>(parent) {
                parent_global
                    .affine()
                    .inverse()
                    .transform_point3(new_position)
            } else {
                new_position
            }
        } else {
            new_position
        };

        if let Some(mut transform) = world.get_mut::<Transform>(entity) {
            transform.translation = local_position;
        }
    }

    pub fn name(&self) -> String {
        match self {
            EditorAction::Duplicate { entity, .. } => format!("duplicate {}", entity),
            EditorAction::FocusCameraOn { .. } => format!("focus camera"),
            EditorAction::Move { entity, .. } => format!("move {}", entity),
        }
    }
}

/// Resource that holds a queue of actions to be executed
#[derive(Resource, Default)]
pub struct ActionQueue {
    pending: Vec<EditorAction>,
    /// History of applied actions (for future undo support)
    history: Vec<EditorAction>,
    /// Index into history for redo support (actions after this index can be redone)
    history_index: usize,
}

impl ActionQueue {
    /// Queue an action to be executed
    pub fn push(&mut self, action: EditorAction) {
        self.pending.push(action);
    }

    /// Take all pending actions, leaving the queue empty
    pub fn take_pending(&mut self) -> Vec<EditorAction> {
        std::mem::take(&mut self.pending)
    }

    /// Record an action in history (called after applying)
    pub fn record(&mut self, action: EditorAction) {
        // When a new action is recorded, truncate any redo history
        self.history.truncate(self.history_index);
        self.history.push(action);
        self.history_index = self.history.len();
    }

    /// Get the history of applied actions
    pub fn history(&self) -> &[EditorAction] {
        &self.history[..self.history_index]
    }

    pub fn history_tail(&self, n: usize) -> &[EditorAction] {
        &self.history[self.history_index.saturating_sub(n)..self.history_index]
    }
}

/// System that processes the action queue and applies pending actions
pub fn process_action_queue(world: &mut World) {
    // Extract pending actions
    let actions =
        world.resource_scope::<ActionQueue, Vec<EditorAction>>(|_, mut queue| queue.take_pending());

    // Apply each action
    for action in actions {
        action.apply(world);

        // Record in history
        world.resource_mut::<ActionQueue>().record(action);
    }
}

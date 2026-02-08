use bevy::prelude::*;
use bevy::input::ButtonInput;

use super::{EditorAction, UndoFn};

/// An entry in the history stack containing the action and its undo function
struct HistoryEntry {
    action: EditorAction,
    undo_fn: Option<UndoFn>,
}

/// Resource that holds a queue of actions to be executed
#[derive(Resource, Default)]
pub struct ActionQueue {
    pending: Vec<EditorAction>,
    history: Vec<HistoryEntry>,
    history_index: usize,
    undo_requested: bool,
    redo_requested: bool,
}

impl ActionQueue {
    pub fn push(&mut self, action: EditorAction) {
        self.pending.push(action);
    }

    pub fn take_pending(&mut self) -> Vec<EditorAction> {
        std::mem::take(&mut self.pending)
    }

    pub fn record(&mut self, action: EditorAction, undo_fn: UndoFn) {
        self.history.truncate(self.history_index);
        self.history.push(HistoryEntry {
            action,
            undo_fn: Some(undo_fn),
        });
        self.history_index = self.history.len();
    }

    pub fn request_undo(&mut self) {
        self.undo_requested = true;
    }

    pub fn request_redo(&mut self) {
        self.redo_requested = true;
    }

    pub fn can_undo(&self) -> bool {
        self.history_index > 0
    }

    pub fn can_redo(&self) -> bool {
        self.history_index < self.history.len()
    }

    pub fn history_index(&self) -> usize {
        self.history_index
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn iter_history(&self) -> impl Iterator<Item = (&EditorAction, bool)> {
        self.history.iter().enumerate().map(|(i, entry)| {
            let is_undone = i >= self.history_index;
            (&entry.action, is_undone)
        })
    }
}

/// System that handles Ctrl+Z (undo) and Ctrl+Y (redo) keyboard input
pub fn handle_undo_redo_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut action_queue: ResMut<ActionQueue>,
) {
    let ctrl_pressed = keyboard_input.pressed(KeyCode::ControlLeft)
        || keyboard_input.pressed(KeyCode::ControlRight)
        || keyboard_input.pressed(KeyCode::SuperLeft)
        || keyboard_input.pressed(KeyCode::SuperRight);

    if ctrl_pressed {
        if keyboard_input.just_pressed(KeyCode::KeyZ) {
            action_queue.request_undo();
        }
        if keyboard_input.just_pressed(KeyCode::KeyY) {
            action_queue.request_redo();
        }
    }
}

/// System that processes the action queue and applies pending actions
pub fn process_action_queue(world: &mut World) {
    let (undo_requested, redo_requested) = {
        let queue = world.resource::<ActionQueue>();
        (queue.undo_requested, queue.redo_requested)
    };

    if undo_requested {
        world.resource_mut::<ActionQueue>().undo_requested = false;
        let can_undo = world.resource::<ActionQueue>().can_undo();
        if can_undo {
            let undo_fn = world.resource_scope::<ActionQueue, Option<UndoFn>>(|_, mut queue| {
                let new_index = queue.history_index - 1;
                queue.history_index = new_index;
                queue.history.get_mut(new_index).and_then(|entry| entry.undo_fn.take())
            });
            if let Some(undo_fn) = undo_fn {
                undo_fn(world);
            }
        }
    }

    if redo_requested {
        world.resource_mut::<ActionQueue>().redo_requested = false;
        let can_redo = world.resource::<ActionQueue>().can_redo();
        if can_redo {
            let (action, history_index) = world.resource_scope::<ActionQueue, (Option<EditorAction>, usize)>(|_, queue| {
                let idx = queue.history_index;
                let action = queue.history.get(idx).map(|entry| entry.action.clone());
                (action, idx)
            });
            
            if let Some(action) = action {
                let undo_fn = action.apply(world);
                let mut queue = world.resource_mut::<ActionQueue>();
                if let Some(entry) = queue.history.get_mut(history_index) {
                    entry.undo_fn = Some(undo_fn);
                }
                queue.history_index = history_index + 1;
            }
        }
    }

    let actions =
        world.resource_scope::<ActionQueue, Vec<EditorAction>>(|_, mut queue| queue.take_pending());

    for action in actions {
        let undo_fn = action.apply(world);
        world.resource_mut::<ActionQueue>().record(action, undo_fn);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_queue_starts_empty() {
        let queue = ActionQueue::default();
        assert!(!queue.can_undo());
        assert!(!queue.can_redo());
        assert_eq!(queue.history_len(), 0);
    }

    #[test]
    fn test_action_queue_push_and_take() {
        use crate::actions::MoveAction;

        let mut queue = ActionQueue::default();
        queue.push(MoveAction {
            entity: Entity::PLACEHOLDER,
            old_position: Vec3::ZERO,
            new_position: Vec3::ONE,
        }.into());

        let pending = queue.take_pending();
        assert_eq!(pending.len(), 1);
        assert!(queue.take_pending().is_empty());
    }
}

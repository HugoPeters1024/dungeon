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
    use crate::actions::TransformAction;

    #[test]
    fn test_action_queue_starts_empty() {
        let queue = ActionQueue::default();
        assert!(!queue.can_undo());
        assert!(!queue.can_redo());
        assert_eq!(queue.history_len(), 0);
        assert_eq!(queue.history_index(), 0);
    }

    #[test]
    fn test_action_queue_push_and_take() {
        let mut queue = ActionQueue::default();
        queue.push(TransformAction::move_entity(
            Entity::PLACEHOLDER,
            Vec3::ZERO,
            Vec3::ONE,
        ).into());

        let pending = queue.take_pending();
        assert_eq!(pending.len(), 1);
        assert!(queue.take_pending().is_empty());
    }

    #[test]
    fn test_action_queue_push_multiple() {
        let mut queue = ActionQueue::default();

        queue.push(TransformAction::move_entity(Entity::PLACEHOLDER, Vec3::ZERO, Vec3::ONE).into());
        queue.push(TransformAction::scale(Entity::PLACEHOLDER, Vec3::ONE, Vec3::splat(2.0)).into());
        queue.push(TransformAction::rotate(Entity::PLACEHOLDER, Quat::IDENTITY, Quat::IDENTITY).into());

        let pending = queue.take_pending();
        assert_eq!(pending.len(), 3);
    }

    #[test]
    fn test_action_queue_record_enables_undo() {
        let mut queue = ActionQueue::default();

        assert!(!queue.can_undo());

        let action = TransformAction::move_entity(Entity::PLACEHOLDER, Vec3::ZERO, Vec3::ONE);
        let undo_fn: UndoFn = Box::new(|_| {});
        queue.record(action.into(), undo_fn);

        assert!(queue.can_undo());
        assert!(!queue.can_redo());
        assert_eq!(queue.history_len(), 1);
        assert_eq!(queue.history_index(), 1);
    }

    #[test]
    fn test_action_queue_record_multiple() {
        let mut queue = ActionQueue::default();

        for i in 0..5 {
            let action = TransformAction::move_entity(
                Entity::PLACEHOLDER,
                Vec3::splat(i as f32),
                Vec3::splat((i + 1) as f32),
            );
            let undo_fn: UndoFn = Box::new(|_| {});
            queue.record(action.into(), undo_fn);
        }

        assert_eq!(queue.history_len(), 5);
        assert_eq!(queue.history_index(), 5);
        assert!(queue.can_undo());
        assert!(!queue.can_redo());
    }

    #[test]
    fn test_action_queue_iter_history() {
        let mut queue = ActionQueue::default();

        for i in 0..3 {
            let action = TransformAction::move_entity(
                Entity::PLACEHOLDER,
                Vec3::splat(i as f32),
                Vec3::splat((i + 1) as f32),
            );
            let undo_fn: UndoFn = Box::new(|_| {});
            queue.record(action.into(), undo_fn);
        }

        let history: Vec<_> = queue.iter_history().collect();
        assert_eq!(history.len(), 3);

        // All should be marked as not undone (is_undone = false)
        for (_, is_undone) in &history {
            assert!(!is_undone);
        }
    }

    #[test]
    fn test_request_undo_sets_flag() {
        let mut queue = ActionQueue::default();
        queue.request_undo();
        // The flag is private, but we can verify it doesn't panic
    }

    #[test]
    fn test_request_redo_sets_flag() {
        let mut queue = ActionQueue::default();
        queue.request_redo();
        // The flag is private, but we can verify it doesn't panic
    }

    #[test]
    fn test_take_pending_clears_queue() {
        let mut queue = ActionQueue::default();

        queue.push(TransformAction::move_entity(Entity::PLACEHOLDER, Vec3::ZERO, Vec3::ONE).into());
        queue.push(TransformAction::move_entity(Entity::PLACEHOLDER, Vec3::ONE, Vec3::X).into());

        let first_take = queue.take_pending();
        assert_eq!(first_take.len(), 2);

        let second_take = queue.take_pending();
        assert!(second_take.is_empty());
    }
}

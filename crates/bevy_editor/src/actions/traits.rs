use bevy::prelude::*;

/// Trait for actions that can be applied and reverted.
/// Actions may store state (like created entity IDs) that persists across undo/redo cycles.
pub trait Action: Clone + std::fmt::Debug + Send + Sync + 'static {
    /// Apply the action to the world. May mutate self to store state needed for revert.
    fn apply(&mut self, world: &mut World);
    /// Revert the action. May mutate self to store state needed for re-apply.
    fn revert(&mut self, world: &mut World);
    /// Get a human-readable name for this action
    fn name(&self) -> String;
}

use bevy::prelude::*;

/// A boxed function that can undo an action
pub type UndoFn = Box<dyn FnOnce(&mut World) + Send + Sync>;

/// Trait for actions that can be applied to the world
pub trait Action: Clone + std::fmt::Debug + Send + Sync + 'static {
    /// Apply the action and return an undo function
    fn apply(&self, world: &mut World) -> UndoFn;
    fn name(&self) -> String;
}

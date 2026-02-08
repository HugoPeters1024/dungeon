mod traits;
mod duplicate;
mod focus_camera;
mod merge;
mod queue;
mod transform;

pub use traits::{Action, UndoFn};
pub use duplicate::DuplicateAction;
pub use focus_camera::FocusCameraAction;
pub use merge::MergeAction;
pub use queue::{ActionQueue, handle_undo_redo_input, process_action_queue};
pub use transform::{TransformAction, TransformSelectionAction, TransformKind};

use bevy::prelude::*;

// Type aliases for backwards compatibility
pub type MoveAction = TransformAction;
pub type MoveSelectionAction = TransformSelectionAction;
pub type ScaleAction = TransformAction;
pub type ScaleSelectionAction = TransformSelectionAction;

/// Represents an action that can be applied to the world.
/// Actions are queued and executed later, enabling undo/redo support.
#[derive(Clone, Debug)]
pub enum EditorAction {
    Duplicate(DuplicateAction),
    FocusCamera(FocusCameraAction),
    Transform(TransformAction),
    TransformSelection(TransformSelectionAction),
    Merge(MergeAction),
}

impl EditorAction {
    pub fn apply(&self, world: &mut World) -> UndoFn {
        match self {
            EditorAction::Duplicate(action) => action.apply(world),
            EditorAction::FocusCamera(action) => action.apply(world),
            EditorAction::Transform(action) => action.apply(world),
            EditorAction::TransformSelection(action) => action.apply(world),
            EditorAction::Merge(action) => action.apply(world),
        }
    }

    pub fn name(&self) -> String {
        match self {
            EditorAction::Duplicate(action) => action.name(),
            EditorAction::FocusCamera(action) => action.name(),
            EditorAction::Transform(action) => action.name(),
            EditorAction::TransformSelection(action) => action.name(),
            EditorAction::Merge(action) => action.name(),
        }
    }
}

impl From<DuplicateAction> for EditorAction {
    fn from(action: DuplicateAction) -> Self {
        EditorAction::Duplicate(action)
    }
}

impl From<FocusCameraAction> for EditorAction {
    fn from(action: FocusCameraAction) -> Self {
        EditorAction::FocusCamera(action)
    }
}

impl From<TransformAction> for EditorAction {
    fn from(action: TransformAction) -> Self {
        EditorAction::Transform(action)
    }
}

impl From<TransformSelectionAction> for EditorAction {
    fn from(action: TransformSelectionAction) -> Self {
        EditorAction::TransformSelection(action)
    }
}

impl From<MergeAction> for EditorAction {
    fn from(action: MergeAction) -> Self {
        EditorAction::Merge(action)
    }
}

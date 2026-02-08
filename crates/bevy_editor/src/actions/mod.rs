mod traits;
mod duplicate;
mod focus_camera;
mod merge;
mod move_action;
mod queue;
mod scale;
mod utils;

pub use traits::{Action, UndoFn};
pub use duplicate::DuplicateAction;
pub use focus_camera::FocusCameraAction;
pub use merge::MergeAction;
pub use move_action::{MoveAction, MoveSelectionAction};
pub use queue::{ActionQueue, handle_undo_redo_input, process_action_queue};
pub use scale::{ScaleAction, ScaleSelectionAction};

use bevy::prelude::*;

/// Represents an action that can be applied to the world.
/// Actions are queued and executed later, enabling undo/redo support.
#[derive(Clone, Debug)]
pub enum EditorAction {
    Duplicate(DuplicateAction),
    FocusCamera(FocusCameraAction),
    Move(MoveAction),
    MoveSelection(MoveSelectionAction),
    Scale(ScaleAction),
    ScaleSelection(ScaleSelectionAction),
    Merge(MergeAction),
}

impl EditorAction {
    pub fn apply(&self, world: &mut World) -> UndoFn {
        match self {
            EditorAction::Duplicate(action) => action.apply(world),
            EditorAction::FocusCamera(action) => action.apply(world),
            EditorAction::Move(action) => action.apply(world),
            EditorAction::MoveSelection(action) => action.apply(world),
            EditorAction::Scale(action) => action.apply(world),
            EditorAction::ScaleSelection(action) => action.apply(world),
            EditorAction::Merge(action) => action.apply(world),
        }
    }

    pub fn name(&self) -> String {
        match self {
            EditorAction::Duplicate(action) => action.name(),
            EditorAction::FocusCamera(action) => action.name(),
            EditorAction::Move(action) => action.name(),
            EditorAction::MoveSelection(action) => action.name(),
            EditorAction::Scale(action) => action.name(),
            EditorAction::ScaleSelection(action) => action.name(),
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

impl From<MoveAction> for EditorAction {
    fn from(action: MoveAction) -> Self {
        EditorAction::Move(action)
    }
}

impl From<MoveSelectionAction> for EditorAction {
    fn from(action: MoveSelectionAction) -> Self {
        EditorAction::MoveSelection(action)
    }
}

impl From<ScaleAction> for EditorAction {
    fn from(action: ScaleAction) -> Self {
        EditorAction::Scale(action)
    }
}

impl From<ScaleSelectionAction> for EditorAction {
    fn from(action: ScaleSelectionAction) -> Self {
        EditorAction::ScaleSelection(action)
    }
}

impl From<MergeAction> for EditorAction {
    fn from(action: MergeAction) -> Self {
        EditorAction::Merge(action)
    }
}

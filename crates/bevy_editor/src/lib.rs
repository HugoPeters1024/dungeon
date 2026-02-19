mod actions;
mod plugin;
mod prefabs;
mod scene;
mod state;
mod ui;
pub mod merged_aabb;

pub use actions::{
    Action, ActionQueue, DuplicateAction, EditorAction, FocusCameraAction, MergeAction,
    RemoveAction, SpawnPrefabAction, TransformAction, TransformSelectionAction, TransformKind,
    // Type aliases for backwards compatibility
    MoveAction, MoveSelectionAction, ScaleAction, ScaleSelectionAction,
};
pub use bevy_panorbit_camera::{self, PanOrbitCamera, TrackpadBehavior};
pub use plugin::{EditorCamera, EditorPlugin};
pub use prefabs::*;
pub use state::{
    AxisMask, ContextMenu, EguiWindow, HoverNormal, Selected, SelectedAction, UiDockState, UiState,
};

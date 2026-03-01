mod duplicate;
mod focus_camera;
mod merge;
mod queue;
mod remove;
mod spawn_prefab;
mod traits;
mod transform;

pub use duplicate::DuplicateAction;
pub use focus_camera::FocusCameraAction;
pub use merge::MergeAction;
pub use queue::{ActionQueue, handle_undo_redo_input, process_action_queue};
pub use remove::RemoveAction;
pub use spawn_prefab::SpawnPrefabAction;
pub use traits::Action;
pub use transform::{TransformAction, TransformKind, TransformSelectionAction};
pub(crate) use transform::{world_position_to_local_q, world_scale_to_local_q};

use bevy::prelude::*;

#[derive(Component)]
#[require(InheritedVisibility, Transform)]
pub struct TrashRootMarker;

#[derive(Resource)]
pub struct TrashRoot(pub Entity);

#[derive(Component)]
pub struct PreviousParent(Entity);

/// Restore an entity from the trash root, returning it to its previous parent
/// (or making it a root entity if it had no parent).
///
/// Re-inserts `Visibility` when removing `ChildOf` to trigger `Changed<Visibility>`,
/// because component removal doesn't fire `Changed<ChildOf>` and the visibility
/// propagation system would leave `InheritedVisibility` stale.
pub fn restore_from_trash(entity_mut: &mut EntityWorldMut) {
    if let Some(PreviousParent(parent)) = entity_mut.take::<PreviousParent>() {
        entity_mut.insert(ChildOf(parent));
    } else {
        entity_mut.remove::<ChildOf>();
        // Hack, re-insert to trigger recompute of InheritedVisibility,
        // can be removed if https://github.com/bevyengine/bevy/pull/23100 is merged
        if let Some(&vis) = entity_mut.get::<Visibility>() {
            entity_mut.insert(vis);
        }
    }
}

/// Move an entity to the trash root, saving its current parent (if any) so it
/// can be restored later.
pub fn move_to_trash(entity_mut: &mut EntityWorldMut, trash: Entity) {
    if let Some(parent) = entity_mut.get::<ChildOf>().map(|c| c.parent()) {
        if parent != trash {
            entity_mut.insert(PreviousParent(parent));
        }
    }
    entity_mut.insert(ChildOf(trash));
}

/// Represents an action that can be applied to the world.
/// Actions are queued and executed later, enabling undo/redo support.
#[derive(Clone, Debug)]
pub enum EditorAction {
    Duplicate(DuplicateAction),
    FocusCamera(FocusCameraAction),
    Transform(TransformAction),
    TransformSelection(TransformSelectionAction),
    Merge(MergeAction),
    Remove(RemoveAction),
    SpawnPrefab(SpawnPrefabAction),
}

impl EditorAction {
    pub fn apply(&mut self, world: &mut World) {
        match self {
            EditorAction::Duplicate(action) => action.apply(world),
            EditorAction::FocusCamera(action) => action.apply(world),
            EditorAction::Transform(action) => action.apply(world),
            EditorAction::TransformSelection(action) => action.apply(world),
            EditorAction::Merge(action) => action.apply(world),
            EditorAction::Remove(action) => action.apply(world),
            EditorAction::SpawnPrefab(action) => action.apply(world),
        }
    }

    pub fn revert(&mut self, world: &mut World) {
        match self {
            EditorAction::Duplicate(action) => action.revert(world),
            EditorAction::FocusCamera(action) => action.revert(world),
            EditorAction::Transform(action) => action.revert(world),
            EditorAction::TransformSelection(action) => action.revert(world),
            EditorAction::Merge(action) => action.revert(world),
            EditorAction::Remove(action) => action.revert(world),
            EditorAction::SpawnPrefab(action) => action.revert(world),
        }
    }

    pub fn name(&self) -> String {
        match self {
            EditorAction::Duplicate(action) => action.name(),
            EditorAction::FocusCamera(action) => action.name(),
            EditorAction::Transform(action) => action.name(),
            EditorAction::TransformSelection(action) => action.name(),
            EditorAction::Merge(action) => action.name(),
            EditorAction::Remove(action) => action.name(),
            EditorAction::SpawnPrefab(action) => action.name(),
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

impl From<RemoveAction> for EditorAction {
    fn from(action: RemoveAction) -> Self {
        EditorAction::Remove(action)
    }
}

impl From<SpawnPrefabAction> for EditorAction {
    fn from(action: SpawnPrefabAction) -> Self {
        EditorAction::SpawnPrefab(action)
    }
}

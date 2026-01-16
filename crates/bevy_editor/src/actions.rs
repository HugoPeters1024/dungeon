use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::EditorCamera;

/// Trait for actions that can be applied to the world
pub trait Action: Clone + std::fmt::Debug + Send + Sync + 'static {
    fn apply(&self, world: &mut World);
    fn name(&self) -> String;
}

/// Duplicate an entity, offsetting it by the given normal vector
#[derive(Clone, Debug)]
pub struct DuplicateAction {
    pub entity: Entity,
    pub offset: Vec3,
}

impl Action for DuplicateAction {
    fn apply(&self, world: &mut World) {
        // Clone the entity
        let new_entity = world.entity_mut(self.entity).clone_and_spawn();

        // Offset the new entity's transform
        if let Some(mut transform) = world.get_mut::<Transform>(new_entity) {
            transform.translation += self.offset;
        }
    }

    fn name(&self) -> String {
        format!("duplicate {}", self.entity)
    }
}

/// Focus the editor camera on a specific world position
#[derive(Clone, Debug)]
pub struct FocusCameraAction {
    pub old_position: Vec3,
    pub new_position: Vec3,
}

impl Action for FocusCameraAction {
    fn apply(&self, world: &mut World) {
        let mut query = world.query_filtered::<&mut PanOrbitCamera, With<EditorCamera>>();
        for mut pan_orbit in query.iter_mut(world) {
            pan_orbit.target_focus = self.new_position;
        }
    }

    fn name(&self) -> String {
        "focus camera".to_string()
    }
}

/// Move an entity from one position to another (in world space)
#[derive(Clone, Debug)]
pub struct MoveAction {
    pub entity: Entity,
    pub old_position: Vec3,
    pub new_position: Vec3,
}

impl Action for MoveAction {
    fn apply(&self, world: &mut World) {
        let local_position = world_position_to_local(world, self.entity, self.new_position);

        if let Some(mut transform) = world.get_mut::<Transform>(self.entity) {
            transform.translation = local_position;
        }
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
    fn apply(&self, world: &mut World) {
        for action in &self.moves {
            action.apply(world);
        }
    }

    fn name(&self) -> String {
        format!("move selection ({})", self.moves.len())
    }
}

#[derive(Clone, Debug)]
pub struct ScaleAction {
    pub entity: Entity,
    pub old_scale: Vec3,
    pub new_scale: Vec3,
}

impl Action for ScaleAction {
    fn apply(&self, world: &mut World) {
        let local_scale = world_scale_to_local(world, self.entity, self.new_scale);

        if let Some(mut transform) = world.get_mut::<Transform>(self.entity) {
            transform.scale = local_scale;
        }
    }

    fn name(&self) -> String {
        format!("scale {}", self.entity)
    }
}

/// Scale multiple entities as a single action
#[derive(Clone, Debug)]
pub struct ScaleSelectionAction {
    pub scales: Vec<ScaleAction>,
}

impl Action for ScaleSelectionAction {
    fn apply(&self, world: &mut World) {
        for action in &self.scales {
            action.apply(world);
        }
    }

    fn name(&self) -> String {
        format!("scale selection ({})", self.scales.len())
    }
}

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
}

impl EditorAction {
    pub fn apply(&self, world: &mut World) {
        match self {
            EditorAction::Duplicate(action) => action.apply(world),
            EditorAction::FocusCamera(action) => action.apply(world),
            EditorAction::Move(action) => action.apply(world),
            EditorAction::MoveSelection(action) => action.apply(world),
            EditorAction::Scale(action) => action.apply(world),
            EditorAction::ScaleSelection(action) => action.apply(world),
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

fn world_position_to_local(world: &World, entity: Entity, world_position: Vec3) -> Vec3 {
    if let Some(child_of) = world.get::<ChildOf>(entity) {
        let parent = child_of.parent();
        if let Some(parent_global) = world.get::<GlobalTransform>(parent) {
            parent_global
                .affine()
                .inverse()
                .transform_point3(world_position)
        } else {
            world_position
        }
    } else {
        world_position
    }
}

fn world_scale_to_local(world: &World, entity: Entity, world_scale: Vec3) -> Vec3 {
    if let Some(child_of) = world.get::<ChildOf>(entity) {
        let parent = child_of.parent();
        if let Some(parent_global) = world.get::<GlobalTransform>(parent) {
            parent_global
                .affine()
                .inverse()
                .to_scale_rotation_translation()
                .0
                * world_scale
        } else {
            world_scale
        }
    } else {
        world_scale
    }
}

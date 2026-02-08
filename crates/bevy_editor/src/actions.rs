use bevy::prelude::*;
use bevy::input::ButtonInput;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::{EditorCamera, PrefabId, Selected};

/// A boxed function that can undo an action
pub type UndoFn = Box<dyn FnOnce(&mut World) + Send + Sync>;

/// Trait for actions that can be applied to the world
pub trait Action: Clone + std::fmt::Debug + Send + Sync + 'static {
    /// Apply the action and return an undo function
    fn apply(&self, world: &mut World) -> UndoFn;
    fn name(&self) -> String;
}

/// Duplicate an entity, offsetting it by the given normal vector
#[derive(Clone, Debug)]
pub struct DuplicateAction {
    pub entity: Entity,
    pub offset: Vec3,
}

impl Action for DuplicateAction {
    fn apply(&self, world: &mut World) -> UndoFn {
        // Clone the entity recursively (including children)
        let new_entity = world
            .entity_mut(self.entity)
            .clone_and_spawn_with_opt_out(|builder| {
                builder.linked_cloning(true);
            });

        // Offset the new entity's transform
        if let Some(mut transform) = world.get_mut::<Transform>(new_entity) {
            transform.translation += self.offset;
        }

        // Return undo function that despawns the created entity
        Box::new(move |world: &mut World| {
            world.entity_mut(new_entity).despawn();
        })
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
    fn apply(&self, world: &mut World) -> UndoFn {
        let mut query = world.query_filtered::<&mut PanOrbitCamera, With<EditorCamera>>();
        for mut pan_orbit in query.iter_mut(world) {
            pan_orbit.target_focus = self.new_position;
        }

        let old_position = self.old_position;
        Box::new(move |world: &mut World| {
            let mut query = world.query_filtered::<&mut PanOrbitCamera, With<EditorCamera>>();
            for mut pan_orbit in query.iter_mut(world) {
                pan_orbit.target_focus = old_position;
            }
        })
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
    fn apply(&self, world: &mut World) -> UndoFn {
        let local_position = world_position_to_local(world, self.entity, self.new_position);

        if let Some(mut transform) = world.get_mut::<Transform>(self.entity) {
            transform.translation = local_position;
        }

        let entity = self.entity;
        let old_position = self.old_position;
        Box::new(move |world: &mut World| {
            let local_position = world_position_to_local(world, entity, old_position);
            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                transform.translation = local_position;
            }
        })
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
    fn apply(&self, world: &mut World) -> UndoFn {
        let undo_fns: Vec<UndoFn> = self.moves.iter().map(|action| action.apply(world)).collect();

        Box::new(move |world: &mut World| {
            for undo_fn in undo_fns {
                undo_fn(world);
            }
        })
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
    fn apply(&self, world: &mut World) -> UndoFn {
        let local_scale = world_scale_to_local(world, self.entity, self.new_scale);

        if let Some(mut transform) = world.get_mut::<Transform>(self.entity) {
            transform.scale = local_scale;
        }

        let entity = self.entity;
        let old_scale = self.old_scale;
        Box::new(move |world: &mut World| {
            let local_scale = world_scale_to_local(world, entity, old_scale);
            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                transform.scale = local_scale;
            }
        })
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
    fn apply(&self, world: &mut World) -> UndoFn {
        let undo_fns: Vec<UndoFn> = self.scales.iter().map(|action| action.apply(world)).collect();

        Box::new(move |world: &mut World| {
            for undo_fn in undo_fns {
                undo_fn(world);
            }
        })
    }

    fn name(&self) -> String {
        format!("scale selection ({})", self.scales.len())
    }
}

/// Merge multiple entities into a new parent entity
/// Removes PrefabId from each entity and creates a new parent with combined PrefabId
#[derive(Clone, Debug)]
pub struct MergeAction {
    pub entities: Vec<Entity>,
}

impl Action for MergeAction {
    fn apply(&self, world: &mut World) -> UndoFn {
        // Collect PrefabId names and world transforms from all entities
        let mut prefab_names: Vec<String> = Vec::new();
        let mut original_prefab_ids: Vec<(Entity, String)> = Vec::new();
        let mut world_transforms: Vec<(Entity, Transform)> = Vec::new();

        for &entity in &self.entities {
            if let Some(prefab_id) = world.get::<PrefabId>(entity) {
                let name = prefab_id.name().to_string();
                prefab_names.push(name.clone());
                original_prefab_ids.push((entity, name));
            }
            // Store the world-space transform for each entity
            if let Some(global_transform) = world.get::<GlobalTransform>(entity) {
                let (scale, rotation, translation) =
                    global_transform.to_scale_rotation_translation();
                world_transforms.push((
                    entity,
                    Transform {
                        translation,
                        rotation,
                        scale,
                    },
                ));
            }
        }

        // Store original world transforms for undo
        let original_world_transforms = world_transforms.clone();

        // Remove PrefabId from all entities
        for &entity in &self.entities {
            world.entity_mut(entity).remove::<PrefabId>();
        }

        // Create combined name
        let combined_name = prefab_names.join("-");

        // Compute average transform for the parent
        let count = world_transforms.len() as f32;
        let parent_transform = if count > 0.0 {
            let avg_translation = world_transforms
                .iter()
                .map(|(_, t)| t.translation)
                .sum::<Vec3>()
                / count;
            let avg_scale = world_transforms.iter().map(|(_, t)| t.scale).sum::<Vec3>() / count;
            // For rotation, use the first entity's rotation (averaging quaternions is complex)
            let avg_rotation = world_transforms
                .first()
                .map(|(_, t)| t.rotation)
                .unwrap_or(Quat::IDENTITY);
            Transform {
                translation: avg_translation,
                rotation: avg_rotation,
                scale: avg_scale,
            }
        } else {
            Transform::default()
        };

        // Spawn new parent entity with combined PrefabId
        // InheritedVisibility is needed for picking to work on the entity and its children
        let parent = world
            .spawn((
                PrefabId::new(combined_name),
                parent_transform,
                InheritedVisibility::default(),
            ))
            .add_children(&self.entities)
            .id();

        // Adjust each child's local transform so it stays at the same world position
        let parent_inverse = parent_transform.compute_affine().inverse();
        for (entity, world_transform) in world_transforms {
            let local_pos = parent_inverse.transform_point3(world_transform.translation);
            let local_rotation = parent_transform.rotation.inverse() * world_transform.rotation;
            let local_scale = world_transform.scale / parent_transform.scale;

            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                transform.translation = local_pos;
                transform.rotation = local_rotation;
                transform.scale = local_scale;
            }
        }

        // Select the newly created parent
        if let Some(mut selected) = world.get_resource_mut::<Selected>() {
            selected.set_single(parent);
        }

        // Return undo function
        let entities = self.entities.clone();
        Box::new(move |world: &mut World| {
            // Remove children from parent (this unparents them)
            for &entity in &entities {
                world.entity_mut(entity).remove_parent_in_place();
            }

            // Restore original world transforms
            for (entity, original_transform) in &original_world_transforms {
                if let Some(mut transform) = world.get_mut::<Transform>(*entity) {
                    transform.translation = original_transform.translation;
                    transform.rotation = original_transform.rotation;
                    transform.scale = original_transform.scale;
                }
            }

            // Restore original PrefabIds
            for (entity, name) in &original_prefab_ids {
                world.entity_mut(*entity).insert(PrefabId::new(name));
            }

            // Despawn the created parent entity
            world.entity_mut(parent).despawn();

            // Select the original entities
            if let Some(mut selected) = world.get_resource_mut::<Selected>() {
                if let Some(&first) = entities.first() {
                    selected.entities = entities.clone();
                    selected.set_single(first);
                }
            }
        })
    }

    fn name(&self) -> String {
        format!("merge {} entities", self.entities.len())
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

/// An entry in the history stack containing the action and its undo function
struct HistoryEntry {
    action: EditorAction,
    undo_fn: Option<UndoFn>,
}

/// Resource that holds a queue of actions to be executed
#[derive(Resource, Default)]
pub struct ActionQueue {
    pending: Vec<EditorAction>,
    /// History of applied actions (for undo/redo support)
    history: Vec<HistoryEntry>,
    /// Index into history - actions before this index have been applied
    history_index: usize,
    /// Flag to request undo
    undo_requested: bool,
    /// Flag to request redo
    redo_requested: bool,
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

    /// Record an action in history with its undo function (called after applying)
    pub fn record(&mut self, action: EditorAction, undo_fn: UndoFn) {
        // When a new action is recorded, truncate any redo history
        self.history.truncate(self.history_index);
        self.history.push(HistoryEntry {
            action,
            undo_fn: Some(undo_fn),
        });
        self.history_index = self.history.len();
    }

    /// Request an undo operation (will be processed in process_action_queue)
    pub fn request_undo(&mut self) {
        self.undo_requested = true;
    }

    /// Request a redo operation (will be processed in process_action_queue)
    pub fn request_redo(&mut self) {
        self.redo_requested = true;
    }

    /// Check if undo is possible
    pub fn can_undo(&self) -> bool {
        self.history_index > 0
    }

    /// Check if redo is possible
    pub fn can_redo(&self) -> bool {
        self.history_index < self.history.len()
    }

    /// Get the current history index
    pub fn history_index(&self) -> usize {
        self.history_index
    }

    /// Get the total history length
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Iterate over history entries for display
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
    // Check for undo/redo requests first
    let (undo_requested, redo_requested) = {
        let queue = world.resource::<ActionQueue>();
        (queue.undo_requested, queue.redo_requested)
    };

    if undo_requested {
        world.resource_mut::<ActionQueue>().undo_requested = false;
        let can_undo = world.resource::<ActionQueue>().can_undo();
        if can_undo {
            // Take the undo function out of the history entry
            let undo_fn = world.resource_scope::<ActionQueue, Option<UndoFn>>(|_, mut queue| {
                let new_index = queue.history_index - 1;
                queue.history_index = new_index;
                queue.history.get_mut(new_index).and_then(|entry| entry.undo_fn.take())
            });
            // Execute the undo function
            if let Some(undo_fn) = undo_fn {
                undo_fn(world);
            }
        }
    }

    if redo_requested {
        world.resource_mut::<ActionQueue>().redo_requested = false;
        let can_redo = world.resource::<ActionQueue>().can_redo();
        if can_redo {
            // Get the action to redo and apply it
            let (action, history_index) = world.resource_scope::<ActionQueue, (Option<EditorAction>, usize)>(|_, queue| {
                let idx = queue.history_index;
                let action = queue.history.get(idx).map(|entry| entry.action.clone());
                (action, idx)
            });
            
            if let Some(action) = action {
                let undo_fn = action.apply(world);
                // Store the new undo function and increment index
                let mut queue = world.resource_mut::<ActionQueue>();
                if let Some(entry) = queue.history.get_mut(history_index) {
                    entry.undo_fn = Some(undo_fn);
                }
                queue.history_index = history_index + 1;
            }
        }
    }

    // Extract pending actions
    let actions =
        world.resource_scope::<ActionQueue, Vec<EditorAction>>(|_, mut queue| queue.take_pending());

    // Apply each action
    for action in actions {
        let undo_fn = action.apply(world);

        // Record in history
        world.resource_mut::<ActionQueue>().record(action, undo_fn);
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

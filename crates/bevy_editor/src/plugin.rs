use std::sync::Mutex;
use std::time::Duration;

use bevy::camera::Viewport;
use bevy::camera::visibility::RenderLayers;
use bevy::color::palettes::tailwind::{PINK_100, RED_500};
use bevy::picking::pointer::PointerInteraction;
use bevy::picking::prelude::Pickable;
use bevy::prelude::*;
use bevy::{ecs::schedule::BoxedCondition, window::PrimaryWindow};
use bevy_egui::prelude::*;
use bevy_panorbit_camera::PanOrbitCameraPlugin;

use bevy_panorbit_camera::PanOrbitCamera;

use crate::actions::{
    ActionQueue, FocusCameraAction, RemoveAction, TransformAction, TransformSelectionAction,
    TrashRoot, TrashRootMarker, handle_undo_redo_input, process_action_queue,
};
use crate::editor_camera::{AxisAlignedProjectionState, sync_axis_aligned_projection};
use crate::state::{AxisMask, UiDockState, UiState};
use crate::{ContextMenu, HoverNormal, PrefabPlugin, Selected, SelectedAction};

const CLICK_DURATION: Duration = Duration::from_millis(500);

#[derive(Component)]
pub struct EditorCamera;

#[derive(Resource, Default)]
struct EditorEnabled(bool);

#[derive(Resource)]
struct EditorCondition {
    system: BoxedCondition,
}

#[derive(Default)]
pub struct EditorPlugin {
    condition: Mutex<Option<BoxedCondition>>,
}

impl EditorPlugin {
    pub fn new() -> Self {
        Self::default()
    }

    /// Only show the UI of the specified condition is active
    pub fn run_if<M>(mut self, condition: impl SystemCondition<M>) -> Self {
        let condition_system = IntoSystem::into_system(condition);
        self.condition = Mutex::new(Some(Box::new(condition_system) as BoxedCondition));
        self
    }
}

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crate::asset_ref::AssetRefPlugin);
        app.add_plugins(PrefabPlugin);
        app.add_plugins(crate::scene::ScenePlugin);
        app.add_plugins(crate::merged_aabb::MergedAabbPlugin);
        let trash_root = app
            .world_mut()
            .spawn((
                Name::new("Trash"),
                TrashRootMarker,
                Visibility::Hidden,
                InheritedVisibility::HIDDEN,
            ))
            .id();
        app.insert_resource(TrashRoot(trash_root));

        if !app.is_plugin_added::<MeshPickingPlugin>() {
            app.add_plugins(MeshPickingPlugin);
        }

        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(bevy_egui::EguiPlugin::default());
        }

        if !app.is_plugin_added::<bevy_inspector_egui::DefaultInspectorConfigPlugin>() {
            app.add_plugins(bevy_inspector_egui::DefaultInspectorConfigPlugin);
        }

        app.add_plugins(PanOrbitCameraPlugin);
        app.init_resource::<ActionQueue>();

        // Initialize EditorEnabled based on whether a custom condition was provided
        let has_custom_condition = self.condition.lock().unwrap().is_some();
        app.insert_resource(EditorEnabled(!has_custom_condition));

        if let Some(mut condition) = self.condition.lock().unwrap().take() {
            let world = app.world_mut();
            condition.initialize(world);
            world.insert_resource(EditorCondition { system: condition });
            app.add_systems(PreUpdate, evaluate_editor_condition);
        }
        app.add_systems(Startup, setup_ui);
        app.add_observer(on_click_in_void);
        app.add_observer(on_click_object);

        app.insert_resource(UiDockState::initialize());
        app.insert_resource(UiState::new());
        app.init_resource::<AxisAlignedProjectionState>();

        app.add_systems(
            Update,
            (
                draw_axes,
                draw_aabb,
                set_hover_normal,
                handle_selected_action_keys,
                handle_grab_mode_movement,
                handle_scale_mode_movement,
            )
                .run_if(resource_exists::<Selected>)
                .run_if(editor_enabled),
        );
        app.add_systems(Update, sync_axis_aligned_projection.run_if(editor_enabled));

        app.add_systems(
            Update,
            (handle_undo_redo_input, process_action_queue)
                .chain()
                .run_if(editor_enabled),
        );

        {
            let mut system = show_ui_system.into_configs();
            let condition = self.condition.lock().unwrap().take();
            if let Some(condition) = condition {
                system.run_if_dyn(condition);
            }

            app.add_systems(EguiPrimaryContextPass, system.run_if(editor_enabled));
        }
    }
}

fn setup_ui(mut commands: Commands, mut egui_global_settings: ResMut<EguiGlobalSettings>) {
    egui_global_settings.auto_create_primary_context = false;

    // egui camera
    commands.spawn((
        Camera2d,
        Name::new("Egui Camera"),
        PrimaryEguiContext,
        RenderLayers::none(),
        Pickable::IGNORE, // Make egui camera ignore picking events so they propagate to entities behind it
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
    ));
}

fn set_camera_viewport(
    ui_state: Res<UiState>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<&mut Camera, With<EditorCamera>>,
    egui_settings: Single<&EguiContextSettings>,
) {
    let Ok(window) = window.single() else {
        return;
    };
    for mut camera in cameras.iter_mut() {
        let scale_factor = window.scale_factor() * egui_settings.scale_factor;

        let viewport_pos = ui_state.viewport.left_top().to_vec2() * scale_factor;
        let viewport_size = ui_state.viewport.size() * scale_factor;

        let physical_position = UVec2::new(viewport_pos.x as u32, viewport_pos.y as u32);
        let physical_size = UVec2::new(viewport_size.x as u32, viewport_size.y as u32);

        let rect = physical_position + physical_size;

        let window_size = window.physical_size();
        if rect.x <= window_size.x && rect.y <= window_size.y {
            camera.viewport = Some(Viewport {
                physical_position,
                physical_size,
                depth: 0.0..1.0,
            });
        }
    }
}

fn show_ui_system(world: &mut World) -> Result {
    let egui_context = world
        .query_filtered::<&mut EguiContext, With<PrimaryEguiContext>>()
        .single(world)?;
    let mut egui_context = egui_context.clone();

    world.resource_scope::<UiState, _>(|world, mut ui_state| {
        ui_state.ui(world, egui_context.get_mut())
    });

    world.run_system_cached(set_camera_viewport)?;

    Ok(())
}

fn on_click_in_void(
    trigger: On<Pointer<Click>>,
    mut commands: Commands,
    mut ui_state: ResMut<UiState>,
    windows: Query<&Window>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut selected: Option<ResMut<Selected>>,
    global_transforms: Query<&GlobalTransform>,
    mut action_queue: ResMut<ActionQueue>,
    editor_enabled: Res<EditorEnabled>,
) {
    if !editor_enabled.0 {
        return;
    }
    if !ui_state.pointer_in_viewport
        || ui_state.egui_wants_pointer_input
        || trigger.duration > CLICK_DURATION
        || !windows.contains(trigger.event_target())
    {
        return;
    }
    let is_performing_action = selected.as_ref().is_some_and(|s| s.action.is_some());
    let is_primary = trigger.button == PointerButton::Primary;
    let is_secondary = trigger.button == PointerButton::Secondary;
    let hover_normal = selected.as_ref().and_then(|s| s.hover_normal.clone());
    let shift_pressed = shift_is_pressed(&keyboard_input);

    if is_primary {
        ui_state.context_menu = ContextMenu::Closed;
        if let Some(selected) = selected.as_mut() {
            if finalize_action_if_active(selected, &global_transforms, &mut action_queue) {
                return;
            }
            if !shift_pressed && selected.action.is_none() {
                commands.remove_resource::<Selected>();
            }
        }
    }

    if is_secondary
        && let Some(hover_normal) = hover_normal
        && !is_performing_action
    {
        ui_state.context_menu = ContextMenu::Open {
            window_location: trigger.pointer_location.position,
            hover_normal,
        }
    }
}

fn on_click_object(
    mut trigger: On<Pointer<Click>>,
    mut commands: Commands,
    mut ui_state: ResMut<UiState>,
    mut selected: Option<ResMut<Selected>>,
    parents: Query<&ChildOf>,
    windows: Query<&Window>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    global_transforms: Query<&GlobalTransform>,
    mut action_queue: ResMut<ActionQueue>,
    editor_enabled: Res<EditorEnabled>,
) {
    if !editor_enabled.0 {
        return;
    }
    if !ui_state.pointer_in_viewport
        || ui_state.egui_wants_pointer_input
        || trigger.duration > CLICK_DURATION
        || windows.contains(trigger.event_target())
    {
        return;
    }
    let is_performing_action = selected.as_ref().is_some_and(|s| s.action.is_some());
    let is_primary = trigger.button == PointerButton::Primary;
    let is_secondary = trigger.button == PointerButton::Secondary;
    let hover_normal = selected.as_ref().and_then(|s| s.hover_normal.clone());
    let shift_pressed = shift_is_pressed(&keyboard_input);

    trigger.propagate(false);

    if is_primary {
        ui_state.context_menu = ContextMenu::Closed;
        if let Some(selected) = selected.as_mut()
            && finalize_action_if_active(selected, &global_transforms, &mut action_queue)
        {
            return;
        }

        let clicked_entity = parents
            .iter_ancestors(trigger.event_target())
            .last()
            .unwrap_or(trigger.event_target());
        match selected.as_mut() {
            Some(selected) => {
                selected.action = None;
                if shift_pressed {
                    let has_selection = selected.toggle(clicked_entity);
                    if !has_selection {
                        commands.remove_resource::<Selected>();
                    }
                } else {
                    selected.set_single(clicked_entity);
                }
            }
            None => {
                commands.insert_resource(Selected::new(clicked_entity));
            }
        }
    }

    if let Some(hover_normal) = hover_normal
        && is_secondary
        && !is_performing_action
    {
        ui_state.context_menu = ContextMenu::Open {
            window_location: trigger.pointer_location.position,
            hover_normal,
        }
    }
}

fn draw_axes(mut gizmos: Gizmos, query: Query<&GlobalTransform>, selected: Res<Selected>) {
    for entity in selected.entities.iter().copied() {
        if let Ok(transform) = query.get(entity) {
            gizmos.axes(*transform, 1.5);
        }
    }
}

fn draw_aabb(
    mut gizmos: Gizmos,
    query: Query<&crate::merged_aabb::MergedAabb>,
    selected: Res<Selected>,
) {
    for entity in selected.entities.iter().copied() {
        if let Ok(merged) = query.get(entity) {
            let center: Vec3 = merged.center.into();
            let size: Vec3 = Vec3::from(merged.half_extents) * 2.0;
            let aabb_transform =
                GlobalTransform::from(Transform::from_translation(center).with_scale(size));
            gizmos.cube(aabb_transform, PINK_100);
        }
    }
}

/// Draws normals at the mouse hover position, but only if the hovered entity
/// is the currently selected entity or one of its descendants.
fn set_hover_normal(
    pointers: Query<&PointerInteraction>,
    mut selected: ResMut<Selected>,
    mut gizmos: Gizmos,
    children_query: Query<&Children>,
) {
    let selected_entity = selected.primary();
    selected.hover_normal = None;

    // Check if an entity is the selected entity or a descendant of it
    let is_selected_or_descendant = |entity: Entity| -> bool {
        if entity == selected_entity {
            return true;
        }
        children_query
            .iter_descendants(selected_entity)
            .any(|descendant| descendant == entity)
    };

    for (point, normal) in pointers
        .iter()
        .filter_map(|interaction| interaction.get_nearest_hit())
        .filter(|(entity, _hit)| is_selected_or_descendant(*entity))
        .filter_map(|(_entity, hit)| hit.position.zip(hit.normal))
    {
        if selected.action.is_none() {
            selected.hover_normal = Some(HoverNormal { point, normal });
        }
    }

    if let Some(hover_normal) = selected.hover_normal.as_ref() {
        gizmos.sphere(hover_normal.point, 0.05, RED_500);
        gizmos.arrow(
            hover_normal.point,
            hover_normal.point + hover_normal.normal * 0.5,
            PINK_100,
        );
    }
}

fn handle_selected_action_keys(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut transforms: Query<&mut Transform>,
    parents: Query<&ChildOf>,
    parent_globals: Query<&GlobalTransform>,
    mut selected: ResMut<Selected>,
    global_transforms: Query<&GlobalTransform>,
    mut action_queue: ResMut<ActionQueue>,
    camera_query: Query<&PanOrbitCamera, With<EditorCamera>>,
) {
    let primary = selected.primary();

    // F key: Focus camera on selected object
    if keyboard_input.just_pressed(KeyCode::KeyF)
        && let Ok(global_transform) = global_transforms.get(primary)
    {
        let old_position = camera_query
            .iter()
            .next()
            .map(|cam| cam.target_focus)
            .unwrap_or(Vec3::ZERO);

        action_queue.push(
            FocusCameraAction {
                old_position,
                new_position: global_transform.translation(),
            }
            .into(),
        );
    }

    match &mut selected.action {
        None if keyboard_input.just_pressed(KeyCode::KeyG) => {
            let initial_world_positions =
                collect_world_positions(&selected.entities, &global_transforms);
            let Some((_, initial_primary_pos)) = initial_world_positions
                .iter()
                .find(|(entity, _)| *entity == primary)
            else {
                return;
            };
            // Store the world positions for grab calculations
            selected.action = Some(SelectedAction::Grab {
                mask: None,
                initial_primary_pos: *initial_primary_pos,
                initial_world_positions,
            });
        }
        None if keyboard_input.just_pressed(KeyCode::KeyS) => {
            let initial_world_scales = collect_world_scales(&selected.entities, &global_transforms);
            if !initial_world_scales
                .iter()
                .any(|(entity, _)| *entity == primary)
            {
                return;
            }
            // Store the world scale for scale calculations
            selected.action = Some(SelectedAction::Scale {
                mask: None,
                initial_cursor_pos: None,
                initial_world_scales,
            });
        }
        None if keyboard_input.just_pressed(KeyCode::KeyX) => {
            // Remove all selected entities
            for &entity in &selected.entities {
                action_queue.push(RemoveAction::new(entity).into());
            }
        }
        None => {}
        Some(SelectedAction::Grab {
            mask,
            initial_world_positions,
            ..
        }) => {
            if keyboard_input.just_pressed(KeyCode::KeyX) {
                *mask = Some(AxisMask::X);
            }
            if keyboard_input.just_pressed(KeyCode::KeyY) {
                *mask = Some(AxisMask::Y);
            }
            if keyboard_input.just_pressed(KeyCode::KeyZ) {
                *mask = Some(AxisMask::Z);
            }

            if keyboard_input.just_pressed(KeyCode::Escape) {
                for (entity, initial_world_pos) in initial_world_positions.iter().copied() {
                    let local_pos = world_position_to_local(
                        entity,
                        initial_world_pos,
                        &parents,
                        &parent_globals,
                    );
                    if let Ok(mut transform) = transforms.get_mut(entity) {
                        transform.translation = local_pos;
                    }
                }
                selected.action = None;
            }
        }
        Some(SelectedAction::Scale {
            mask,
            initial_cursor_pos: _,
            initial_world_scales,
            ..
        }) => {
            if keyboard_input.just_pressed(KeyCode::KeyX) {
                *mask = Some(AxisMask::X);
            }
            if keyboard_input.just_pressed(KeyCode::KeyY) {
                *mask = Some(AxisMask::Y);
            }
            if keyboard_input.just_pressed(KeyCode::KeyZ) {
                *mask = Some(AxisMask::Z);
            }

            if keyboard_input.just_pressed(KeyCode::Escape) {
                for (entity, initial_world_scale) in initial_world_scales.iter().copied() {
                    let local_scale = world_scale_to_local(
                        entity,
                        initial_world_scale,
                        &parents,
                        &parent_globals,
                    );
                    if let Ok(mut transform) = transforms.get_mut(entity) {
                        transform.scale = local_scale;
                    }
                }
                selected.action = None;
            }
        }
    }
}

fn handle_grab_mode_movement(
    ui: Res<UiState>,
    mut transforms: Query<&mut Transform>,
    parents: Query<&ChildOf>,
    parent_globals: Query<&GlobalTransform>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut selected: ResMut<Selected>,
) {
    if !ui.pointer_in_viewport {
        return;
    }

    let Some(SelectedAction::Grab {
        mask,
        initial_primary_pos,
        initial_world_positions,
        ..
    }) = &mut selected.action
    else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    // Use Bevy's built-in viewport_to_world to get a ray from camera through cursor
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_pos) else {
        return;
    };

    let camera_forward = camera_transform.forward();

    // Define plane perpendicular to camera forward, passing through initial entity position (world space)
    // This keeps the object at a constant "depth" from the camera
    let plane_normal = *camera_forward;
    let plane_point = *initial_primary_pos;

    // Ray-plane intersection (in world space)
    let denominator = ray.direction.dot(plane_normal);
    if denominator.abs() < 1e-6 {
        return;
    }

    let t = (plane_point - ray.origin).dot(plane_normal) / denominator;
    let intersection = ray.origin + *ray.direction * t;

    // Apply axis mask in world space
    let new_primary_world_pos = if let Some(axis) = &mask {
        match axis {
            AxisMask::X => initial_primary_pos.with_x(intersection.x),
            AxisMask::Y => initial_primary_pos.with_y(intersection.y),
            AxisMask::Z => initial_primary_pos.with_z(intersection.z),
        }
    } else {
        intersection
    };

    let delta_world = new_primary_world_pos - *initial_primary_pos;

    for (entity, initial_world_pos) in initial_world_positions.iter().copied() {
        let new_world_pos = initial_world_pos + delta_world;
        let new_local_pos =
            world_position_to_local(entity, new_world_pos, &parents, &parent_globals);
        if let Ok(mut transform) = transforms.get_mut(entity) {
            transform.translation = new_local_pos;
        }
    }
}

fn handle_scale_mode_movement(
    ui: Res<UiState>,
    mut transforms: Query<&mut Transform>,
    global_transforms: Query<&GlobalTransform>,
    parents: Query<&ChildOf>,
    parent_globals: Query<&GlobalTransform>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut selected: ResMut<Selected>,
) {
    if !ui.pointer_in_viewport {
        return;
    }

    let primary = selected.primary();

    let Some(SelectedAction::Scale {
        mask,
        initial_cursor_pos,
        initial_world_scales,
        ..
    }) = &mut selected.action
    else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    // Initialize the initial cursor position on first frame
    let initial_cursor = match initial_cursor_pos {
        Some(pos) => *pos,
        None => {
            *initial_cursor_pos = Some(cursor_pos);
            return;
        }
    };

    // Get the entity's world position for the reference plane
    let entity_world_pos = global_transforms
        .get(primary)
        .map(|t| t.translation())
        .unwrap_or(Vec3::ZERO);

    // Calculate distances from entity center for both initial and current cursor positions
    let camera_forward = camera_transform.forward();
    let plane_normal = *camera_forward;
    let plane_point = entity_world_pos;

    // Get initial intersection point
    let Ok(initial_ray) = camera.viewport_to_world(camera_transform, initial_cursor) else {
        return;
    };
    let initial_denominator = initial_ray.direction.dot(plane_normal);
    if initial_denominator.abs() < 1e-6 {
        return;
    }
    let initial_t = (plane_point - initial_ray.origin).dot(plane_normal) / initial_denominator;
    let initial_intersection = initial_ray.origin + *initial_ray.direction * initial_t;
    let initial_distance = (initial_intersection - entity_world_pos).length();

    // Get current intersection point
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_pos) else {
        return;
    };
    let denominator = ray.direction.dot(plane_normal);
    if denominator.abs() < 1e-6 {
        return;
    }
    let t = (plane_point - ray.origin).dot(plane_normal) / denominator;
    let intersection = ray.origin + *ray.direction * t;
    let current_distance = (intersection - entity_world_pos).length();

    // Calculate scale factor based on the ratio of distances
    // When cursor hasn't moved, distances are equal, so scale_factor = 1.0
    let scale_factor = if initial_distance > 1e-6 {
        (current_distance / initial_distance).max(0.01)
    } else {
        1.0
    };

    // Apply axis mask
    for (entity, initial_world_scale) in initial_world_scales.iter().copied() {
        let new_world_scale = if let Some(axis) = &mask {
            match axis {
                AxisMask::X => initial_world_scale.with_x(initial_world_scale.x * scale_factor),
                AxisMask::Y => initial_world_scale.with_y(initial_world_scale.y * scale_factor),
                AxisMask::Z => initial_world_scale.with_z(initial_world_scale.z * scale_factor),
            }
        } else {
            initial_world_scale * scale_factor
        };
        let local_scale = world_scale_to_local(entity, new_world_scale, &parents, &parent_globals);
        if let Ok(mut transform) = transforms.get_mut(entity) {
            transform.scale = local_scale;
        }
    }
}

fn record_selected_actions(
    selected: &Selected,
    global_transforms: &Query<&GlobalTransform>,
    action_queue: &mut ActionQueue,
) {
    match &selected.action {
        Some(SelectedAction::Grab {
            initial_world_positions,
            ..
        }) => {
            let mut transforms = Vec::new();
            for (entity, old_position) in initial_world_positions.iter().copied() {
                if let Ok(global_transform) = global_transforms.get(entity) {
                    let new_position = global_transform.translation();
                    if (old_position - new_position).length_squared() > 1e-6 {
                        transforms.push(TransformAction::move_entity(
                            entity,
                            old_position,
                            new_position,
                        ));
                    }
                }
            }
            if !transforms.is_empty() {
                action_queue.push(TransformSelectionAction::new(transforms).into());
            }
        }
        Some(SelectedAction::Scale {
            initial_world_scales,
            initial_cursor_pos: _,
            ..
        }) => {
            let mut transforms = Vec::new();
            for (entity, old_scale) in initial_world_scales.iter().copied() {
                if let Ok(global_transform) = global_transforms.get(entity) {
                    let new_scale = global_transform.to_scale_rotation_translation().0;
                    if (old_scale - new_scale).length_squared() > 1e-6 {
                        transforms.push(TransformAction::scale(entity, old_scale, new_scale));
                    }
                }
            }
            if !transforms.is_empty() {
                action_queue.push(TransformSelectionAction::new(transforms).into());
            }
        }
        None => {}
    }
}

fn editor_enabled(enabled: Res<EditorEnabled>) -> bool {
    enabled.0
}

fn evaluate_editor_condition(world: &mut World) {
    let mut condition = world
        .remove_resource::<EditorCondition>()
        .expect("EditorCondition must exist when scheduled");
    let enabled = condition.system.run((), world).unwrap_or(false);
    world.insert_resource(condition);
    world.resource_mut::<EditorEnabled>().0 = enabled;
}

fn shift_is_pressed(keyboard_input: &ButtonInput<KeyCode>) -> bool {
    keyboard_input.pressed(KeyCode::ShiftLeft) || keyboard_input.pressed(KeyCode::ShiftRight)
}

fn finalize_action_if_active(
    selected: &mut Selected,
    global_transforms: &Query<&GlobalTransform>,
    action_queue: &mut ActionQueue,
) -> bool {
    if selected.action.is_some() {
        record_selected_actions(selected, global_transforms, action_queue);
        selected.action = None;
        return true;
    }
    false
}

fn collect_world_positions(
    entities: &[Entity],
    global_transforms: &Query<&GlobalTransform>,
) -> Vec<(Entity, Vec3)> {
    let mut positions = Vec::new();
    for entity in entities.iter().copied() {
        if let Ok(global_transform) = global_transforms.get(entity) {
            positions.push((entity, global_transform.translation()));
        }
    }
    positions
}

fn collect_world_scales(
    entities: &[Entity],
    global_transforms: &Query<&GlobalTransform>,
) -> Vec<(Entity, Vec3)> {
    let mut scales = Vec::new();
    for entity in entities.iter().copied() {
        if let Ok(global_transform) = global_transforms.get(entity) {
            scales.push((entity, global_transform.to_scale_rotation_translation().0));
        }
    }
    scales
}

fn world_position_to_local(
    entity: Entity,
    world_position: Vec3,
    parents: &Query<&ChildOf>,
    parent_globals: &Query<&GlobalTransform>,
) -> Vec3 {
    let parent_global: Option<&GlobalTransform> = parents
        .get(entity)
        .ok()
        .and_then(|child_of| parent_globals.get(child_of.parent()).ok());

    if let Some(parent_global) = parent_global {
        parent_global
            .affine()
            .inverse()
            .transform_point3(world_position)
    } else {
        world_position
    }
}

fn world_scale_to_local(
    entity: Entity,
    world_scale: Vec3,
    parents: &Query<&ChildOf>,
    parent_globals: &Query<&GlobalTransform>,
) -> Vec3 {
    let parent_global: Option<&GlobalTransform> = parents
        .get(entity)
        .ok()
        .and_then(|child_of| parent_globals.get(child_of.parent()).ok());

    if let Some(parent_global) = parent_global {
        parent_global
            .affine()
            .inverse()
            .to_scale_rotation_translation()
            .0
            * world_scale
    } else {
        world_scale
    }
}

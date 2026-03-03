use std::sync::Mutex;
use std::time::Duration;

use bevy::camera::Viewport;
use bevy::camera::visibility::RenderLayers;
use bevy::color::palettes::tailwind::{PINK_100, RED_500};
use bevy::ecs::message::MessageReader;
use bevy::input::keyboard::KeyboardInput;
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
    world_position_to_local_q,
};
use crate::editor_camera::{AxisAlignedProjectionState, sync_axis_aligned_projection};
use crate::state::{AxisMask, TypedTransformInput, UiDockState, UiState};
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
                handle_typed_input,
            )
                .run_if(resource_exists::<Selected>)
                .run_if(editor_enabled),
        );
        app.add_systems(
            Update,
            (handle_grab_mode_movement, handle_scale_mode_movement)
                .run_if(resource_exists::<Selected>)
                .run_if(editor_enabled),
        );
        app.add_systems(
            Update,
            (sync_axis_aligned_projection, draw_ground_grid).run_if(editor_enabled),
        );

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
        ui_state.ui(world, egui_context.get_mut());

        let (mode_label, typed, has_mask) = match world
            .get_resource::<Selected>()
            .and_then(|s| s.action.as_ref())
        {
            Some(SelectedAction::Grab {
                typed_input, mask, ..
            }) => ("Grab", typed_input.as_str(), mask.is_some()),
            Some(SelectedAction::Scale {
                typed_input, mask, ..
            }) => ("Scale", typed_input.as_str(), mask.is_some()),
            _ => return,
        };

        if typed.is_empty() && !has_mask {
            return;
        }

        let viewport = ui_state.viewport;
        let overlay_x = viewport.center().x;
        let overlay_y = viewport.min.y + 40.0;

        let mask_str = match world
            .get_resource::<Selected>()
            .and_then(|s| s.action.as_ref())
        {
            Some(SelectedAction::Grab { mask: Some(m), .. })
            | Some(SelectedAction::Scale { mask: Some(m), .. }) => match m {
                AxisMask::X => " [X]",
                AxisMask::Y => " [Y]",
                AxisMask::Z => " [Z]",
            },
            _ => "",
        };

        let display = if typed.is_empty() {
            format!("{}{}", mode_label, mask_str)
        } else {
            format!("{}{}: {}_", mode_label, mask_str, typed)
        };

        bevy_egui::egui::Area::new(bevy_egui::egui::Id::new("typed_input_overlay"))
            .fixed_pos(bevy_egui::egui::pos2(overlay_x, overlay_y))
            .show(egui_context.get_mut(), |ui| {
                bevy_egui::egui::Frame::new()
                    .fill(bevy_egui::egui::Color32::from_black_alpha(200))
                    .corner_radius(4.0)
                    .inner_margin(bevy_egui::egui::Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.label(
                            bevy_egui::egui::RichText::new(display)
                                .size(14.0)
                                .color(bevy_egui::egui::Color32::WHITE)
                                .monospace(),
                        );
                    });
            });
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
    local_transforms: Query<&Transform>,
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
            if finalize_action_if_active(
                selected,
                &local_transforms,
                &global_transforms,
                &mut action_queue,
            ) {
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
    local_transforms: Query<&Transform>,
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
            && finalize_action_if_active(
                selected,
                &local_transforms,
                &global_transforms,
                &mut action_queue,
            )
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

fn draw_ground_grid(
    mut gizmos: Gizmos,
    camera_query: Query<(&Projection, &GlobalTransform), With<EditorCamera>>,
) {
    let Ok((projection, camera_transform)) = camera_query.single() else {
        return;
    };

    if !matches!(projection, Projection::Perspective(_)) {
        return;
    }

    let cam_pos = camera_transform.translation();
    let cam_y = cam_pos.y.abs().max(5.0);

    let raw_step = cam_y / 8.0;
    let exponent = raw_step.log10().floor();
    let base = 10.0f32.powf(exponent);
    let fraction = raw_step / base;
    let step = base
        * if fraction <= 1.0 {
            1.0
        } else if fraction <= 2.0 {
            2.0
        } else if fraction <= 5.0 {
            5.0
        } else {
            10.0
        };

    let half_extent = step * 20.0;

    let snap = |v: f32| (v / step).round() * step;
    let center_x = snap(cam_pos.x);
    let center_z = snap(cam_pos.z);

    let count = (half_extent / step).ceil() as i32;

    let minor_color = Color::srgba(1.0, 1.0, 1.0, 0.04);
    let major_color = Color::srgba(1.0, 1.0, 1.0, 0.1);
    let x_axis_color = Color::srgba(0.9, 0.27, 0.27, 0.6);
    let z_axis_color = Color::srgba(0.27, 0.47, 1.0, 0.6);

    let major_step = step * 10.0;
    let is_major = |world_coord: f32| {
        (world_coord / major_step).round() * major_step - world_coord < step * 0.01
            && world_coord.abs() >= step * 0.01
    };

    for i in -count..=count {
        let offset = i as f32 * step;

        let world_x = center_x + offset;
        let on_origin = world_x.abs() < step * 0.01;
        let color = if on_origin {
            z_axis_color
        } else if is_major(world_x) {
            major_color
        } else {
            minor_color
        };
        gizmos.line(
            Vec3::new(world_x, 0.0, center_z - half_extent),
            Vec3::new(world_x, 0.0, center_z + half_extent),
            color,
        );

        let world_z = center_z + offset;
        let on_origin = world_z.abs() < step * 0.01;
        let color = if on_origin {
            x_axis_color
        } else if is_major(world_z) {
            major_color
        } else {
            minor_color
        };
        gizmos.line(
            Vec3::new(center_x - half_extent, 0.0, world_z),
            Vec3::new(center_x + half_extent, 0.0, world_z),
            color,
        );
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
    mut selected: ResMut<Selected>,
    global_transforms: Query<&GlobalTransform>,
    mut action_queue: ResMut<ActionQueue>,
    camera_query: Query<&PanOrbitCamera, With<EditorCamera>>,
) {
    let primary = selected.primary();

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

    let axis_key = [KeyCode::KeyX, KeyCode::KeyY, KeyCode::KeyZ]
        .into_iter()
        .find(|k| keyboard_input.just_pressed(*k))
        .and_then(AxisMask::from_key);

    match &mut selected.action {
        None if keyboard_input.just_pressed(KeyCode::KeyG) => {
            let initial_world_positions =
                collect_world_positions(&selected.entities, &global_transforms);
            let initial_local_transforms =
                collect_local_transforms(&selected.entities, &transforms);
            let Some((_, initial_primary_pos)) = initial_world_positions
                .iter()
                .find(|(entity, _)| *entity == primary)
            else {
                return;
            };
            selected.action = Some(SelectedAction::Grab {
                mask: None,
                initial_primary_pos: *initial_primary_pos,
                initial_world_positions,
                initial_local_transforms,
                typed_input: String::new(),
            });
        }
        None if keyboard_input.just_pressed(KeyCode::KeyS) => {
            let initial_world_scales = collect_world_scales(&selected.entities, &global_transforms);
            let initial_local_transforms =
                collect_local_transforms(&selected.entities, &transforms);
            if !initial_world_scales
                .iter()
                .any(|(entity, _)| *entity == primary)
            {
                return;
            }
            selected.action = Some(SelectedAction::Scale {
                mask: None,
                initial_cursor_pos: None,
                initial_world_scales,
                initial_local_transforms,
                typed_input: String::new(),
            });
        }
        None if keyboard_input.just_pressed(KeyCode::KeyX) => {
            for &entity in &selected.entities {
                action_queue.push(RemoveAction::new(entity).into());
            }
        }
        None => {}
        Some(SelectedAction::Grab {
            mask,
            initial_local_transforms,
            typed_input,
            ..
        }) => {
            if typed_input.is_empty() {
                if let Some(new_mask) = axis_key {
                    *mask = Some(new_mask);
                }
                if keyboard_input.just_pressed(KeyCode::KeyS) {
                    restore_local_transforms(initial_local_transforms, &mut transforms);
                    let initial_world_scales =
                        collect_world_scales(&selected.entities, &global_transforms);
                    let initial_local_transforms =
                        collect_local_transforms(&selected.entities, &transforms);
                    selected.action = Some(SelectedAction::Scale {
                        mask: None,
                        initial_cursor_pos: None,
                        initial_world_scales,
                        initial_local_transforms,
                        typed_input: String::new(),
                    });
                    return;
                }
            }
            if keyboard_input.just_pressed(KeyCode::Escape) {
                restore_local_transforms(initial_local_transforms, &mut transforms);
                selected.action = None;
            }
        }
        Some(SelectedAction::Scale {
            mask,
            initial_local_transforms,
            typed_input,
            ..
        }) => {
            if typed_input.is_empty() {
                if let Some(new_mask) = axis_key {
                    *mask = Some(new_mask);
                }
                if keyboard_input.just_pressed(KeyCode::KeyG) {
                    restore_local_transforms(initial_local_transforms, &mut transforms);
                    let initial_world_positions =
                        collect_world_positions(&selected.entities, &global_transforms);
                    let initial_local_transforms =
                        collect_local_transforms(&selected.entities, &transforms);
                    let Some((_, initial_primary_pos)) = initial_world_positions
                        .iter()
                        .find(|(entity, _)| *entity == primary)
                    else {
                        return;
                    };
                    selected.action = Some(SelectedAction::Grab {
                        mask: None,
                        initial_primary_pos: *initial_primary_pos,
                        initial_world_positions,
                        initial_local_transforms,
                        typed_input: String::new(),
                    });
                    return;
                }
            }
            if keyboard_input.just_pressed(KeyCode::Escape) {
                restore_local_transforms(initial_local_transforms, &mut transforms);
                selected.action = None;
            }
        }
    }
}

/// Cast a ray from the camera through `cursor_pos` and intersect it with a plane
/// perpendicular to the camera's forward direction passing through `plane_point`.
fn cursor_ray_to_plane(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    cursor_pos: Vec2,
    plane_point: Vec3,
) -> Option<Vec3> {
    let ray = camera
        .viewport_to_world(camera_transform, cursor_pos)
        .ok()?;
    let plane_normal = *camera_transform.forward();
    let denominator = ray.direction.dot(plane_normal);
    if denominator.abs() < 1e-6 {
        return None;
    }
    let t = (plane_point - ray.origin).dot(plane_normal) / denominator;
    Some(ray.origin + *ray.direction * t)
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
        typed_input,
        ..
    }) = &mut selected.action
    else {
        return;
    };

    if !typed_input.is_empty() {
        return;
    }

    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    let Some(intersection) =
        cursor_ray_to_plane(camera, camera_transform, cursor_pos, *initial_primary_pos)
    else {
        return;
    };

    let new_primary_world_pos = match mask {
        Some(axis) => axis.apply(*initial_primary_pos, intersection),
        None => intersection,
    };

    let delta_world = new_primary_world_pos - *initial_primary_pos;

    for (entity, initial_world_pos) in initial_world_positions.iter().copied() {
        let new_world_pos = initial_world_pos + delta_world;
        let new_local_pos =
            world_position_to_local_q(entity, new_world_pos, &parents, &parent_globals);
        if let Ok(mut transform) = transforms.get_mut(entity) {
            transform.translation = new_local_pos;
        }
    }
}

fn handle_scale_mode_movement(
    ui: Res<UiState>,
    mut transforms: Query<&mut Transform>,
    global_transforms: Query<&GlobalTransform>,
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
        initial_local_transforms,
        typed_input,
        ..
    }) = &mut selected.action
    else {
        return;
    };

    if !typed_input.is_empty() {
        return;
    }

    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    let initial_cursor = match initial_cursor_pos {
        Some(pos) => *pos,
        None => {
            *initial_cursor_pos = Some(cursor_pos);
            return;
        }
    };

    let entity_world_pos = global_transforms
        .get(primary)
        .map(|t| t.translation())
        .unwrap_or(Vec3::ZERO);

    let Some(initial_intersection) =
        cursor_ray_to_plane(camera, camera_transform, initial_cursor, entity_world_pos)
    else {
        return;
    };
    let Some(current_intersection) =
        cursor_ray_to_plane(camera, camera_transform, cursor_pos, entity_world_pos)
    else {
        return;
    };

    let initial_distance = (initial_intersection - entity_world_pos).length();
    let current_distance = (current_intersection - entity_world_pos).length();

    let scale_factor = if initial_distance > 1e-6 {
        (current_distance / initial_distance).max(0.01)
    } else {
        1.0
    };

    for (entity, initial_local) in initial_local_transforms.iter().copied() {
        if let Ok(mut transform) = transforms.get_mut(entity) {
            transform.scale = match mask {
                Some(axis) => axis.apply_scale(initial_local.scale, scale_factor),
                None => initial_local.scale * scale_factor,
            };
        }
    }
}

fn handle_typed_input(
    mut char_events: MessageReader<KeyboardInput>,
    mut selected: ResMut<Selected>,
    mut transforms: Query<&mut Transform>,
    mut action_queue: ResMut<ActionQueue>,
) {
    let typed_input = match &mut selected.action {
        Some(SelectedAction::Grab { typed_input, .. })
        | Some(SelectedAction::Scale { typed_input, .. }) => typed_input,
        _ => return,
    };

    let mut changed = false;
    let mut confirmed = false;

    for event in char_events.read() {
        if !event.state.is_pressed() {
            continue;
        }
        match &event.logical_key {
            bevy::input::keyboard::Key::Character(c) => {
                let c = c.as_str();
                for ch in c.chars() {
                    match ch {
                        '0'..='9' | '.' | '-' | '=' | 'x' | 'X' | 'y' | 'Y' | 'z' | 'Z' => {
                            typed_input.push(ch);
                            changed = true;
                        }
                        _ => {}
                    }
                }
            }
            bevy::input::keyboard::Key::Backspace => {
                typed_input.pop();
                changed = true;
            }
            bevy::input::keyboard::Key::Enter => {
                confirmed = true;
                break;
            }
            _ => {}
        }
    }

    if confirmed {
        let input_str = match &selected.action {
            Some(SelectedAction::Grab { typed_input, .. })
            | Some(SelectedAction::Scale { typed_input, .. }) => typed_input.clone(),
            _ => return,
        };
        let parsed = TypedTransformInput::parse(&input_str);
        commit_typed_input(&mut selected, parsed, &mut action_queue);
        return;
    }

    if changed {
        apply_typed_input_live(&mut selected, &mut transforms);
    }
}

/// Apply the current typed input as a live preview, always relative to the stored initial transforms.
fn apply_typed_input_live(selected: &mut ResMut<Selected>, transforms: &mut Query<&mut Transform>) {
    match &selected.action {
        Some(SelectedAction::Grab {
            mask,
            initial_local_transforms,
            typed_input,
            ..
        }) => {
            let parsed = TypedTransformInput::parse(typed_input);
            let initial = initial_local_transforms.clone();
            restore_local_transforms(&initial, transforms);

            if let Some(parsed) = parsed {
                let axis = parsed.axis.as_ref().or(mask.as_ref());
                for (entity, _) in initial.iter().copied() {
                    if let Ok(mut transform) = transforms.get_mut(entity) {
                        if parsed.exact {
                            match axis {
                                Some(AxisMask::X) => transform.translation.x = parsed.value,
                                Some(AxisMask::Y) => transform.translation.y = parsed.value,
                                Some(AxisMask::Z) => transform.translation.z = parsed.value,
                                None => transform.translation = Vec3::splat(parsed.value),
                            }
                        } else {
                            let delta = match axis {
                                Some(AxisMask::X) => Vec3::new(parsed.value, 0.0, 0.0),
                                Some(AxisMask::Y) => Vec3::new(0.0, parsed.value, 0.0),
                                Some(AxisMask::Z) => Vec3::new(0.0, 0.0, parsed.value),
                                None => Vec3::splat(parsed.value),
                            };
                            transform.translation += delta;
                        }
                    }
                }
            }
        }
        Some(SelectedAction::Scale {
            mask,
            initial_local_transforms,
            typed_input,
            ..
        }) => {
            let parsed = TypedTransformInput::parse(typed_input);
            let initial = initial_local_transforms.clone();
            restore_local_transforms(&initial, transforms);

            if let Some(parsed) = parsed {
                let axis = parsed.axis.as_ref().or(mask.as_ref());
                for (entity, _) in initial.iter().copied() {
                    if let Ok(mut transform) = transforms.get_mut(entity) {
                        if parsed.exact {
                            match axis {
                                Some(AxisMask::X) => transform.scale.x = parsed.value,
                                Some(AxisMask::Y) => transform.scale.y = parsed.value,
                                Some(AxisMask::Z) => transform.scale.z = parsed.value,
                                None => transform.scale = Vec3::splat(parsed.value),
                            }
                        } else {
                            match axis {
                                Some(AxisMask::X) => transform.scale.x *= parsed.value,
                                Some(AxisMask::Y) => transform.scale.y *= parsed.value,
                                Some(AxisMask::Z) => transform.scale.z *= parsed.value,
                                None => transform.scale *= parsed.value,
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Commit the typed input as an undoable action. The transforms are already in their final
/// state from the live preview, so we just need to record old vs new.
fn commit_typed_input(
    selected: &mut ResMut<Selected>,
    parsed: Option<TypedTransformInput>,
    action_queue: &mut ResMut<ActionQueue>,
) {
    let Some(parsed) = parsed else {
        // Invalid input — revert was already handled by live preview restoring to initial
        match &selected.action {
            Some(SelectedAction::Grab {
                initial_local_transforms,
                ..
            })
            | Some(SelectedAction::Scale {
                initial_local_transforms,
                ..
            }) => {
                let _ = initial_local_transforms; // already restored by live preview
            }
            _ => {}
        }
        selected.action = None;
        return;
    };

    match &selected.action {
        Some(SelectedAction::Grab {
            mask,
            initial_local_transforms,
            ..
        }) => {
            let axis = parsed.axis.as_ref().or(mask.as_ref());
            let mut action_transforms = Vec::new();
            for (entity, old_local) in initial_local_transforms.iter().copied() {
                let mut new_local = old_local;
                if parsed.exact {
                    match axis {
                        Some(AxisMask::X) => new_local.translation.x = parsed.value,
                        Some(AxisMask::Y) => new_local.translation.y = parsed.value,
                        Some(AxisMask::Z) => new_local.translation.z = parsed.value,
                        None => new_local.translation = Vec3::splat(parsed.value),
                    }
                } else {
                    let delta = match axis {
                        Some(AxisMask::X) => Vec3::new(parsed.value, 0.0, 0.0),
                        Some(AxisMask::Y) => Vec3::new(0.0, parsed.value, 0.0),
                        Some(AxisMask::Z) => Vec3::new(0.0, 0.0, parsed.value),
                        None => Vec3::splat(parsed.value),
                    };
                    new_local.translation += delta;
                }
                action_transforms.push(TransformAction::full(entity, old_local, new_local));
            }
            if !action_transforms.is_empty() {
                action_queue.push(TransformSelectionAction::new(action_transforms).into());
            }
        }
        Some(SelectedAction::Scale {
            mask,
            initial_local_transforms,
            ..
        }) => {
            let axis = parsed.axis.as_ref().or(mask.as_ref());
            let mut action_transforms = Vec::new();
            for (entity, old_local) in initial_local_transforms.iter().copied() {
                let mut new_local = old_local;
                if parsed.exact {
                    match axis {
                        Some(AxisMask::X) => new_local.scale.x = parsed.value,
                        Some(AxisMask::Y) => new_local.scale.y = parsed.value,
                        Some(AxisMask::Z) => new_local.scale.z = parsed.value,
                        None => new_local.scale = Vec3::splat(parsed.value),
                    }
                } else {
                    match axis {
                        Some(AxisMask::X) => new_local.scale.x *= parsed.value,
                        Some(AxisMask::Y) => new_local.scale.y *= parsed.value,
                        Some(AxisMask::Z) => new_local.scale.z *= parsed.value,
                        None => new_local.scale *= parsed.value,
                    }
                }
                action_transforms.push(TransformAction::full(entity, old_local, new_local));
            }
            if !action_transforms.is_empty() {
                action_queue.push(TransformSelectionAction::new(action_transforms).into());
            }
        }
        _ => {}
    }

    selected.action = None;
}

fn record_selected_actions(
    selected: &Selected,
    local_transforms: &Query<&Transform>,
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
            initial_local_transforms,
            ..
        }) => {
            let mut transforms = Vec::new();
            for (entity, old_local) in initial_local_transforms.iter().copied() {
                if let Ok(&new_local) = local_transforms.get(entity) {
                    if (old_local.scale - new_local.scale).length_squared() > 1e-6 {
                        transforms.push(TransformAction::full(entity, old_local, new_local));
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
    local_transforms: &Query<&Transform>,
    global_transforms: &Query<&GlobalTransform>,
    action_queue: &mut ActionQueue,
) -> bool {
    if selected.action.is_some() {
        record_selected_actions(selected, local_transforms, global_transforms, action_queue);
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

fn collect_local_transforms(
    entities: &[Entity],
    transforms: &Query<&mut Transform>,
) -> Vec<(Entity, Transform)> {
    entities
        .iter()
        .copied()
        .filter_map(|entity| transforms.get(entity).ok().map(|t| (entity, *t)))
        .collect()
}

fn restore_local_transforms(
    initial: &[(Entity, Transform)],
    transforms: &mut Query<&mut Transform>,
) {
    for (entity, initial_transform) in initial.iter().copied() {
        if let Ok(mut transform) = transforms.get_mut(entity) {
            *transform = initial_transform;
        }
    }
}

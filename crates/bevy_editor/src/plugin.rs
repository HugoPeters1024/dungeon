use std::sync::Mutex;
use std::time::Duration;

use bevy::camera::Viewport;
use bevy::camera::visibility::RenderLayers;
use bevy::color::palettes::tailwind::{PINK_100, RED_500};
use bevy::mesh::Indices;
use bevy::picking::pointer::PointerInteraction;
use bevy::picking::prelude::Pickable;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::{ecs::schedule::BoxedCondition, window::PrimaryWindow};
use bevy_egui::prelude::*;
use bevy_panorbit_camera::PanOrbitCameraPlugin;

use bevy_panorbit_camera::PanOrbitCamera;

use crate::actions::{ActionQueue, FocusCameraAction, MoveAction, ScaleAction, process_action_queue};
use crate::state::{AxisMask, Prefabs, SpawnPosition, UiDockState, UiState};
use crate::{ContextMenu, HoverNormal, Selected, SelectedAction};

const CLICK_DURATION: Duration = Duration::from_millis(500);

#[derive(Component)]
pub struct EditorCamera;

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
        app.init_resource::<Prefabs>();
        app.init_resource::<SpawnPosition>();
        app.init_resource::<ActionQueue>();
        app.add_systems(Startup, setup_ui);
        app.add_systems(Startup, spawn_wireframe_plane);
        app.add_observer(on_click_in_void);
        app.add_observer(on_click_object);

        app.insert_resource(UiDockState::initialize());
        app.insert_resource(UiState::new());

        app.add_systems(
            Update,
            (
                draw_axes,
                set_hover_normal,
                handle_selected_action_keys,
                handle_grab_mode_movement,
                handle_scale_mode_movement,
            )
                .run_if(resource_exists::<Selected>),
        );

        app.add_systems(Update, process_action_queue);

        {
            let mut system = show_ui_system.into_configs();
            let condition = self.condition.lock().unwrap().take();
            if let Some(condition) = condition {
                system.run_if_dyn(condition);
            }

            app.add_systems(EguiPrimaryContextPass, system);
        }
    }
}

fn spawn_wireframe_plane(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Create a large grid mesh for the wireframe plane
    let size = 1000.0; // Large size to appear infinite
    let grid_size = 1000; // Number of grid cells

    let mut mesh = Mesh::new(
        PrimitiveTopology::LineList,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );

    let mut positions = Vec::new();
    let mut indices = Vec::new();

    let cell_size = size / grid_size as f32;
    let half_size = size / 2.0;

    // Create grid lines along X axis (lines parallel to X)
    for i in 0..=grid_size {
        let z = -half_size + i as f32 * cell_size;
        positions.push([-half_size, 0.0, z]);
        positions.push([half_size, 0.0, z]);
        let base_idx = (positions.len() - 2) as u32;
        indices.push(base_idx);
        indices.push(base_idx + 1);
    }

    // Create grid lines along Z axis (lines parallel to Z)
    for i in 0..=grid_size {
        let x = -half_size + i as f32 * cell_size;
        positions.push([x, 0.0, -half_size]);
        positions.push([x, 0.0, half_size]);
        let base_idx = (positions.len() - 2) as u32;
        indices.push(base_idx);
        indices.push(base_idx + 1);
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(Indices::U32(indices));

    // Create a material for the wireframe
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.5, 0.5),
        unlit: true,
        ..default()
    });

    // Spawn the wireframe plane
    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material),
        Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
    ));
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
    mut selected: Option<ResMut<Selected>>,
    global_transforms: Query<&GlobalTransform>,
    transforms: Query<&Transform>,
    mut action_queue: ResMut<ActionQueue>,
) {
    if !ui_state.pointer_in_viewport
        || ui_state.egui_wants_pointer_input
        || trigger.duration > CLICK_DURATION
        || !windows.contains(trigger.event_target())
    {
        return;
    }
    println!("clicked on window");

    let is_performing_action = selected.as_ref().is_some_and(|s| s.action.is_some());
    let is_primary = trigger.button == PointerButton::Primary;
    let is_secondary = trigger.button == PointerButton::Secondary;
    let hover_normal = selected.as_ref().and_then(|s| s.hover_normal.clone());

    if is_primary {
        ui_state.context_menu = ContextMenu::Closed;
        if is_performing_action {
            if let Some(selected) = selected.as_mut() {
                let entity = selected.entity;

                // Record the move action before clearing it
                if let Some(SelectedAction::Grab {
                    initial_entity_pos, ..
                }) = &selected.action
                {
                    if let Ok(global_transform) = global_transforms.get(entity) {
                        let new_position = global_transform.translation();
                        // Only record if position actually changed
                        if (*initial_entity_pos - new_position).length_squared() > 1e-6 {
                            action_queue.push(
                                MoveAction {
                                    entity,
                                    old_position: *initial_entity_pos,
                                    new_position,
                                }
                                .into(),
                            );
                        }
                    }
                }

                // Record the scale action before clearing it
                if let Some(SelectedAction::Scale {
                    initial_entity_scale,
                    ..
                }) = &selected.action
                {
                    if let Ok(transform) = transforms.get(entity) {
                        let new_scale = transform.scale;
                        // Only record if scale actually changed
                        if (*initial_entity_scale - new_scale).length_squared() > 1e-6 {
                            action_queue.push(
                                ScaleAction {
                                    entity,
                                    old_scale: *initial_entity_scale,
                                    new_scale,
                                }
                                .into(),
                            );
                        }
                    }
                }

                selected.action = None;
            }
        } else {
            commands.remove_resource::<Selected>();
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
    names: Query<&Name>,
    mut ui_state: ResMut<UiState>,
    selected: Option<ResMut<Selected>>,
    windows: Query<&Window>,
    global_transforms: Query<&GlobalTransform>,
    transforms: Query<&Transform>,
    mut action_queue: ResMut<ActionQueue>,
) {
    if !ui_state.pointer_in_viewport
        || ui_state.egui_wants_pointer_input
        || trigger.duration > CLICK_DURATION
        || windows.contains(trigger.event_target())
    {
        return;
    }
    println!(
        "clicked on object function called entity={}, name={:?}",
        trigger.event_target(),
        names.get(trigger.event_target())
    );

    let is_performing_action = selected.as_ref().is_some_and(|s| s.action.is_some());
    let is_primary = trigger.button == PointerButton::Primary;
    let is_secondary = trigger.button == PointerButton::Secondary;
    let hover_normal = selected.as_ref().and_then(|s| s.hover_normal.clone());

    trigger.propagate(false);

    if is_primary {
        ui_state.context_menu = ContextMenu::Closed;

        // Record move action if we were performing a grab
        if let Some(ref selected) = selected {
            let entity = selected.entity;

            if let Some(SelectedAction::Grab {
                initial_entity_pos, ..
            }) = &selected.action
            {
                if let Ok(global_transform) = global_transforms.get(entity) {
                    let new_position = global_transform.translation();
                    // Only record if position actually changed
                    if (*initial_entity_pos - new_position).length_squared() > 1e-6 {
                        action_queue.push(
                            MoveAction {
                                entity,
                                old_position: *initial_entity_pos,
                                new_position,
                            }
                            .into(),
                        );
                    }
                }
            }

            // Record scale action if we were performing a scale
            if let Some(SelectedAction::Scale {
                initial_entity_scale,
                ..
            }) = &selected.action
            {
                if let Ok(transform) = transforms.get(entity) {
                    let new_scale = transform.scale;
                    // Only record if scale actually changed
                    if (*initial_entity_scale - new_scale).length_squared() > 1e-6 {
                        action_queue.push(
                            ScaleAction {
                                entity,
                                old_scale: *initial_entity_scale,
                                new_scale,
                            }
                            .into(),
                        );
                    }
                }
            }
        }

        commands.insert_resource(Selected {
            entity: trigger.event_target(),
            hover_normal: None,
            action: None,
        });
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
    if let Ok(transform) = query.get(selected.entity) {
        gizmos.axes(*transform, 1.5);
    }
}

/// Draws normals at the mouse hover position, but only if the hovered entity
/// is the currently selected entity.
fn set_hover_normal(
    pointers: Query<&PointerInteraction>,
    mut selected: ResMut<Selected>,
    mut gizmos: Gizmos,
) {
    let selected_entity = selected.entity;
    selected.hover_normal = None;
    for (point, normal) in pointers
        .iter()
        .filter_map(|interaction| interaction.get_nearest_hit())
        .filter(|(entity, _hit)| *entity == selected_entity)
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
    mut transforms: Query<(&mut Transform, &GlobalTransform)>,
    parents: Query<&ChildOf>,
    parent_globals: Query<&GlobalTransform>,
    mut selected: ResMut<Selected>,
    global_transforms: Query<&GlobalTransform>,
    mut action_queue: ResMut<ActionQueue>,
    camera_query: Query<&PanOrbitCamera, With<EditorCamera>>,
) {
    let entity = selected.entity;

    // F key: Focus camera on selected object
    if keyboard_input.just_pressed(KeyCode::KeyF) {
        if let Ok(global_transform) = global_transforms.get(entity) {
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
    }

    match &mut selected.action {
        None if keyboard_input.just_pressed(KeyCode::KeyG) => {
            let Ok((_, global_transform)) = transforms.get(selected.entity) else {
                return;
            };
            // Store the world position for grab calculations
            selected.action = Some(SelectedAction::Grab {
                mask: None,
                initial_entity_pos: global_transform.translation(),
            });
        }
        None if keyboard_input.just_pressed(KeyCode::KeyS) => {
            let Ok((_, global_transform)) = transforms.get(selected.entity) else {
                return;
            };
            // Store the world position for grab calculations
            selected.action = Some(SelectedAction::Scale {
                mask: None,
                initial_entity_scale: global_transform.to_scale_rotation_translation().0,
            });
        }
        None => {}
        Some(SelectedAction::Grab {
            mask,
            initial_entity_pos,
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
                // Convert world position back to local space
                let parent_global: Option<&GlobalTransform> = parents
                    .get(entity)
                    .ok()
                    .and_then(|child_of| parent_globals.get(child_of.parent()).ok());

                let local_pos = if let Some(parent_global) = parent_global {
                    parent_global
                        .affine()
                        .inverse()
                        .transform_point3(*initial_entity_pos)
                } else {
                    *initial_entity_pos
                };

                if let Ok((mut transform, _)) = transforms.get_mut(entity) {
                    transform.translation = local_pos;
                };
                selected.action = None;
            }
        }
        Some(SelectedAction::Scale {
            mask,
            initial_entity_scale,
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
                // Restore original scale
                if let Ok((mut transform, _)) = transforms.get_mut(entity) {
                    transform.scale = *initial_entity_scale;
                };
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

    let entity = selected.entity;

    let Some(SelectedAction::Grab {
        mask,
        initial_entity_pos,
        ..
    }) = &mut selected.action
    else {
        return;
    };

    let Ok(mut transform) = transforms.get_mut(entity) else {
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
    let plane_point = *initial_entity_pos;

    // Ray-plane intersection (in world space)
    let denominator = ray.direction.dot(plane_normal);
    if denominator.abs() < 1e-6 {
        return;
    }

    let t = (plane_point - ray.origin).dot(plane_normal) / denominator;
    let intersection = ray.origin + *ray.direction * t;

    // Apply axis mask in world space
    let new_world_pos = if let Some(axis) = &mask {
        match axis {
            AxisMask::X => initial_entity_pos.with_x(intersection.x),
            AxisMask::Y => initial_entity_pos.with_y(intersection.y),
            AxisMask::Z => initial_entity_pos.with_z(intersection.z),
        }
    } else {
        intersection
    };

    // Convert world position to local space, accounting for parent transform
    let parent_global: Option<&GlobalTransform> = parents
        .get(entity)
        .ok()
        .and_then(|child_of| parent_globals.get(child_of.parent()).ok());

    let new_local_pos = if let Some(parent_global) = parent_global {
        parent_global
            .affine()
            .inverse()
            .transform_point3(new_world_pos)
    } else {
        new_world_pos
    };

    transform.translation = new_local_pos;
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

    let entity = selected.entity;

    let Some(SelectedAction::Scale {
        mask,
        initial_entity_scale,
        ..
    }) = &mut selected.action
    else {
        return;
    };

    let Ok(mut transform) = transforms.get_mut(entity) else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    // Get the entity's world position for the reference plane
    let entity_world_pos = global_transforms
        .get(entity)
        .map(|t| t.translation())
        .unwrap_or(Vec3::ZERO);

    // Use Bevy's built-in viewport_to_world to get a ray from camera through cursor
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_pos) else {
        return;
    };

    let camera_forward = camera_transform.forward();

    // Define plane perpendicular to camera forward, passing through entity position
    let plane_normal = *camera_forward;
    let plane_point = entity_world_pos;

    // Ray-plane intersection
    let denominator = ray.direction.dot(plane_normal);
    if denominator.abs() < 1e-6 {
        return;
    }

    let t = (plane_point - ray.origin).dot(plane_normal) / denominator;
    let intersection = ray.origin + *ray.direction * t;

    // Calculate scale factor based on distance from entity center
    // The further the cursor is from the entity, the larger the scale
    let offset = intersection - entity_world_pos;
    let distance = offset.length();

    // Use a base distance to normalize the scale (adjust this for sensitivity)
    let base_distance = 2.0;
    let scale_factor = (distance / base_distance).max(0.01); // Prevent zero/negative scale

    // Apply axis mask
    let new_scale = if let Some(axis) = &mask {
        match axis {
            AxisMask::X => initial_entity_scale.with_x(initial_entity_scale.x * scale_factor),
            AxisMask::Y => initial_entity_scale.with_y(initial_entity_scale.y * scale_factor),
            AxisMask::Z => initial_entity_scale.with_z(initial_entity_scale.z * scale_factor),
        }
    } else {
        *initial_entity_scale * scale_factor
    };

    transform.scale = new_scale;
}

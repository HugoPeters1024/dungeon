use std::sync::Mutex;
use std::time::Duration;

use bevy::camera::Viewport;
use bevy::camera::visibility::RenderLayers;
use bevy::mesh::Indices;
use bevy::picking::prelude::Pickable;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::{ecs::schedule::BoxedCondition, window::PrimaryWindow};
use bevy_egui::prelude::*;
use bevy_panorbit_camera::PanOrbitCameraPlugin;

use crate::state::{AxisMask, ContextMenu, Prefabs, UiDockState, UiState};

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
        app.add_systems(Startup, setup_ui);
        app.add_systems(Startup, spawn_wireframe_plane);
        app.add_observer(set_selected_entity_on_click);

        app.add_systems(Update, draw_axes);

        app.insert_resource(UiDockState::initialize());
        app.insert_resource(UiState::new());

        app.add_systems(
            Update,
            (
                handle_grab_mode_input,
                handle_grab_mode_movement,
                handle_grab_mode_exit,
            ),
        );

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

fn set_selected_entity_on_click(
    mut trigger: On<Pointer<Click>>,
    names: Query<&Name>,
    windows: Query<&Window>,
    mut ui_state: ResMut<UiState>,
) {
    if !(ui_state.pointer_in_viewport || ui_state.egui_wants_pointer_input) {
        return;
    }
    println!(
        "test function called entity={}, name={:?}",
        trigger.event_target(),
        names.get(trigger.event_target())
    );

    let clicked_in_void = windows.contains(trigger.event_target());

    trigger.propagate(false);
    if trigger.duration < CLICK_DURATION {
        if clicked_in_void {
            match trigger.button {
                PointerButton::Primary => {
                    ui_state.selected_entity = None;
                    ui_state.context_menu = ContextMenu::Closed;
                }
                PointerButton::Secondary => {
                    ui_state.context_menu = ContextMenu::Open(trigger.pointer_location.position)
                }
                PointerButton::Middle => {}
            }
        } else {
            match trigger.button {
                PointerButton::Primary => {
                    ui_state.context_menu = ContextMenu::Closed;
                    ui_state.selected_entity = Some(trigger.event_target());
                }
                PointerButton::Secondary => {
                    ui_state.context_menu = ContextMenu::Open(trigger.pointer_location.position);
                    ui_state.selected_entity = Some(trigger.event_target());
                }
                PointerButton::Middle => {}
            };
        }
    }
}

fn draw_axes(mut gizmos: Gizmos, query: Query<&GlobalTransform>, ui_state: Res<UiState>) {
    if let Some(entity) = ui_state.selected_entity
        && let Ok(transform) = query.get(entity)
    {
        gizmos.axes(*transform, 1.5);
    }
}

fn handle_grab_mode_input(keyboard_input: Res<ButtonInput<KeyCode>>, mut ui: ResMut<UiState>) {
    // Enter grab mode when 'G' is pressed and an entity is selected
    if keyboard_input.just_pressed(KeyCode::KeyG)
        && !ui.grab_mode.is_active
        && ui.selected_entity.is_some()
        && ui.pointer_in_viewport
    {
        ui.grab_mode.is_active = true;
        ui.grab_mode.initial_mouse_pos = None;
        ui.grab_mode.initial_entity_pos = None;
        ui.grab_mode.axis_mask = None;
    }

    if ui.grab_mode.is_active {
        if keyboard_input.just_pressed(KeyCode::KeyX) {
            ui.grab_mode.axis_mask = Some(AxisMask::X);
        }
        if keyboard_input.just_pressed(KeyCode::KeyY) {
            ui.grab_mode.axis_mask = Some(AxisMask::Y);
        }
        if keyboard_input.just_pressed(KeyCode::KeyZ) {
            ui.grab_mode.axis_mask = Some(AxisMask::Z);
        }
    }
}

fn handle_grab_mode_movement(
    mut ui: ResMut<UiState>,
    mut transforms: Query<&mut Transform>,
    camera_query: Query<(&Camera, &Projection, &GlobalTransform), With<EditorCamera>>,
    window: Query<&Window, With<PrimaryWindow>>,
) {
    if !ui.grab_mode.is_active {
        return;
    }

    // Only process movement when pointer is in viewport
    if !ui.pointer_in_viewport {
        return;
    }

    let Some(selected_entity) = ui.selected_entity else {
        return;
    };

    let Ok(mut transform) = transforms.get_mut(selected_entity) else {
        return;
    };

    let Ok((camera, projection, camera_transform)) = camera_query.single() else {
        return;
    };

    let Ok(window) = window.single() else {
        return;
    };

    // Get current mouse position in viewport coordinates
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    // Convert cursor position to viewport coordinates
    let viewport = camera.logical_viewport_rect().unwrap_or_default();
    let viewport_cursor = cursor_pos - viewport.min;

    // Initialize grab mode state on first movement
    if ui.grab_mode.initial_mouse_pos.is_none() {
        ui.grab_mode.initial_mouse_pos = Some(viewport_cursor);
        ui.grab_mode.initial_entity_pos = Some(transform.translation);
    }

    let initial_pos = ui.grab_mode.initial_entity_pos.unwrap();
    let camera_pos = camera_transform.translation();
    let camera_forward = *camera_transform.forward();

    // Define a plane perpendicular to camera forward, passing through initial object position
    // Plane equation: dot(point - plane_point, plane_normal) = 0
    let plane_normal = camera_forward;
    let plane_point = initial_pos;

    // Convert screen coordinates to normalized device coordinates (NDC)
    // NDC ranges from -1 to 1 in both axes
    let viewport_size = viewport.size();
    let ndc_x = (viewport_cursor.x / viewport_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (viewport_cursor.y / viewport_size.y) * 2.0; // Invert Y axis

    // Get camera's right and up vectors for constructing the ray
    let camera_right = *camera_transform.right();
    let camera_up = *camera_transform.up();

    // Calculate aspect ratio and FOV
    // For perspective projection, we need to account for FOV
    let aspect = viewport_size.x / viewport_size.y;

    // Get FOV from projection
    let fov_y = match projection {
        Projection::Perspective(perspective_projection) => perspective_projection.fov,
        _ => std::f32::consts::PI / 4.0, // 45 degrees default
    };
    let tan_half_fov = (fov_y / 2.0).tan();

    // Convert NDC to view space direction
    let view_space_x = ndc_x * aspect * tan_half_fov;
    let view_space_y = ndc_y * tan_half_fov;
    let view_space_dir = Vec3::new(view_space_x, view_space_y, -1.0).normalize();

    // Transform view space direction to world space
    // View space: +X right, +Y up, -Z forward
    // World space: use camera's right, up, forward vectors
    let world_space_dir = (camera_right * view_space_dir.x
        + camera_up * view_space_dir.y
        + camera_forward * -view_space_dir.z)
        .normalize();

    // Cast ray from camera through mouse cursor
    let ray_origin = camera_pos;
    let ray_direction = world_space_dir;

    // Calculate intersection of ray with plane
    // Ray: P = ray_origin + t * ray_direction
    // Plane: dot(P - plane_point, plane_normal) = 0
    // Solving: dot(ray_origin + t * ray_direction - plane_point, plane_normal) = 0
    // t = dot(plane_point - ray_origin, plane_normal) / dot(ray_direction, plane_normal)
    let denominator = ray_direction.dot(plane_normal);

    // Avoid division by zero (ray parallel to plane)
    if denominator.abs() < 1e-6 {
        return;
    }

    let numerator = (plane_point - ray_origin).dot(plane_normal);
    let t = numerator / denominator;

    // Calculate intersection point
    let intersection_point = ray_origin + ray_direction * t;

    // Update transform to intersection point
    if let Some(initial_pos) = ui.grab_mode.initial_entity_pos {
        transform.translation = initial_pos;
    }
    if let Some(axis) = &ui.grab_mode.axis_mask {
        match axis {
            AxisMask::X => transform.translation.x = intersection_point.x,
            AxisMask::Y => transform.translation.y = intersection_point.y,
            AxisMask::Z => transform.translation.z = intersection_point.z,
        }
    } else {
        transform.translation = intersection_point;
    }
}

fn handle_grab_mode_exit(
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<UiState>,
    mut transforms: Query<&mut Transform>,
) {
    if !ui.grab_mode.is_active {
        return;
    }

    // Exit grab mode when left mouse button is clicked (confirms the move)
    if mouse_button.just_pressed(MouseButton::Left) {
        ui.grab_mode.is_active = false;
        ui.grab_mode.initial_mouse_pos = None;
        ui.grab_mode.initial_entity_pos = None;
    }

    // Exit grab mode when Escape is pressed (cancels the move and restores position)
    if keyboard_input.just_pressed(KeyCode::Escape) {
        if let Some(selected_entity) = ui.selected_entity
            && let Ok(mut transform) = transforms.get_mut(selected_entity)
            && let Some(initial_pos) = ui.grab_mode.initial_entity_pos
        {
            transform.translation = initial_pos;
        }
        ui.grab_mode.is_active = false;
        ui.grab_mode.initial_mouse_pos = None;
        ui.grab_mode.initial_entity_pos = None;
    }
}

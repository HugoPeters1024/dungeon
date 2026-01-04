use std::sync::Mutex;
use std::time::Duration;

use bevy::camera::Viewport;
use bevy::camera::visibility::RenderLayers;
use bevy::ecs::system::SystemId;
use bevy::mesh::Indices;
use bevy::picking::prelude::Pickable;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::{ecs::schedule::BoxedCondition, window::PrimaryWindow};
use bevy_egui::prelude::*;
use bevy_inspector_egui::bevy_inspector::{ui_for_entity_with_children, ui_for_world};
use bevy_panorbit_camera::PanOrbitCameraPlugin;
use egui::{LayerId, Sense};
use egui_dock::{DockArea, DockState, NodeIndex};

const CLICK_DURATION: Duration = Duration::from_millis(200);

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
        app.add_observer(test);

        app.add_systems(Update, draw_axes);

        app.insert_resource(UiState::new());
        app.init_resource::<GrabModeState>();

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

#[derive(Resource, Default, Deref)]
pub struct Prefabs {
    pub prefabs: HashMap<String, SystemId>,
}

impl Prefabs {
    pub fn add(&mut self, name: impl Into<String>, system: SystemId) {
        self.prefabs.insert(name.into(), system);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum EguiWindow {
    GameView,
    Prefabs,
    WorldInspector,
    SelectedInspector,
}

struct UiViewer<'a> {
    world: &'a mut World,
    viewport: &'a mut egui::Rect,
    prefabs: &'a Prefabs,
    pointer_in_viewport: &'a mut bool,
    selected_entity: &'a mut Option<Entity>,
}

impl egui_dock::TabViewer for UiViewer<'_> {
    type Tab = EguiWindow;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        format!("{tab:?}").into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            EguiWindow::GameView => {
                // Allocate space for the game view without consuming pointer events
                // This allows pointer events to pass through to entities behind the egui camera
                let available_size = ui.available_size();
                let response = ui.allocate_response(available_size, Sense::empty());
                *self.viewport = response.rect;
                // Track if pointer is in GameView viewport (matching the example's approach)
                *self.pointer_in_viewport = ui
                    .ctx()
                    .rect_contains_pointer(LayerId::background(), self.viewport.shrink(16.));
            }
            EguiWindow::Prefabs => {
                ui.label("Prefabs");
                for (name, on_click) in self.prefabs.iter() {
                    if ui.button(name).clicked() {
                        self.world.run_system(on_click.clone()).unwrap();
                    }
                }
            }
            EguiWindow::WorldInspector => {
                ui_for_world(self.world, ui);
            }
            EguiWindow::SelectedInspector => {
                if let Some(entity) = self.selected_entity {
                    if let Ok(child_of) = self.world.query::<&ChildOf>().get(self.world, *entity) {
                        if ui.button("Go to parent").clicked() {
                            *entity = child_of.0;
                        }
                    }
                    ui_for_entity_with_children(self.world, *entity, ui);
                } else {
                    ui.label("No entity selected");
                }
            }
        };
    }

    fn clear_background(&self, tab: &Self::Tab) -> bool {
        !matches!(tab, EguiWindow::GameView)
    }
}

#[derive(Resource)]
struct GrabModeState {
    is_active: bool,
    initial_mouse_pos: Option<Vec2>,
    initial_entity_pos: Option<Vec3>,
    axis_mask: u8,
}

impl Default for GrabModeState {
    fn default() -> Self {
        Self {
            is_active: false,
            initial_mouse_pos: None,
            initial_entity_pos: None,
            axis_mask: 0b111,
        }
    }
}

#[derive(Resource)]
struct UiState {
    state: DockState<EguiWindow>,
    viewport: egui::Rect,
    pointer_in_viewport: bool,
    selected_entity: Option<Entity>,
}

impl UiState {
    pub fn new() -> Self {
        let mut state = DockState::new(vec![EguiWindow::GameView, EguiWindow::WorldInspector]);
        let tree = state.main_surface_mut();
        let [_game, inspector] =
            tree.split_right(NodeIndex::root(), 0.75, vec![EguiWindow::SelectedInspector]);
        let [_inspector, _prefabs] = tree.split_below(inspector, 0.5, vec![EguiWindow::Prefabs]);

        Self {
            state,
            viewport: egui::Rect::NOTHING,
            pointer_in_viewport: false,
            selected_entity: None,
        }
    }

    pub fn ui(&mut self, world: &mut World, ctx: &mut egui::Context) {
        world.resource_scope::<Prefabs, _>(|world, prefabs| {
            let mut viewer = UiViewer {
                world,
                viewport: &mut self.viewport,
                prefabs: &prefabs,
                pointer_in_viewport: &mut self.pointer_in_viewport,
                selected_entity: &mut self.selected_entity,
            };

            DockArea::new(&mut self.state)
                .style(egui_dock::Style::from_egui(ctx.style().as_ref()))
                .show(ctx, &mut viewer);
        });
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
    window: Single<&Window, With<PrimaryWindow>>,
    mut cam: Single<&mut Camera, With<EditorCamera>>,
    egui_settings: Single<&EguiContextSettings>,
) {
    let scale_factor = window.scale_factor() * egui_settings.scale_factor;

    let viewport_pos = ui_state.viewport.left_top().to_vec2() * scale_factor;
    let viewport_size = ui_state.viewport.size() * scale_factor;

    let physical_position = UVec2::new(viewport_pos.x as u32, viewport_pos.y as u32);
    let physical_size = UVec2::new(viewport_size.x as u32, viewport_size.y as u32);

    let rect = physical_position + physical_size;

    let window_size = window.physical_size();
    if rect.x <= window_size.x && rect.y <= window_size.y {
        cam.viewport = Some(Viewport {
            physical_position,
            physical_size,
            depth: 0.0..1.0,
        });
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

fn test(
    mut trigger: On<Pointer<Click>>,
    names: Query<&Name>,
    windows: Query<&Window>,
    mut ui_state: ResMut<UiState>,
) {
    if !ui_state.pointer_in_viewport {
        return;
    }
    if trigger.button != PointerButton::Primary {
        return;
    }
    println!(
        "test function called entity={}, name={:?}",
        trigger.event_target(),
        names.get(trigger.event_target())
    );
    trigger.propagate(false);
    if trigger.duration < CLICK_DURATION {
        if windows.contains(trigger.event_target()) {
            ui_state.selected_entity = None;
        } else {
            ui_state.selected_entity = Some(trigger.event_target());
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

fn handle_grab_mode_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    ui_state: Res<UiState>,
    mut grab_mode: ResMut<GrabModeState>,
) {
    // Enter grab mode when 'G' is pressed and an entity is selected
    if keyboard_input.just_pressed(KeyCode::KeyG) && !grab_mode.is_active {
        if ui_state.selected_entity.is_some() && ui_state.pointer_in_viewport {
            grab_mode.is_active = true;
            grab_mode.initial_mouse_pos = None;
            grab_mode.initial_entity_pos = None;
            grab_mode.axis_mask = 0b111;
        }
    }

    if grab_mode.is_active {
        if keyboard_input.just_pressed(KeyCode::KeyX) {
            grab_mode.axis_mask = 0b100;
        }
        if keyboard_input.just_pressed(KeyCode::KeyY) {
            grab_mode.axis_mask = 0b010;
        }
        if keyboard_input.just_pressed(KeyCode::KeyZ) {
            grab_mode.axis_mask = 0b001;
        }
    }
}

fn handle_grab_mode_movement(
    mut grab_mode: ResMut<GrabModeState>,
    ui_state: Res<UiState>,
    mut transforms: Query<&mut Transform>,
    camera_query: Query<(&Camera, &Projection, &GlobalTransform), With<EditorCamera>>,
    window: Query<&Window, With<PrimaryWindow>>,
) {
    if !grab_mode.is_active {
        return;
    }

    // Only process movement when pointer is in viewport
    if !ui_state.pointer_in_viewport {
        return;
    }

    let Some(selected_entity) = ui_state.selected_entity else {
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
    if grab_mode.initial_mouse_pos.is_none() {
        grab_mode.initial_mouse_pos = Some(viewport_cursor);
        grab_mode.initial_entity_pos = Some(transform.translation);
    }

    let initial_pos = grab_mode.initial_entity_pos.unwrap();
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
    if let Some(initial_pos) = grab_mode.initial_entity_pos {
        transform.translation = initial_pos;
    }
    if (grab_mode.axis_mask & 0b100) != 0 {
        transform.translation.x = intersection_point.x;
    }
    if (grab_mode.axis_mask & 0b010) != 0 {
        transform.translation.y = intersection_point.y;
    }
    if (grab_mode.axis_mask & 0b001) != 0 {
        transform.translation.z = intersection_point.z;
    }
}

fn handle_grab_mode_exit(
    mut grab_mode: ResMut<GrabModeState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    ui_state: Res<UiState>,
    mut transforms: Query<&mut Transform>,
) {
    if !grab_mode.is_active {
        return;
    }

    // Exit grab mode when left mouse button is clicked (confirms the move)
    if mouse_button.just_pressed(MouseButton::Left) {
        grab_mode.is_active = false;
        grab_mode.initial_mouse_pos = None;
        grab_mode.initial_entity_pos = None;
    }

    // Exit grab mode when Escape is pressed (cancels the move and restores position)
    if keyboard_input.just_pressed(KeyCode::Escape) {
        if let Some(selected_entity) = ui_state.selected_entity {
            if let Ok(mut transform) = transforms.get_mut(selected_entity) {
                if let Some(initial_pos) = grab_mode.initial_entity_pos {
                    transform.translation = initial_pos;
                }
            }
        }
        grab_mode.is_active = false;
        grab_mode.initial_mouse_pos = None;
        grab_mode.initial_entity_pos = None;
    }
}

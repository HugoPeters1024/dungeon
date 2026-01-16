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
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};

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
            )
                .run_if(resource_exists::<Selected>),
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

fn on_click_in_void(
    trigger: On<Pointer<Click>>,
    mut commands: Commands,
    mut ui_state: ResMut<UiState>,
    windows: Query<&Window>,
    mut selected: Option<ResMut<Selected>>,
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
    windows: Query<&Window>
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
    mut camera_query: Query<&mut PanOrbitCamera, With<EditorCamera>>,
) {
    let entity = selected.entity;

    // F key: Focus camera on selected object
    if keyboard_input.just_pressed(KeyCode::KeyF) {
        if let Ok((_, global_transform)) = transforms.get(entity) {
            let target_pos = global_transform.translation();
            for mut pan_orbit in camera_query.iter_mut() {
                pan_orbit.target_focus = target_pos;
            }
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

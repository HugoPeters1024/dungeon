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
        cam.is_active = true;
        cam.viewport = Some(Viewport {
            physical_position,
            physical_size,
            depth: 0.0..1.0,
        });
    } else {
        cam.is_active = false;
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

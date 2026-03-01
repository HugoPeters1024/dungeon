use bevy::prelude::*;
use bevy_egui::egui;

use crate::{prefabs::Prefabs, scene::SceneCommands, ui::UiViewer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EguiWindow {
    GameView,
    Prefabs,
    WorldInspector,
    SelectedInspector,
    History,
    HierarchyGraph,
}

pub enum AxisMask {
    X,
    Y,
    Z,
}

impl AxisMask {
    /// Replace only the masked axis of `original` with the corresponding component from `new`.
    pub fn apply(&self, original: Vec3, new: Vec3) -> Vec3 {
        match self {
            AxisMask::X => original.with_x(new.x),
            AxisMask::Y => original.with_y(new.y),
            AxisMask::Z => original.with_z(new.z),
        }
    }

    /// Scale only the masked axis, leaving the others unchanged.
    pub fn apply_scale(&self, original: Vec3, factor: f32) -> Vec3 {
        match self {
            AxisMask::X => original.with_x(original.x * factor),
            AxisMask::Y => original.with_y(original.y * factor),
            AxisMask::Z => original.with_z(original.z * factor),
        }
    }

    pub fn from_key(key: KeyCode) -> Option<Self> {
        match key {
            KeyCode::KeyX => Some(AxisMask::X),
            KeyCode::KeyY => Some(AxisMask::Y),
            KeyCode::KeyZ => Some(AxisMask::Z),
            _ => None,
        }
    }
}

pub enum ContextMenu {
    Closed,
    Open {
        window_location: Vec2,
        hover_normal: HoverNormal,
    },
}

#[derive(Resource)]
pub struct UiDockState(egui_dock::DockState<EguiWindow>);

impl UiDockState {
    pub fn initialize() -> Self {
        let mut state =
            egui_dock::DockState::new(vec![EguiWindow::GameView, EguiWindow::WorldInspector]);
        let tree = state.main_surface_mut();
        let [_game, inspector] = tree.split_right(
            egui_dock::NodeIndex::root(),
            0.75,
            vec![EguiWindow::SelectedInspector],
        );
        let [_inspector, _prefabs] = tree.split_below(
            inspector,
            0.5,
            vec![
                EguiWindow::Prefabs,
                EguiWindow::History,
                EguiWindow::HierarchyGraph,
            ],
        );
        Self(state)
    }
}

pub enum SelectedAction {
    Grab {
        mask: Option<AxisMask>,
        initial_primary_pos: Vec3,
        initial_world_positions: Vec<(Entity, Vec3)>,
    },
    Scale {
        mask: Option<AxisMask>,
        initial_cursor_pos: Option<Vec2>,
        initial_world_scales: Vec<(Entity, Vec3)>,
    },
}

#[derive(Clone, Debug)]
pub struct HoverNormal {
    pub point: Vec3,
    pub normal: Vec3,
}

#[derive(Resource)]
pub struct Selected {
    pub entities: Vec<Entity>,
    pub hover_normal: Option<HoverNormal>,
    pub action: Option<SelectedAction>,
}

impl Selected {
    pub fn new(primary: Entity) -> Self {
        Self {
            entities: vec![primary],
            hover_normal: None,
            action: None,
        }
    }

    pub fn primary(&self) -> Entity {
        *self
            .entities
            .last()
            .expect("Selected must contain at least one entity")
    }

    pub fn is_selected(&self, entity: Entity) -> bool {
        self.entities.contains(&entity)
    }

    pub fn set_single(&mut self, entity: Entity) {
        self.entities.clear();
        self.entities.push(entity);
        self.hover_normal = None;
        self.action = None;
    }

    pub fn toggle(&mut self, entity: Entity) -> bool {
        if let Some(index) = self.entities.iter().position(|&e| e == entity) {
            self.entities.swap_remove(index);
        } else {
            self.entities.push(entity);
        }
        self.hover_normal = None;
        !self.entities.is_empty()
    }
}

#[derive(Resource)]
pub struct UiState {
    pub viewport: egui::Rect,
    pub pointer_in_viewport: bool,
    pub context_menu: ContextMenu,
    pub egui_wants_pointer_input: bool,
    pub spawn_distance: f32,
    pub prefab_search: String,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            viewport: egui::Rect::NOTHING,
            pointer_in_viewport: false,
            context_menu: ContextMenu::Closed,
            egui_wants_pointer_input: false,
            spawn_distance: 3.0,
            prefab_search: String::new(),
        }
    }
}

impl UiState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ui(&mut self, world: &mut World, ctx: &mut egui::Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui
                        .add(egui::Button::new("Save Scene").shortcut_text("Ctrl+S"))
                        .clicked()
                    {
                        world.resource_mut::<SceneCommands>().save_requested = true;
                        ui.close();
                    }
                    if ui
                        .add(egui::Button::new("Load Scene").shortcut_text("Ctrl+L"))
                        .clicked()
                    {
                        world.resource_mut::<SceneCommands>().load_requested = true;
                        ui.close();
                    }
                });
            });
        });

        world.resource_scope::<Prefabs, _>(|world, prefabs| {
            world.resource_scope::<UiDockState, _>(|world, mut dock_state| {
                let mut viewer = UiViewer {
                    world,
                    state: self,
                    prefabs: &prefabs,
                };

                egui_dock::DockArea::new(&mut dock_state.0)
                    .style(egui_dock::Style::from_egui(ctx.style().as_ref()))
                    .show(ctx, &mut viewer);

                self.egui_wants_pointer_input = ctx.wants_pointer_input();
            });
        });
    }
}

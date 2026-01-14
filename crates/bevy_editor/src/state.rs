use bevy::ecs::system::SystemId;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_egui::egui;

use crate::ui::UiViewer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EguiWindow {
    GameView,
    Prefabs,
    WorldInspector,
    SelectedInspector,
}

pub enum AxisMask {
    X,
    Y,
    Z,
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
        let [_inspector, _prefabs] = tree.split_below(inspector, 0.5, vec![EguiWindow::Prefabs]);
        Self(state)
    }
}

pub enum SelectedAction {
    Grab {
        mask: Option<AxisMask>,
        initial_mouse_pos: Option<Vec2>,
        initial_entity_pos: Vec3,
    },
}

#[derive(Clone)]
pub struct HoverNormal {
    pub point: Vec3,
    pub normal: Vec3,
}

#[derive(Resource)]
pub struct Selected {
    pub entity: Entity,
    pub hover_normal: Option<HoverNormal>,
    pub action: Option<SelectedAction>,
}

#[derive(Resource)]
pub struct UiState {
    pub viewport: egui::Rect,
    pub pointer_in_viewport: bool,
    pub context_menu: ContextMenu,
    pub egui_wants_pointer_input: bool,
    /// Distance from camera at which new prefabs are spawned
    pub spawn_distance: f32,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            viewport: egui::Rect::NOTHING,
            pointer_in_viewport: false,
            context_menu: ContextMenu::Closed,
            egui_wants_pointer_input: false,
            spawn_distance: 3.0,
        }
    }
}

impl UiState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ui(&mut self, world: &mut World, ctx: &mut egui::Context) {
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

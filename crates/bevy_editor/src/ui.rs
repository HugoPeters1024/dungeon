use bevy::prelude::*;
use bevy_egui::egui;
use bevy_inspector_egui::bevy_inspector::{ui_for_entity_with_children, ui_for_world};

use crate::state::{ContextMenu, EguiWindow, Prefabs, UiState};

pub struct UiViewer<'a> {
    pub world: &'a mut World,
    pub state: &'a mut UiState,
    pub prefabs: &'a Prefabs,
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
                let response = ui.allocate_response(available_size, egui::Sense::empty());
                self.state.viewport = response.rect;
                // Track if pointer is in GameView viewport (matching the example's approach)
                self.state.pointer_in_viewport = ui.ctx().rect_contains_pointer(
                    egui::LayerId::background(),
                    self.state.viewport.shrink(16.),
                );

                match self.state.context_menu {
                    ContextMenu::Closed => {}
                    ContextMenu::Open(vec2) => {
                        egui::Area::new(egui::Id::new("context_menu"))
                            .fixed_pos(egui::Pos2 {
                                x: vec2.x,
                                y: vec2.y,
                            })
                            .show(ui.ctx(), |ui| {
                                egui::Frame::popup(ui.style()).show(ui, |ui| {
                                    if self.state.selected_entity.is_some() {
                                        if ui.button("Duplicate").clicked() {}
                                    } else {
                                        if ui.button("Option 1").clicked() {
                                            // Handle option 1
                                        }
                                        if ui.button("Option 2").clicked() {
                                            // Handle option 2
                                        }
                                        if ui.button("Option 3").clicked() {
                                            // Handle option 3
                                        }
                                    }
                                });
                            });
                    }
                }
            }
            EguiWindow::Prefabs => {
                ui.label("Prefabs");
                for (name, on_click) in self.prefabs.iter() {
                    if ui.button(name).clicked() {
                        self.world.run_system(*on_click).unwrap();
                    }
                }
            }
            EguiWindow::WorldInspector => {
                ui_for_world(self.world, ui);
            }
            EguiWindow::SelectedInspector => {
                if let Some(entity) = self.state.selected_entity.as_mut()
                    && let Ok(child_of) = self.world.query::<&ChildOf>().get(self.world, *entity)
                {
                    if ui.button("Go to parent").clicked() {
                        *entity = child_of.0;
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

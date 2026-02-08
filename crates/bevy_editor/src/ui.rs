use bevy::prelude::*;
use bevy_egui::egui;
use bevy_inspector_egui::bevy_inspector::{ui_for_entity_with_children, ui_for_world};

use crate::{
    ActionQueue, ContextMenu, DuplicateAction, EditorAction, EditorCamera, FocusCameraAction,
    MergeAction, Selected,
    prefabs::Prefabs,
    state::{EguiWindow, PendingPrefabSpawns, UiState},
};

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

                let mut should_close = false;
                let mut pending_actions: Vec<EditorAction> = Vec::new();

                match &self.state.context_menu {
                    ContextMenu::Closed => {}
                    ContextMenu::Open {
                        window_location,
                        hover_normal,
                    } => {
                        let hover_normal = hover_normal.clone();
                        egui::Area::new(egui::Id::new("context_menu"))
                            .fixed_pos(egui::Pos2 {
                                x: window_location.x,
                                y: window_location.y,
                            })
                            .show(ui.ctx(), |ui| {
                                egui::Frame::popup(ui.style()).show(ui, |ui| {
                                    if let Some(selected) = self.world.get_resource::<Selected>() {
                                        let entity = selected.primary();
                                        let selection_count = selected.entities.len();

                                        // Duplicate only available when exactly 1 entity is selected
                                        if selection_count == 1 && ui.button("Duplicate").clicked()
                                        {
                                            pending_actions.push(
                                                DuplicateAction::new(entity, hover_normal.normal).into(),
                                            );
                                            should_close = true;
                                        }

                                        // Merge available when multiple entities are selected
                                        if selection_count > 1 && ui.button("Merge").clicked() {
                                            pending_actions.push(
                                            MergeAction::new(selected.entities.clone())
                                                .into(),
                                            );
                                            should_close = true;
                                        }

                                        if ui.button("Lock Camera Onto").clicked() {
                                            // Get entity position first
                                            let new_position = self
                                                .world
                                                .get::<GlobalTransform>(entity)
                                                .map(|t| t.translation());

                                            if let Some(new_position) = new_position {
                                                // Get current camera focus for undo support
                                                let old_position = self
                                                    .world
                                                    .query_filtered::<
                                                        &bevy_panorbit_camera::PanOrbitCamera,
                                                        With<EditorCamera>,
                                                    >()
                                                    .iter(self.world)
                                                    .next()
                                                    .map(|cam| cam.target_focus)
                                                    .unwrap_or(Vec3::ZERO);

                                                pending_actions.push(
                                                    FocusCameraAction {
                                                        old_position,
                                                        new_position,
                                                    }
                                                    .into(),
                                                );
                                            }
                                            should_close = true;
                                        }
                                    }
                                });
                            });
                    }
                }

                // Queue any pending actions
                if !pending_actions.is_empty()
                    && let Some(mut action_queue) = self.world.get_resource_mut::<ActionQueue>()
                {
                    for action in pending_actions {
                        action_queue.push(action);
                    }
                }

                if should_close {
                    self.state.context_menu = ContextMenu::Closed;
                }

                // Show selected object position at bottom right of game view
                if let Some(selected) = self.world.get_resource::<Selected>()
                    && let Some(transform) = self.world.get::<Transform>(selected.primary())
                {
                    let pos = transform.translation;
                    let text = format!("X: {:.2}  Y: {:.2}  Z: {:.2}", pos.x, pos.y, pos.z);
                    let padding = 8.0;
                    // Estimate text width based on character count (monospace)
                    let char_width = 10.0;
                    let text_width = text.len() as f32 * char_width;
                    let text_height = 16.0;
                    let pos_x = self.state.viewport.right() - text_width - padding - 12.0;
                    let pos_y = self.state.viewport.bottom() - text_height - padding - 12.0;

                    egui::Area::new(egui::Id::new("selected_position_overlay"))
                        .fixed_pos(egui::pos2(pos_x, pos_y))
                        .show(ui.ctx(), |ui| {
                            egui::Frame::new()
                                .fill(egui::Color32::from_black_alpha(180))
                                .corner_radius(3.0)
                                .inner_margin(egui::Margin::same(4))
                                .show(ui, |ui| {
                                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                                    ui.label(
                                        egui::RichText::new(text)
                                            .size(11.0)
                                            .color(egui::Color32::WHITE)
                                            .monospace(),
                                    );
                                });
                        });
                }
            }
            EguiWindow::Prefabs => {
                ui.label("Prefabs");
                for id in self.prefabs.get_prefab_ids() {
                    if ui.button(id.name()).clicked() {
                        // Queue the spawn to happen after all resource_scopes end
                        self.world
                            .resource_mut::<PendingPrefabSpawns>()
                            .0
                            .push(id.clone());
                    }
                }
            }
            EguiWindow::WorldInspector => {
                ui_for_world(self.world, ui);
            }
            EguiWindow::SelectedInspector => {
                self.world
                    .try_resource_scope::<Selected, ()>(|world, mut selected| {
                        if let Ok(child_of) =
                            world.query::<&ChildOf>().get(world, selected.primary())
                            && ui.button("Go to parent").clicked()
                        {
                            selected.set_single(child_of.0);
                        }
                        ui_for_entity_with_children(world, selected.primary(), ui);
                    });
            }
            EguiWindow::History => {
                self.world
                    .resource_scope::<ActionQueue, ()>(|_world, action_queue| {
                        let history_index = action_queue.history_index();
                        let history_len = action_queue.history_len();

                        // Show undo/redo status
                        ui.horizontal(|ui| {
                            ui.label(format!("{}/{}", history_index, history_len));
                            ui.separator();
                            if action_queue.can_undo() {
                                ui.label("Ctrl+Z: undo");
                            }
                            if action_queue.can_redo() {
                                ui.label("Ctrl+Y: redo");
                            }
                        });
                        ui.separator();

                        // Show recent history with current position indicator
                        let start = history_index.saturating_sub(5);
                        let end = (history_index + 3).min(history_len);

                        for (i, (action, is_undone)) in action_queue.iter_history().enumerate() {
                            if i < start || i >= end {
                                continue;
                            }
                            let is_current = i == history_index.saturating_sub(1) && history_index > 0;

                            let text = if is_current {
                                format!("> {}", action.name())
                            } else if is_undone {
                                format!("  {} (undone)", action.name())
                            } else {
                                format!("  {}", action.name())
                            };

                            let color = if is_undone {
                                egui::Color32::GRAY
                            } else if is_current {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::LIGHT_GRAY
                            };

                            ui.label(egui::RichText::new(text).color(color));
                        }
                    });
            }
        };
    }

    fn clear_background(&self, tab: &Self::Tab) -> bool {
        !matches!(tab, EguiWindow::GameView)
    }
}

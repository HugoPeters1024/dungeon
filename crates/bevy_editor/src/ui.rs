use bevy::prelude::*;
use bevy_egui::egui;
use bevy_inspector_egui::bevy_inspector::{ui_for_entity_with_children, ui_for_world};
use bevy_panorbit_camera::PanOrbitCamera;

use crate::{
    ContextMenu, EditorCamera, Selected, SpawnPosition,
    state::{EguiWindow, Prefabs, UiState},
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
                match &self.state.context_menu {
                    ContextMenu::Closed => {}
                    ContextMenu::Open {
                        window_location,
                        hover_normal,
                    } => {
                        egui::Area::new(egui::Id::new("context_menu"))
                            .fixed_pos(egui::Pos2 {
                                x: window_location.x,
                                y: window_location.y,
                            })
                            .show(ui.ctx(), |ui| {
                                egui::Frame::popup(ui.style()).show(ui, |ui| {
                                    self.world.try_resource_scope::<Selected, ()>(
                                        |world, selected| {
                                            if ui.button("Duplicate").clicked() {
                                                let new_entity = world
                                                    .entity_mut(selected.entity)
                                                    .clone_and_spawn();
                                                if let Ok(mut transform) = world
                                                    .query::<&mut Transform>()
                                                    .get_mut(world, new_entity)
                                                {
                                                    transform.translation += hover_normal.normal;
                                                }
                                                should_close = true;
                                            }

                                            if ui.button("Lock Camera Onto").clicked() {
                                                // Get the global transform of the selected entity
                                                if let Some(global_transform) =
                                                    world.get::<GlobalTransform>(selected.entity)
                                                {
                                                    let target_pos = global_transform.translation();
                                                    // Find the editor camera and set its target_focus
                                                    let mut query = world
                                                        .query_filtered::<&mut PanOrbitCamera, With<EditorCamera>>();
                                                    for mut pan_orbit in query.iter_mut(world) {
                                                        pan_orbit.target_focus = target_pos;
                                                    }
                                                }
                                                should_close = true;
                                            }
                                        },
                                    );
                                });
                            });
                    }
                }

                if should_close {
                    self.state.context_menu = ContextMenu::Closed;
                }

                // Show selected object position at bottom right of game view
                if let Some(selected) = self.world.get_resource::<Selected>() {
                    if let Some(transform) = self.world.get::<Transform>(selected.entity) {
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
            }
            EguiWindow::Prefabs => {
                ui.label("Prefabs");
                for (name, on_click) in self.prefabs.iter() {
                    if ui.button(name).clicked() {
                        // Calculate spawn position from editor camera
                        let spawn_pos = self
                            .world
                            .query_filtered::<&Transform, With<EditorCamera>>()
                            .iter(self.world)
                            .next()
                            .map(|cam_transform| {
                                let forward = cam_transform.forward();
                                cam_transform.translation + *forward * self.state.spawn_distance
                            })
                            .unwrap_or(Vec3::ZERO);

                        // Set the spawn position resource so prefab systems can use it
                        self.world.insert_resource(SpawnPosition(spawn_pos));

                        // Run the spawn system
                        self.world.run_system(*on_click).unwrap();
                    }
                }
            }
            EguiWindow::WorldInspector => {
                ui_for_world(self.world, ui);
            }
            EguiWindow::SelectedInspector => {
                self.world
                    .try_resource_scope::<Selected, ()>(|world, mut selected| {
                        if let Ok(child_of) = world.query::<&ChildOf>().get(world, selected.entity)
                            && ui.button("Go to parent").clicked()
                        {
                            selected.entity = child_of.0;
                        }
                        ui_for_entity_with_children(world, selected.entity, ui);
                    });
            }
        };
    }

    fn clear_background(&self, tab: &Self::Tab) -> bool {
        !matches!(tab, EguiWindow::GameView)
    }
}

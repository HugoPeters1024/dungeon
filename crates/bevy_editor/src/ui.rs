use bevy::prelude::*;
use bevy_egui::egui;
use bevy_inspector_egui::bevy_inspector::{ui_for_entity_with_children, ui_for_world};

use crate::{
    ContextMenu, EditorCamera, Selected,
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
                                        },
                                    );
                                });
                            });
                    }
                }

                if should_close {
                    self.state.context_menu = ContextMenu::Closed;
                }
            }
            EguiWindow::Prefabs => {
                ui.label("Prefabs");
                for (name, on_click) in self.prefabs.iter() {
                    if ui.button(name).clicked() {
                        // Get the editor camera transform to calculate spawn position
                        let spawn_position = self
                            .world
                            .query_filtered::<&Transform, With<EditorCamera>>()
                            .iter(self.world)
                            .next()
                            .map(|cam_transform| {
                                let forward = cam_transform.forward();
                                cam_transform.translation + *forward * self.state.spawn_distance
                            });

                        // Collect existing entities with transforms before spawning
                        let existing_entities: Vec<Entity> = self
                            .world
                            .query_filtered::<Entity, With<Transform>>()
                            .iter(self.world)
                            .collect();

                        // Run the spawn system
                        self.world.run_system(*on_click).unwrap();

                        // Find and move newly spawned entities to the camera spawn position
                        if let Some(spawn_pos) = spawn_position {
                            let new_entities: Vec<Entity> = self
                                .world
                                .query_filtered::<Entity, With<Transform>>()
                                .iter(self.world)
                                .filter(|e| !existing_entities.contains(e))
                                .collect();

                            // Move only root entities (those without a parent) to spawn position
                            for entity in new_entities {
                                let has_parent = self
                                    .world
                                    .query::<&ChildOf>()
                                    .get(self.world, entity)
                                    .is_ok();

                                if !has_parent {
                                    if let Ok(mut transform) = self
                                        .world
                                        .query::<&mut Transform>()
                                        .get_mut(self.world, entity)
                                    {
                                        transform.translation = spawn_pos;
                                    }
                                }
                            }
                        }
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

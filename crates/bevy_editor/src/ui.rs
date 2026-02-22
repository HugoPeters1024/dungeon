use bevy::prelude::*;
use bevy_egui::egui;
use bevy_inspector_egui::bevy_inspector::{ui_for_entity_with_children, ui_for_world};

use crate::{
    ActionQueue, ContextMenu, DuplicateAction, EditorAction, EditorCamera, FocusCameraAction,
    MergeAction, RemoveAction, Selected, SpawnPrefabAction,
    prefabs::Prefabs,
    state::{EguiWindow, UiState},
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
                                    // Extract all needed data from Selected before any mutable borrows
                                    let selection_data =
                                        self.world.get_resource::<Selected>().map(|selected| {
                                            (
                                                selected.primary(),
                                                selected.entities.len(),
                                                selected.entities.clone(),
                                            )
                                        });

                                    if let Some((entity, selection_count, entities)) =
                                        selection_data
                                    {
                                        // Duplicate only available when exactly 1 entity is selected
                                        if selection_count == 1 && ui.button("Duplicate").clicked()
                                        {
                                            pending_actions.push(
                                                DuplicateAction::new(entity, hover_normal.normal)
                                                    .into(),
                                            );
                                            should_close = true;
                                        }

                                        // Merge available when multiple entities are selected
                                        if selection_count > 1 && ui.button("Merge").clicked() {
                                            pending_actions
                                                .push(MergeAction::new(entities.clone()).into());
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

                                        if ui.button("Remove").clicked() {
                                            for entity in entities {
                                                pending_actions
                                                    .push(RemoveAction::new(entity).into());
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

                // Orientation gizmo in top-right corner
                {
                    enum GizmoInteraction {
                        Drag(egui::Vec2),
                        SnapAxis { yaw: f32, pitch: f32 },
                    }

                    let gizmo_radius: f32 = 45.0;
                    let gizmo_pad: f32 = 12.0;
                    let axis_len = gizmo_radius * 0.75;

                    let cam_rotation = self
                        .world
                        .query_filtered::<
                            &bevy_panorbit_camera::PanOrbitCamera,
                            With<EditorCamera>,
                        >()
                        .iter(self.world)
                        .next()
                        .map(|cam| {
                            let yaw = cam.yaw.unwrap_or(cam.target_yaw);
                            let pitch = cam.pitch.unwrap_or(cam.target_pitch);
                            Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch)
                        });

                    if let Some(rotation) = cam_rotation {
                        let inv = rotation.inverse();

                        let axis_defs: [(Vec3, egui::Color32, &str, &str); 3] = [
                            (Vec3::X, egui::Color32::from_rgb(230, 70, 70), "X", "-X"),
                            (Vec3::Y, egui::Color32::from_rgb(100, 210, 70), "Y", "-Y"),
                            (Vec3::Z, egui::Color32::from_rgb(70, 120, 255), "Z", "-Z"),
                        ];

                        // (screen_x, screen_y, depth, color, label, endpoint_radius)
                        let mut ends: Vec<(f32, f32, f32, egui::Color32, &str, f32)> =
                            Vec::with_capacity(6);
                        for &(axis, color, pos_label, neg_label) in &axis_defs {
                            let v = inv * axis;
                            ends.push((v.x, -v.y, v.z, color, pos_label, 10.0));
                            let nv = inv * (-axis);
                            let dim = egui::Color32::from_rgba_unmultiplied(
                                color.r() / 2,
                                color.g() / 2,
                                color.b() / 2,
                                180,
                            );
                            ends.push((nv.x, -nv.y, nv.z, dim, neg_label, 8.0));
                        }
                        ends.sort_by(|a, b| {
                            a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal)
                        });

                        let total = (gizmo_radius + gizmo_pad) * 2.0;
                        let area_x = self.state.viewport.right() - total - gizmo_pad;
                        let area_y = self.state.viewport.top() + gizmo_pad;

                        let area_resp = egui::Area::new(egui::Id::new("orientation_gizmo"))
                            .fixed_pos(egui::pos2(area_x, area_y))
                            .show(ui.ctx(), |ui| {
                                let (resp, painter) = ui.allocate_painter(
                                    egui::vec2(total, total),
                                    egui::Sense::click_and_drag(),
                                );
                                let c = resp.rect.center();

                                painter.circle_filled(
                                    c,
                                    gizmo_radius + 4.0,
                                    egui::Color32::from_black_alpha(140),
                                );
                                painter.circle_stroke(
                                    c,
                                    gizmo_radius + 4.0,
                                    egui::Stroke::new(1.0, egui::Color32::from_white_alpha(30)),
                                );

                                // Lines (back to front)
                                for &(sx, sy, _, color, _, radius) in &ends {
                                    let ep = egui::pos2(
                                        c.x + sx * axis_len,
                                        c.y + sy * axis_len,
                                    );
                                    let width = if radius >= 10.0 { 2.5 } else { 1.5 };
                                    painter.line_segment(
                                        [c, ep],
                                        egui::Stroke::new(width, color),
                                    );
                                }

                                // Endpoints on top (back to front)
                                for &(sx, sy, _, color, label, radius) in &ends {
                                    let ep = egui::pos2(
                                        c.x + sx * axis_len,
                                        c.y + sy * axis_len,
                                    );
                                    painter.circle_filled(ep, radius, color);
                                    let font_size = if radius >= 10.0 { 11.0 } else { 9.0 };
                                    painter.text(
                                        ep,
                                        egui::Align2::CENTER_CENTER,
                                        label,
                                        egui::FontId::proportional(font_size),
                                        egui::Color32::WHITE,
                                    );
                                }

                                painter.circle_filled(
                                    c,
                                    3.0,
                                    egui::Color32::from_white_alpha(100),
                                );

                                if resp.dragged() {
                                    Some(GizmoInteraction::Drag(resp.drag_delta()))
                                } else if resp.clicked() {
                                    resp.interact_pointer_pos().and_then(|pos| {
                                        ends.iter()
                                            .rev()
                                            .find(|&&(sx, sy, _, _, _, radius)| {
                                                let ep = egui::pos2(
                                                    c.x + sx * axis_len,
                                                    c.y + sy * axis_len,
                                                );
                                                pos.distance(ep) <= radius
                                            })
                                            .map(|&(_, _, _, _, lbl, _)| {
                                                use std::f32::consts::{FRAC_PI_2, PI};
                                                match lbl {
                                                    "X" => GizmoInteraction::SnapAxis {
                                                        yaw: FRAC_PI_2,
                                                        pitch: 0.0,
                                                    },
                                                    "-X" => GizmoInteraction::SnapAxis {
                                                        yaw: -FRAC_PI_2,
                                                        pitch: 0.0,
                                                    },
                                                    "Y" => GizmoInteraction::SnapAxis {
                                                        yaw: 0.0,
                                                        pitch: FRAC_PI_2,
                                                    },
                                                    "-Y" => GizmoInteraction::SnapAxis {
                                                        yaw: 0.0,
                                                        pitch: -FRAC_PI_2,
                                                    },
                                                    "Z" => GizmoInteraction::SnapAxis {
                                                        yaw: 0.0,
                                                        pitch: 0.0,
                                                    },
                                                    "-Z" => GizmoInteraction::SnapAxis {
                                                        yaw: PI,
                                                        pitch: 0.0,
                                                    },
                                                    _ => unreachable!(),
                                                }
                                            })
                                    })
                                } else {
                                    None
                                }
                            });

                        match area_resp.inner {
                            Some(GizmoInteraction::Drag(delta)) => {
                                let sensitivity = 0.01;
                                let mut q = self.world.query_filtered::<
                                    &mut bevy_panorbit_camera::PanOrbitCamera,
                                    With<EditorCamera>,
                                >();
                                for mut cam in q.iter_mut(self.world) {
                                    cam.target_yaw -= delta.x * sensitivity;
                                    cam.target_pitch += delta.y * sensitivity;
                                }
                            }
                            Some(GizmoInteraction::SnapAxis { yaw, pitch }) => {
                                let mut q = self.world.query_filtered::<
                                    &mut bevy_panorbit_camera::PanOrbitCamera,
                                    With<EditorCamera>,
                                >();
                                for mut cam in q.iter_mut(self.world) {
                                    cam.target_yaw = yaw;
                                    cam.target_pitch = pitch;
                                }
                            }
                            None => {}
                        }
                    }
                }
            }
            EguiWindow::Prefabs => {
                ui.label("Prefabs");

                // Calculate spawn position from editor camera
                let spawn_pos = self
                    .world
                    .query_filtered::<&Transform, With<EditorCamera>>()
                    .iter(self.world)
                    .next()
                    .map(|cam_transform| {
                        let forward = cam_transform.forward();
                        cam_transform.translation + *forward * 3.0
                    })
                    .unwrap_or(Vec3::ZERO);

                let mut action_queue = self.world.resource_mut::<ActionQueue>();
                for id in self.prefabs.get_prefab_ids() {
                    if ui.button(id.name()).clicked() {
                        action_queue.push(SpawnPrefabAction::new(id.clone(), spawn_pos).into());
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
                            let is_current =
                                i == history_index.saturating_sub(1) && history_index > 0;

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

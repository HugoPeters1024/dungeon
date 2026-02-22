use std::collections::HashMap;

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

                crate::screen_grid::show(ui.ctx(), self.state.viewport, self.world);

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

                crate::orientation_gizmo::show(ui.ctx(), self.state.viewport, self.world);
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
            EguiWindow::HierarchyGraph => {
                hierarchy_graph_tab(ui, self.world);
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

struct GraphNode {
    entity: Entity,
    label: String,
    depth: usize,
    children: Vec<usize>,
}

fn find_hierarchy_root(world: &World, entity: Entity) -> Entity {
    let mut current = entity;
    while let Some(child_of) = world.get::<ChildOf>(current) {
        current = child_of.parent();
    }
    current
}

fn collect_hierarchy(world: &World, root: Entity) -> Vec<GraphNode> {
    let mut nodes = Vec::new();
    collect_recursive(world, root, 0, &mut nodes);
    nodes
}

fn collect_recursive(
    world: &World,
    entity: Entity,
    depth: usize,
    nodes: &mut Vec<GraphNode>,
) -> usize {
    let idx = nodes.len();

    let label = world
        .get::<Name>(entity)
        .map(|n| n.as_str().to_string())
        .unwrap_or_else(|| format!("{}", entity));

    nodes.push(GraphNode {
        entity,
        label,
        depth,
        children: Vec::new(),
    });

    if let Some(children) = world.get::<Children>(entity) {
        let child_entities: Vec<Entity> = children.iter().collect();
        for child in child_entities {
            let child_idx = collect_recursive(world, child, depth + 1, nodes);
            nodes[idx].children.push(child_idx);
        }
    }

    idx
}

fn compute_subtree_width(nodes: &[GraphNode], idx: usize) -> f32 {
    let children = &nodes[idx].children;
    if children.is_empty() {
        return 1.0;
    }
    children
        .iter()
        .map(|&c| compute_subtree_width(nodes, c))
        .sum()
}

fn assign_positions(
    nodes: &[GraphNode],
    idx: usize,
    x_start: f32,
    node_width: f32,
    level_height: f32,
    positions: &mut HashMap<usize, egui::Pos2>,
) {
    let subtree_w = compute_subtree_width(nodes, idx);
    let center_x = x_start + subtree_w * node_width / 2.0;
    let y = nodes[idx].depth as f32 * level_height;
    positions.insert(idx, egui::pos2(center_x, y));

    let mut child_x = x_start;
    for &child_idx in &nodes[idx].children {
        let child_w = compute_subtree_width(nodes, child_idx);
        assign_positions(nodes, child_idx, child_x, node_width, level_height, positions);
        child_x += child_w * node_width;
    }
}

fn hierarchy_graph_tab(ui: &mut egui::Ui, world: &mut World) {
    let selected_entity = world
        .get_resource::<Selected>()
        .map(|s| s.primary());

    let Some(selected) = selected_entity else {
        ui.centered_and_justified(|ui| {
            ui.label("No entity selected");
        });
        return;
    };

    let root = find_hierarchy_root(world, selected);
    let nodes = collect_hierarchy(world, root);

    if nodes.is_empty() {
        return;
    }

    let node_width = 140.0_f32;
    let node_height = 28.0_f32;
    let level_height = 60.0_f32;

    let mut positions: HashMap<usize, egui::Pos2> = HashMap::new();
    assign_positions(&nodes, 0, 0.0, node_width, level_height, &mut positions);

    let max_depth = nodes.iter().map(|n| n.depth).max().unwrap_or(0);
    let total_width = compute_subtree_width(&nodes, 0) * node_width;
    let total_height = (max_depth + 1) as f32 * level_height;

    let padding = 20.0;

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let (response, painter) = ui.allocate_painter(
                egui::vec2(total_width + padding * 2.0, total_height + padding * 2.0),
                egui::Sense::click(),
            );
            let origin = response.rect.min + egui::vec2(padding, padding + node_height / 2.0);

            let edge_color = ui.visuals().text_color().gamma_multiply(0.4);

            for (idx, node) in nodes.iter().enumerate() {
                let parent_pos = positions[&idx];
                let parent_screen = origin + egui::vec2(parent_pos.x, parent_pos.y);

                for &child_idx in &node.children {
                    let child_pos = positions[&child_idx];
                    let child_screen = origin + egui::vec2(child_pos.x, child_pos.y);

                    let mid_y = (parent_screen.y + node_height / 2.0 + child_screen.y - node_height / 2.0) / 2.0;

                    let points = vec![
                        egui::pos2(parent_screen.x, parent_screen.y + node_height / 2.0),
                        egui::pos2(parent_screen.x, mid_y),
                        egui::pos2(child_screen.x, mid_y),
                        egui::pos2(child_screen.x, child_screen.y - node_height / 2.0),
                    ];

                    for pair in points.windows(2) {
                        painter.line_segment(
                            [pair[0], pair[1]],
                            egui::Stroke::new(1.5, edge_color),
                        );
                    }
                }
            }

            let mut clicked_entity = None;

            for (idx, node) in nodes.iter().enumerate() {
                let pos = positions[&idx];
                let screen_pos = origin + egui::vec2(pos.x, pos.y);

                let node_rect = egui::Rect::from_center_size(
                    egui::pos2(screen_pos.x, screen_pos.y),
                    egui::vec2(node_width - 10.0, node_height),
                );

                let is_selected = node.entity == selected;
                let (bg, border, text_color) = if is_selected {
                    (
                        egui::Color32::from_rgb(50, 90, 160),
                        egui::Color32::from_rgb(100, 160, 255),
                        egui::Color32::WHITE,
                    )
                } else {
                    (
                        ui.visuals().widgets.inactive.bg_fill,
                        ui.visuals().widgets.inactive.bg_stroke.color,
                        ui.visuals().text_color(),
                    )
                };

                painter.rect(
                    node_rect,
                    egui::CornerRadius::same(4),
                    bg,
                    egui::Stroke::new(if is_selected { 2.0 } else { 1.0 }, border),
                    egui::epaint::StrokeKind::Outside,
                );

                let truncated = if node.label.len() > 16 {
                    format!("{}...", &node.label[..13])
                } else {
                    node.label.clone()
                };

                painter.text(
                    node_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &truncated,
                    egui::FontId::proportional(11.0),
                    text_color,
                );

                if response.clicked() {
                    if let Some(pointer) = response.interact_pointer_pos() {
                        if node_rect.contains(pointer) {
                            clicked_entity = Some(node.entity);
                        }
                    }
                }
            }

            if let Some(entity) = clicked_entity {
                if let Some(mut sel) = world.get_resource_mut::<Selected>() {
                    sel.set_single(entity);
                }
            }
        });
}

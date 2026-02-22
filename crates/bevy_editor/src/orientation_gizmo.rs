use std::f32::consts::{FRAC_PI_2, PI};

use bevy::prelude::*;
use bevy_egui::egui;

use crate::EditorCamera;

const GIZMO_RADIUS: f32 = 45.0;
const GIZMO_PAD: f32 = 12.0;
const AXIS_LEN: f32 = GIZMO_RADIUS * 0.75;
const POS_ENDPOINT_RADIUS: f32 = 10.0;
const NEG_ENDPOINT_RADIUS: f32 = 8.0;
const DRAG_SENSITIVITY: f32 = 0.01;

const AXIS_DEFS: [(Vec3, [u8; 3], &str, &str, f32, f32); 3] = [
    (Vec3::X, [230, 70, 70], "X", "-X", FRAC_PI_2, 0.0),
    (Vec3::Y, [100, 210, 70], "Y", "-Y", 0.0, FRAC_PI_2),
    (Vec3::Z, [70, 120, 255], "Z", "-Z", 0.0, 0.0),
];

enum Interaction {
    Drag(egui::Vec2),
    SnapAxis { yaw: f32, pitch: f32 },
}

struct AxisEnd {
    screen_x: f32,
    screen_y: f32,
    depth: f32,
    color: egui::Color32,
    label: &'static str,
    radius: f32,
    snap_yaw: f32,
    snap_pitch: f32,
}

fn project_axes(inv_rotation: Quat) -> Vec<AxisEnd> {
    let mut ends = Vec::with_capacity(6);
    for &(axis, rgb, pos_label, neg_label, snap_yaw, snap_pitch) in &AXIS_DEFS {
        let color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
        let v = inv_rotation * axis;
        ends.push(AxisEnd {
            screen_x: v.x,
            screen_y: -v.y,
            depth: v.z,
            color,
            label: pos_label,
            radius: POS_ENDPOINT_RADIUS,
            snap_yaw,
            snap_pitch,
        });

        let nv = inv_rotation * (-axis);
        let dim = egui::Color32::from_rgba_unmultiplied(rgb[0] / 2, rgb[1] / 2, rgb[2] / 2, 180);
        let (neg_yaw, neg_pitch) = negate_snap(snap_yaw, snap_pitch, neg_label);
        ends.push(AxisEnd {
            screen_x: nv.x,
            screen_y: -nv.y,
            depth: nv.z,
            color: dim,
            label: neg_label,
            radius: NEG_ENDPOINT_RADIUS,
            snap_yaw: neg_yaw,
            snap_pitch: neg_pitch,
        });
    }
    ends.sort_by(|a, b| {
        a.depth
            .partial_cmp(&b.depth)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ends
}

fn negate_snap(pos_yaw: f32, pos_pitch: f32, neg_label: &str) -> (f32, f32) {
    match neg_label {
        "-X" => (-pos_yaw, pos_pitch),
        "-Y" => (pos_yaw, -pos_pitch),
        "-Z" => (PI, pos_pitch),
        _ => unreachable!(),
    }
}

fn paint(painter: &egui::Painter, center: egui::Pos2, ends: &[AxisEnd]) {
    painter.circle_filled(
        center,
        GIZMO_RADIUS + 4.0,
        egui::Color32::from_black_alpha(140),
    );
    painter.circle_stroke(
        center,
        GIZMO_RADIUS + 4.0,
        egui::Stroke::new(1.0, egui::Color32::from_white_alpha(30)),
    );

    for end in ends {
        let ep = egui::pos2(
            center.x + end.screen_x * AXIS_LEN,
            center.y + end.screen_y * AXIS_LEN,
        );
        let width = if end.radius >= POS_ENDPOINT_RADIUS {
            2.5
        } else {
            1.5
        };
        painter.line_segment([center, ep], egui::Stroke::new(width, end.color));
    }

    for end in ends {
        let ep = egui::pos2(
            center.x + end.screen_x * AXIS_LEN,
            center.y + end.screen_y * AXIS_LEN,
        );
        painter.circle_filled(ep, end.radius, end.color);
        let font_size = if end.radius >= POS_ENDPOINT_RADIUS {
            11.0
        } else {
            9.0
        };
        painter.text(
            ep,
            egui::Align2::CENTER_CENTER,
            end.label,
            egui::FontId::proportional(font_size),
            egui::Color32::WHITE,
        );
    }

    painter.circle_filled(center, 3.0, egui::Color32::from_white_alpha(100));
}

fn hit_test(ends: &[AxisEnd], center: egui::Pos2, click: egui::Pos2) -> Option<Interaction> {
    ends.iter()
        .rev()
        .find(|end| {
            let ep = egui::pos2(
                center.x + end.screen_x * AXIS_LEN,
                center.y + end.screen_y * AXIS_LEN,
            );
            click.distance(ep) <= end.radius
        })
        .map(|end| Interaction::SnapAxis {
            yaw: end.snap_yaw,
            pitch: end.snap_pitch,
        })
}

/// Draws the orientation gizmo overlay and applies any camera interactions.
pub fn show(ctx: &egui::Context, viewport: egui::Rect, world: &mut World) {
    let cam_rotation = world
        .query_filtered::<&bevy_panorbit_camera::PanOrbitCamera, With<EditorCamera>>()
        .iter(world)
        .next()
        .map(|cam| {
            let yaw = cam.yaw.unwrap_or(cam.target_yaw);
            let pitch = cam.pitch.unwrap_or(cam.target_pitch);
            Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch)
        });

    let Some(rotation) = cam_rotation else {
        return;
    };

    let ends = project_axes(rotation.inverse());

    let total = (GIZMO_RADIUS + GIZMO_PAD) * 2.0;
    let area_x = viewport.right() - total - GIZMO_PAD;
    let area_y = viewport.top() + GIZMO_PAD;

    let area_resp = egui::Area::new(egui::Id::new("orientation_gizmo"))
        .fixed_pos(egui::pos2(area_x, area_y))
        .show(ctx, |ui| {
            let (resp, painter) =
                ui.allocate_painter(egui::vec2(total, total), egui::Sense::click_and_drag());
            let center = resp.rect.center();

            paint(&painter, center, &ends);

            if resp.dragged() {
                Some(Interaction::Drag(resp.drag_delta()))
            } else if resp.clicked() {
                resp.interact_pointer_pos()
                    .and_then(|pos| hit_test(&ends, center, pos))
            } else {
                None
            }
        });

    match area_resp.inner {
        Some(Interaction::Drag(delta)) => {
            let mut q = world
                .query_filtered::<&mut bevy_panorbit_camera::PanOrbitCamera, With<EditorCamera>>();
            for mut cam in q.iter_mut(world) {
                cam.target_yaw -= delta.x * DRAG_SENSITIVITY;
                cam.target_pitch += delta.y * DRAG_SENSITIVITY;
            }
        }
        Some(Interaction::SnapAxis { yaw, pitch }) => {
            let mut q = world
                .query_filtered::<&mut bevy_panorbit_camera::PanOrbitCamera, With<EditorCamera>>();
            for mut cam in q.iter_mut(world) {
                cam.target_yaw = yaw;
                cam.target_pitch = pitch;
            }
        }
        None => {}
    }
}

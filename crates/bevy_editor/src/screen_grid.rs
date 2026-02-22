use bevy::prelude::*;
use bevy_egui::egui;

use crate::EditorCamera;

const ALIGN_DOT_THRESHOLD: f32 = 0.9999;
const TARGET_MAJOR_PIXELS: f32 = 72.0;
const MIN_MINOR_PIXELS: f32 = 9.0;
const GRID_PAD_UNITS: f32 = 2.0;
const MAX_GRID_LINES: i64 = 600;

#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
    Z,
}

#[derive(Clone, Copy)]
struct GridPlane {
    normal: Vec3,
    u: Vec3,
    v: Vec3,
    u_axis: Axis,
    v_axis: Axis,
}

fn axis_color(axis: Axis) -> egui::Color32 {
    match axis {
        Axis::X => egui::Color32::from_rgb(230, 70, 70),
        Axis::Y => egui::Color32::from_rgb(100, 210, 70),
        Axis::Z => egui::Color32::from_rgb(70, 120, 255),
    }
}

fn classify_axis_aligned(forward: Vec3) -> Option<GridPlane> {
    let f = forward.normalize_or_zero();
    if f == Vec3::ZERO {
        return None;
    }

    let abs = f.abs();

    if abs.x > ALIGN_DOT_THRESHOLD {
        return Some(GridPlane {
            normal: if f.x >= 0.0 { Vec3::X } else { -Vec3::X },
            u: Vec3::Z,
            v: Vec3::Y,
            u_axis: Axis::Z,
            v_axis: Axis::Y,
        });
    }
    if abs.y > ALIGN_DOT_THRESHOLD {
        return Some(GridPlane {
            normal: if f.y >= 0.0 { Vec3::Y } else { -Vec3::Y },
            u: Vec3::X,
            v: Vec3::Z,
            u_axis: Axis::X,
            v_axis: Axis::Z,
        });
    }
    if abs.z > ALIGN_DOT_THRESHOLD {
        return Some(GridPlane {
            normal: if f.z >= 0.0 { Vec3::Z } else { -Vec3::Z },
            u: Vec3::X,
            v: Vec3::Y,
            u_axis: Axis::X,
            v_axis: Axis::Y,
        });
    }

    None
}

fn nice_step(raw_step: f32) -> f32 {
    if !raw_step.is_finite() || raw_step <= 0.0 {
        return 1.0;
    }

    let exponent = raw_step.log10().floor();
    let base = 10.0f32.powf(exponent);
    let fraction = raw_step / base;

    let snapped = if fraction <= 1.0 {
        1.0
    } else if fraction <= 2.0 {
        2.0
    } else if fraction <= 5.0 {
        5.0
    } else {
        10.0
    };

    snapped * base
}

fn grid_to_screen(center: Vec2, u_basis: Vec2, v_basis: Vec2, u: f32, v: f32) -> egui::Pos2 {
    egui::pos2(
        center.x + u_basis.x * u + v_basis.x * v,
        center.y + u_basis.y * u + v_basis.y * v,
    )
}

fn line_count(start: i64, end: i64) -> i64 {
    end.saturating_sub(start) + 1
}

fn draw_grid_family(
    painter: &egui::Painter,
    center: Vec2,
    u_basis: Vec2,
    v_basis: Vec2,
    fixed_start: i64,
    fixed_end: i64,
    fixed_step: f32,
    varying_min: f32,
    varying_max: f32,
    major_every: i64,
    minor_stroke: egui::Stroke,
    major_stroke: egui::Stroke,
    axis_stroke: egui::Stroke,
) {
    let count = line_count(fixed_start, fixed_end);
    if count <= 0 || count > MAX_GRID_LINES {
        return;
    }

    for idx in fixed_start..=fixed_end {
        let fixed = idx as f32 * fixed_step;
        let p1 = grid_to_screen(center, u_basis, v_basis, fixed, varying_min);
        let p2 = grid_to_screen(center, u_basis, v_basis, fixed, varying_max);

        let stroke = if idx == 0 {
            axis_stroke
        } else if idx % major_every == 0 {
            major_stroke
        } else {
            minor_stroke
        };

        painter.line_segment([p1, p2], stroke);
    }
}

/// Draws a 2D overlay grid only when camera is axis-aligned.
pub fn show(ctx: &egui::Context, viewport: egui::Rect, world: &mut World) {
    if viewport.width() <= 1.0 || viewport.height() <= 1.0 {
        return;
    }

    let mut query = world.query_filtered::<(
        &Camera,
        &GlobalTransform,
        &bevy_panorbit_camera::PanOrbitCamera,
    ), With<EditorCamera>>();
    let Some((camera, camera_transform, pan_orbit)) = query.iter(world).next() else {
        return;
    };

    let forward = *camera_transform.forward();
    let Some(grid_plane) = classify_axis_aligned(forward) else {
        return;
    };

    let depth = pan_orbit.target_focus.dot(grid_plane.normal);
    let plane_origin = grid_plane.normal * depth;

    let Ok(center) = camera.world_to_viewport(camera_transform, plane_origin) else {
        return;
    };
    let Ok(u_one) = camera.world_to_viewport(camera_transform, plane_origin + grid_plane.u) else {
        return;
    };
    let Ok(v_one) = camera.world_to_viewport(camera_transform, plane_origin + grid_plane.v) else {
        return;
    };

    let u_basis = u_one - center;
    let v_basis = v_one - center;
    let det = u_basis.x * v_basis.y - u_basis.y * v_basis.x;
    if det.abs() < 1e-5 {
        return;
    }

    let pixels_per_world = (u_basis.length() + v_basis.length()) * 0.5;
    if pixels_per_world <= 1e-4 {
        return;
    }

    let major_step = nice_step(TARGET_MAJOR_PIXELS / pixels_per_world);
    let minor_step = major_step / 10.0;
    let draw_minor = minor_step * pixels_per_world >= MIN_MINOR_PIXELS;

    let mut step = if draw_minor { minor_step } else { major_step };
    let mut major_every = (major_step / step).round().max(1.0) as i64;

    let corners = [
        viewport.left_top(),
        viewport.right_top(),
        viewport.left_bottom(),
        viewport.right_bottom(),
    ];

    let mut min_u = f32::INFINITY;
    let mut max_u = f32::NEG_INFINITY;
    let mut min_v = f32::INFINITY;
    let mut max_v = f32::NEG_INFINITY;

    for corner in corners {
        let d = Vec2::new(corner.x - center.x, corner.y - center.y);
        let u = (d.x * v_basis.y - d.y * v_basis.x) / det;
        let v = (u_basis.x * d.y - u_basis.y * d.x) / det;
        min_u = min_u.min(u);
        max_u = max_u.max(u);
        min_v = min_v.min(v);
        max_v = max_v.max(v);
    }

    min_u -= GRID_PAD_UNITS * step;
    max_u += GRID_PAD_UNITS * step;
    min_v -= GRID_PAD_UNITS * step;
    max_v += GRID_PAD_UNITS * step;

    let mut u_start = (min_u / step).floor() as i64;
    let mut u_end = (max_u / step).ceil() as i64;
    let mut v_start = (min_v / step).floor() as i64;
    let mut v_end = (max_v / step).ceil() as i64;

    while line_count(u_start, u_end) > MAX_GRID_LINES || line_count(v_start, v_end) > MAX_GRID_LINES
    {
        step *= 2.0;
        major_every = (major_step / step).round().max(1.0) as i64;
        u_start = (min_u / step).floor() as i64;
        u_end = (max_u / step).ceil() as i64;
        v_start = (min_v / step).floor() as i64;
        v_end = (max_v / step).ceil() as i64;
    }

    let minor_stroke = egui::Stroke::new(1.0, egui::Color32::from_white_alpha(24));
    let major_stroke = egui::Stroke::new(1.0, egui::Color32::from_white_alpha(56));
    let u_axis_stroke = egui::Stroke::new(1.5, axis_color(grid_plane.u_axis));
    let v_axis_stroke = egui::Stroke::new(1.5, axis_color(grid_plane.v_axis));

    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("editor_screen_grid"));
    let painter = ctx.layer_painter(layer).with_clip_rect(viewport);

    // u = constant => lines run along v
    draw_grid_family(
        &painter,
        center,
        u_basis,
        v_basis,
        u_start,
        u_end,
        step,
        min_v,
        max_v,
        major_every,
        minor_stroke,
        major_stroke,
        v_axis_stroke,
    );

    // v = constant => lines run along u (swap basis/coords)
    draw_grid_family(
        &painter,
        center,
        v_basis,
        u_basis,
        v_start,
        v_end,
        step,
        min_u,
        max_u,
        major_every,
        minor_stroke,
        major_stroke,
        u_axis_stroke,
    );
}

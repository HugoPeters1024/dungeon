use bevy::camera::{OrthographicProjection, PerspectiveProjection, Projection, ScalingMode};
use bevy::prelude::*;

use crate::plugin::EditorCamera;

const ORTHO_ALIGN_DOT_THRESHOLD: f32 = 0.9999;

#[derive(Resource, Default)]
pub(crate) struct AxisAlignedProjectionState {
    saved_projection: Option<Projection>,
    active_camera: Option<Entity>,
}

fn matched_orthographic_from_perspective(
    perspective: &PerspectiveProjection,
) -> OrthographicProjection {
    // PanOrbitCamera uses orthographic scale as zoom and perspective radius as zoom.
    // Matching visible height at the focus plane for all radii requires:
    // ortho_visible_height = scale * viewport_height = radius * (2 * tan(fov/2))
    let viewport_height = (2.0 * (perspective.fov * 0.5).tan()).max(1e-4);
    let mut orthographic = OrthographicProjection::default_3d();
    orthographic.scaling_mode = ScalingMode::FixedVertical { viewport_height };
    orthographic.near = perspective.near;
    orthographic.far = perspective.far;
    orthographic
}

fn is_axis_aligned_forward(forward: Vec3) -> bool {
    let f = forward.normalize_or_zero();
    if f == Vec3::ZERO {
        return false;
    }
    let abs = f.abs();
    abs.x > ORTHO_ALIGN_DOT_THRESHOLD
        || abs.y > ORTHO_ALIGN_DOT_THRESHOLD
        || abs.z > ORTHO_ALIGN_DOT_THRESHOLD
}

pub(crate) fn sync_axis_aligned_projection(
    mut state: ResMut<AxisAlignedProjectionState>,
    mut query: Query<(Entity, &mut Projection, &GlobalTransform), With<EditorCamera>>,
) {
    let Ok((camera_entity, mut projection, camera_transform)) = query.single_mut() else {
        state.saved_projection = None;
        state.active_camera = None;
        return;
    };

    let forward = *camera_transform.forward();
    let axis_aligned = is_axis_aligned_forward(forward);

    if !axis_aligned {
        if state.active_camera == Some(camera_entity) {
            if let Some(saved_projection) = state.saved_projection.take() {
                *projection = saved_projection;
                info!("Editor camera: restored perspective projection (left axis-aligned mode)");
            }
            state.active_camera = None;
        }
        return;
    }

    if state.active_camera != Some(camera_entity) {
        state.saved_projection = Some(projection.clone());
        state.active_camera = Some(camera_entity);
        let orthographic = match state.saved_projection.as_ref().unwrap_or(&*projection) {
            Projection::Perspective(perspective) => {
                matched_orthographic_from_perspective(perspective)
            }
            Projection::Orthographic(orthographic) => orthographic.clone(),
            Projection::Custom(_) => {
                let mut fallback = PerspectiveProjection::default();
                fallback.fov = std::f32::consts::FRAC_PI_4;
                fallback.aspect_ratio = 1.0;
                fallback.near = 0.01;
                fallback.far = 10_000.0;
                matched_orthographic_from_perspective(&fallback)
            }
        };
        *projection = Projection::Orthographic(orthographic);
        info!("Editor camera: switching to orthographic projection (axis-aligned mode)");
    } else if state.saved_projection.is_none() {
        state.saved_projection = Some(projection.clone());
    } else if !matches!(*projection, Projection::Orthographic(_)) {
        // If another system changed projection while still axis-aligned, reapply once.
        if let Some(Projection::Perspective(perspective)) = state.saved_projection.as_ref() {
            *projection =
                Projection::Orthographic(matched_orthographic_from_perspective(perspective));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::CameraProjection;

    fn ndc_xy(clip_from_view: Mat4, view_point: Vec3) -> Vec2 {
        let clip = clip_from_view * view_point.extend(1.0);
        Vec2::new(clip.x / clip.w, clip.y / clip.w)
    }

    #[test]
    fn matched_ortho_keeps_plane_screen_positions_close() {
        let width = 1920.0;
        let height = 1080.0;
        let radii = [3.0, 7.5, 12.0];

        let mut perspective = PerspectiveProjection::default();
        perspective.fov = 55.0_f32.to_radians();
        perspective.aspect_ratio = width / height;
        perspective.near = 0.05;
        perspective.far = 5000.0;
        perspective.update(width, height);

        for radius in radii {
            let mut orthographic = matched_orthographic_from_perspective(&perspective);
            // Simulate PanOrbitCamera orthographic behavior: it writes scale from orbit radius.
            orthographic.scale = radius;
            orthographic.update(width, height);

            let persp_clip = perspective.get_clip_from_view();
            let ortho_clip = orthographic.get_clip_from_view();

            let half_height = radius * (perspective.fov * 0.5).tan();
            let half_width = half_height * perspective.aspect_ratio;
            let samples = [
                Vec3::new(0.0, 0.0, -radius),
                Vec3::new(0.5 * half_width, 0.0, -radius),
                Vec3::new(-0.5 * half_width, 0.3 * half_height, -radius),
                Vec3::new(0.2 * half_width, -0.6 * half_height, -radius),
            ];

            for point in samples {
                let p = ndc_xy(persp_clip, point);
                let o = ndc_xy(ortho_clip, point);
                assert!(
                    (p - o).length() < 1e-3,
                    "NDC mismatch at radius {radius} for point {point:?}: perspective={p:?}, ortho={o:?}"
                );
            }
        }
    }
}

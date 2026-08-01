//! Renders a small 3D preview of each registered prefab and exposes it to the
//! egui-based prefab browser.
//!
//! Each prefab is instanced once under a far-away "staging" parent on a
//! dedicated [`RenderLayers`] so it is invisible to the game/editor cameras and
//! physically isolated from the real scene. A throwaway camera renders that
//! instance into an offscreen [`Image`], which is registered with egui as a
//! texture. Jobs run one at a time through a queue.

use bevy::{
    camera::{ClearColorConfig, RenderTarget, visibility::RenderLayers},
    platform::collections::{HashMap, HashSet},
    prelude::*,
    render::render_resource::TextureFormat,
};
use bevy_egui::{EguiTextureHandle, EguiUserTextures, egui};
use std::collections::VecDeque;

use crate::{merged_aabb::MergedAabb, prefabs::Prefabs};

/// Pixel size of the (square) thumbnail textures.
const THUMBNAIL_SIZE: u32 = 128;
/// Render layer used to isolate the previewed prefab from every other camera.
const THUMBNAIL_LAYER: usize = 16;
/// Where prefab instances are staged while being photographed — far enough away
/// that their colliders never interact with the real scene.
const STAGING_ORIGIN: Vec3 = Vec3::new(0.0, -100_000.0, 0.0);
/// Frames to keep rendering after framing the prefab, so the pipeline has time
/// to draw at least one complete frame into the target texture.
const CAPTURE_FRAMES: u32 = 3;
/// Give a prefab this many frames to stream its meshes before we frame it with
/// whatever bounds it has (avoids hanging on prefabs that never report an AABB).
const BOUNDS_TIMEOUT_FRAMES: u32 = 180;

/// Maps prefab name -> egui texture id once its thumbnail has been rendered.
#[derive(Resource, Default)]
pub struct PrefabThumbnails {
    ready: HashMap<String, egui::TextureId>,
    queue: VecDeque<String>,
    enqueued: HashSet<String>,
}

impl PrefabThumbnails {
    pub fn texture_id(&self, prefab_name: &str) -> Option<egui::TextureId> {
        self.ready.get(prefab_name).copied()
    }
}

pub(crate) enum Stage {
    AwaitingBounds { ttl: u32 },
    Capturing { frames_left: u32 },
}

/// Component placed on the throwaway camera that drives a single thumbnail job.
#[derive(Component)]
pub(crate) struct ThumbnailJob {
    prefab: String,
    image: Handle<Image>,
    staging: Entity,
    instance: Entity,
    light: Entity,
    stage: Stage,
}

/// Drives thumbnail generation: enqueues new prefabs, advances the active job,
/// and starts the next queued job when idle. Runs one prefab at a time.
pub(crate) fn manage_prefab_thumbnails(
    mut commands: Commands,
    prefabs: Res<Prefabs>,
    mut thumbnails: ResMut<PrefabThumbnails>,
    mut images: ResMut<Assets<Image>>,
    mut egui_textures: ResMut<EguiUserTextures>,
    mut jobs: Query<(Entity, &mut ThumbnailJob, &mut Camera, &mut Transform)>,
    merged_aabbs: Query<&MergedAabb>,
    children: Query<&Children>,
) {
    enqueue_missing(&prefabs, &mut thumbnails);

    if let Ok((camera_entity, mut job, mut camera, mut camera_transform)) = jobs.single_mut() {
        let instance = job.instance;
        // Re-apply the render layer every frame: glTF prefabs stream their mesh
        // children in over several frames, and each needs to be isolated too.
        apply_layer_recursive(&mut commands, instance, &children);

        match &mut job.stage {
            Stage::AwaitingBounds { ttl } => {
                let bounds = prefab_world_bounds(&merged_aabbs, instance)
                    .or_else(|| (*ttl == 0).then_some((Vec3::ZERO, Vec3::splat(0.5))));

                if let Some((center, half_extents)) = bounds {
                    frame_prefab(&mut camera_transform, center, half_extents);
                    camera.is_active = true;
                    job.stage = Stage::Capturing {
                        frames_left: CAPTURE_FRAMES,
                    };
                } else {
                    *ttl -= 1;
                }
            }
            Stage::Capturing { frames_left } if *frames_left > 0 => {
                *frames_left -= 1;
            }
            Stage::Capturing { .. } => {
                let texture_id =
                    egui_textures.add_image(EguiTextureHandle::Strong(job.image.clone()));
                thumbnails.ready.insert(job.prefab.clone(), texture_id);

                // Despawning the staging parent takes the whole prefab subtree.
                commands.entity(job.staging).despawn();
                commands.entity(job.light).despawn();
                commands.entity(camera_entity).despawn();
            }
        }
        return;
    }

    if let Some(prefab) = thumbnails.queue.pop_front() {
        start_job(&mut commands, &mut images, prefab);
    }
}

fn enqueue_missing(prefabs: &Prefabs, thumbnails: &mut PrefabThumbnails) {
    for id in prefabs.get_prefab_ids() {
        let name = id.name();
        if !thumbnails.enqueued.contains(name) {
            thumbnails.enqueued.insert(name.to_string());
            thumbnails.queue.push_back(name.to_string());
        }
    }
}

fn start_job(commands: &mut Commands, images: &mut Assets<Image>, prefab: String) {
    let image = images.add(Image::new_target_texture(
        THUMBNAIL_SIZE,
        THUMBNAIL_SIZE,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let layer = RenderLayers::layer(THUMBNAIL_LAYER);

    let staging = commands
        .spawn((
            Name::new(format!("Thumbnail staging: {prefab}")),
            Transform::from_translation(STAGING_ORIGIN),
            GlobalTransform::default(),
            Visibility::Visible,
            layer.clone(),
        ))
        .id();

    // Spawning the `PrefabId` triggers the prefab factory on this entity.
    let instance = commands
        .spawn((
            crate::prefabs::PrefabId::new(prefab.clone()),
            ChildOf(staging),
        ))
        .id();

    let light = commands
        .spawn((
            DirectionalLight {
                illuminance: 6_000.0,
                ..default()
            },
            Transform::from_translation(STAGING_ORIGIN)
                .looking_to(Vec3::new(-1.0, -1.2, -0.8), Vec3::Y),
            layer.clone(),
        ))
        .id();

    commands.spawn((
        Name::new(format!("Thumbnail camera: {prefab}")),
        Camera3d::default(),
        Camera {
            // Render behind the main window cameras; it targets an image anyway.
            order: -10,
            is_active: false,
            clear_color: ClearColorConfig::Custom(Color::srgba(0.13, 0.13, 0.15, 1.0)),
            ..default()
        },
        RenderTarget::from(image.clone()),
        // A touch of ambient so faces turned away from the key light aren't black.
        AmbientLight {
            brightness: 600.0,
            ..default()
        },
        Transform::default(),
        layer,
        ThumbnailJob {
            prefab,
            image,
            staging,
            instance,
            light,
            stage: Stage::AwaitingBounds {
                ttl: BOUNDS_TIMEOUT_FRAMES,
            },
        },
    ));
}

fn apply_layer_recursive(commands: &mut Commands, root: Entity, children: &Query<&Children>) {
    let layer = RenderLayers::layer(THUMBNAIL_LAYER);
    commands.entity(root).insert(layer.clone());
    for descendant in children.iter_descendants(root) {
        commands.entity(descendant).insert(layer.clone());
    }
}

/// World-space `(center, half_extents)` of the instance, or `None` until it has
/// reported a non-degenerate bounding box.
fn prefab_world_bounds(
    merged_aabbs: &Query<&MergedAabb>,
    instance: Entity,
) -> Option<(Vec3, Vec3)> {
    let merged = merged_aabbs.get(instance).ok()?;
    let half_extents: Vec3 = merged.half_extents.into();
    (half_extents.length() > 1e-3).then(|| (Vec3::from(merged.center), half_extents))
}

/// Position the camera at a 45° azimuth, looking slightly down at the prefab,
/// pulling back far enough to fit the bounding sphere in view.
fn frame_prefab(camera_transform: &mut Transform, center: Vec3, half_extents: Vec3) {
    const FOV: f32 = std::f32::consts::FRAC_PI_4;

    let radius = half_extents.length().max(0.05);
    let distance = radius / (FOV * 0.5).sin() * 1.25;

    // 45° around Y, raised so we look down at roughly 30°.
    let direction = Vec3::new(1.0, 0.7, 1.0).normalize();
    *camera_transform =
        Transform::from_translation(center + direction * distance).looking_at(center, Vec3::Y);
}

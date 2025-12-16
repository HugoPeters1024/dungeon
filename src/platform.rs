use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;

pub struct PlatformPlugin;

impl Plugin for PlatformPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, progress_path);
    }
}

#[derive(Component)]
#[require(Transform, PathIndex, LinearVelocity)]
pub struct PlatformPath {
    pub path: Vec<Vec3>,
    pub speed: f32,
}

#[derive(Component, Default)]
struct PathIndex(usize);

fn progress_path(
    mut q: Query<(
        &PlatformPath,
        &mut Transform,
        &mut LinearVelocity,
        &mut PathIndex,
    )>,
) {
    for (path, t, mut linvel, mut idx) in q.iter_mut() {
        if idx.0 >= path.path.len() {
            idx.0 %= path.path.len();
        }

        let next_target = &path.path[idx.0];
        let current = t.translation;

        let towards = next_target - current;
        if towards.length() < 0.01 {
            idx.0 += 1;
        }
        linvel.0 = towards.normalize_or_zero() * Vec3::splat(path.speed);
        linvel.0 = linvel.0.min(towards + Vec3::splat(1.0));
    }
}

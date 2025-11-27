use avian3d::prelude::*;
use bevy::{math::Affine2, prelude::*};
use bevy_hanabi::prelude::*;

use crate::assets::GameAssets;

#[derive(Component)]
#[require(Transform, InheritedVisibility)]
pub struct SpawnTorch;

pub struct SpawnPlugin;

#[derive(Component)]
pub struct Torch {
    flikker_offset: f32,
}

impl Plugin for SpawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_spawn_torch);
        app.add_systems(Update, torch_flikkers);
    }
}

fn on_spawn_torch(
    on: On<Add, SpawnTorch>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let root = on.event_target();

    // cube with stone texture
    let cube = commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::new(1.0, 3.0, 1.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color_texture: Some(assets.stones.clone()),
                uv_transform: Affine2::from_scale(Vec2::new(1.0, 3.0)),
                perceptual_roughness: 0.9,
                ..default()
            })),
            ChildOf(root),
            RigidBody::Static,
            Collider::cuboid(1.0, 3.0, 1.0),
        ))
        .id();

    // torch model
    let torch = commands
        .spawn((
            ChildOf(cube),
            SceneRoot(assets.torch.clone()),
            Transform::from_xyz(0.0, 0.0, 0.55)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        ))
        .id();

    // Create and spawn fire particle effect
    let fire_effect = commands
        .spawn((
            ParticleEffect::new(assets.fire.clone()),
            Transform::from_xyz(0.0, 0.5, -0.2),
            ChildOf(torch),
        ))
        .id();

    // light
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            intensity: light_consts::lumens::LUMENS_PER_LED_WATTS * 150.0,
            color: Color::srgb(1.0, 0.6, 0.2),
            ..default()
        },
        Torch {
            flikker_offset: rand::random::<f32>() * 100.0,
        },
        Transform::from_xyz(0.0, 0.0, -0.5),
        ChildOf(fire_effect),
    ));
}

fn torch_flikkers(mut q: Query<(&mut PointLight, &Torch)>, time: Res<Time>) {
    for (mut p, t) in q.iter_mut() {
        let t = time.elapsed_secs() * 4.0 + t.flikker_offset;
        let noise = (t * 2.0).sin() * (t * 3.7).cos();
        p.intensity = light_consts::lumens::LUMENS_PER_LED_WATTS * (450.0 + 40.0 * noise)
    }
}

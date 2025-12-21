use std::f32::consts::PI;

use avian3d::prelude::*;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::post_process::bloom::Bloom;
use bevy::post_process::motion_blur::MotionBlur;
use bevy::{math::Affine2, prelude::*};
use bevy_hanabi::prelude::*;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_kira_audio::AudioPlugin;
use bevy_tnua::{TnuaNotPlatform, prelude::*};
use bevy_tnua_avian3d::prelude::*;

use crate::assets::*;
use crate::camera::ThirdPersonCameraPlugin;
use crate::chunks::ChunkObserver;
use crate::hud::HudPlugin;
use crate::platform::PlatformPath;
use crate::player::controller::PlayerRoot;
use crate::spawners::*;
use crate::talents::TalentsPlugin;

pub struct GamePlugin;

#[derive(Component)]
pub struct Pickupable;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(avian3d::prelude::PhysicsPlugins::default());
        app.insert_resource(avian3d::prelude::Gravity(Vec3::NEG_Y * 9.0));
        //app.add_plugins(avian3d::prelude::PhysicsDebugPlugin::default());
        app.add_plugins(TnuaControllerPlugin::new(FixedUpdate));
        app.add_plugins(TnuaAvian3dPlugin::new(FixedUpdate));
        app.add_plugins(EguiPlugin::default());
        app.add_plugins(AudioPlugin);

        #[cfg(not(target_arch = "wasm32"))]
        app.add_plugins(WorldInspectorPlugin::new());

        app.add_plugins(HanabiPlugin);
        app.add_plugins(crate::assets::AssetPlugin);
        app.add_plugins(crate::spawners::SpawnPlugin);
        app.add_plugins(TalentsPlugin);
        app.add_plugins(HudPlugin);
        app.add_plugins(crate::player::PlayerPlugin);
        app.add_plugins(crate::platform::PlatformPlugin);
        app.add_plugins(crate::chunks::ChunksPlugin);
        app.add_plugins(ThirdPersonCameraPlugin);
        app.insert_resource(ClearColor(Color::srgb(0.08, 0.02, 0.02))); // Very dark black background
        app.add_systems(OnEnter(MyStates::Next), setup);
    }
}

/// set up a simple 3D scene
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ambient_light: ResMut<AmbientLight>,
    assets: Res<GameAssets>,
) {
    ambient_light.brightness = 100.0;

    commands.spawn((
        DirectionalLight {
            illuminance: light_consts::lux::OVERCAST_DAY,
            shadows_enabled: true,
            ..default()
        },
        Transform {
            translation: Vec3::new(0.0, 2.0, 0.0),
            rotation: Quat::from_rotation_x(-PI / 4.),
            ..default()
        },
        // The default cascade config is designed to handle large scenes.
        // As this example has a much smaller world, we can tighten the shadow
        // bounds for better visual quality.
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 4.0,
            maximum_distance: 100.0,
            ..default()
        }
        .build(),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 0.5, 2.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(assets.lava.clone()),
            uv_transform: Affine2::from_scale(Vec2::new(3.0, 3.0)),
            perceptual_roughness: 1.0,
            emissive: LinearRgba {
                red: 8.0,
                green: 4.0,
                blue: 2.5,
                alpha: 1.0,
            },
            ..default()
        })),
        RigidBody::Kinematic,
        Collider::cuboid(2.0, 0.5, 2.0),
        Name::new("Platform"),
        Transform::from_xyz(0.0, 1.0, 10.0),
        PlatformPath {
            path: vec![
                Vec3::new(0.0, 1.0, 1.0),
                Vec3::new(0.0, 1.0, 10.0),
                Vec3::new(0.0, 10.0, 5.0),
            ],
            speed: 2.0,
        },
    ));

    for i in 0..10 {
        commands.spawn((
            Mesh3d(assets.stairs.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color_texture: Some(assets.mossy_stones.clone()),
                perceptual_roughness: 1.0,
                ..default()
            })),
            Transform::from_xyz(0.0 - i as f32, 0.25 + 0.5 * i as f32, 2.0)
                .with_scale(Vec3::new(0.5, 0.25, 0.5)),
            Name::new("Stairs"),
            RigidBody::Static,
            //ColliderConstructor::TrimeshFromMesh,
            ColliderConstructor::ConvexHullFromMesh,
        ));
    }

    const WIDTH: usize = 5;
    for y in 0..=WIDTH {
        for x in 0..=y {
            commands.spawn((
                Pickupable,
                Mesh3d(assets.wineglass.clone()),
                MeshMaterial3d(assets.wineglass_material.clone()),
                Transform::from_xyz(
                    0.3 * (x as f32 - y as f32 / 2.0),
                    0.32 + 0.44 * (WIDTH as f32 - y as f32),
                    -2.0,
                )
                .with_scale(Vec3::splat(0.1)),
                Name::new("Wineglass"),
                Mass(0.2),
                CenterOfMass(Vec3::new(0.0, 0.25, 0.0)),
                RigidBody::Dynamic,
                TnuaNotPlatform,
                ColliderConstructor::Cuboid {
                    x_length: 2.5,
                    y_length: 4.0,
                    z_length: 2.5,
                },
            ));
        }
    }
    for x in 0..40 {
        commands.spawn((
            Pickupable,
            Mesh3d(assets.wineglass.clone()),
            MeshMaterial3d(assets.wineglass_material.clone()),
            Transform::from_xyz(0.3 * x as f32, 1.32, -2.0).with_scale(Vec3::splat(0.1)),
            Name::new("Wineglass"),
            Mass(0.2),
            RigidBody::Dynamic,
            TnuaNotPlatform,
            ColliderConstructor::Cuboid {
                x_length: 2.5,
                y_length: 4.0,
                z_length: 2.5,
            },
        ));
    }

    commands.spawn((
        Mesh3d(assets.trophy.clone()),
        MeshMaterial3d(assets.trophy_material.clone()),
        Transform::from_xyz(0.0, 4.0, 4.0).with_scale(Vec3::splat(0.1)),
        Name::new("Trophy"),
        Pickupable,
        Mass(0.5),
        RigidBody::Dynamic,
        TnuaNotPlatform,
        ColliderConstructor::Cuboid {
            x_length: 2.5,
            y_length: 4.0,
            z_length: 2.5,
        },
    ));

    commands.spawn((
        Mesh3d(assets.bong.clone()),
        MeshMaterial3d(assets.bong_material.clone()),
        Transform::from_xyz(2.0, 4.0, 4.0).with_scale(Vec3::splat(0.3)),
        Name::new("Bong"),
        Pickupable,
        Mass(0.5),
        RigidBody::Dynamic,
        TnuaNotPlatform,
        ColliderConstructor::Cuboid {
            x_length: 2.5,
            y_length: 4.0,
            z_length: 2.5,
        },
    ));

    // Player-following camera
    let mut camera_entity = commands.spawn((
        Camera3d::default(),
        crate::camera::ThirdPersonCamera::default(),
        Transform::from_xyz(0.0, 3.0, 5.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
        Bloom::NATURAL,
    ));

    camera_entity.insert(MotionBlur {
        shutter_angle: 1.25,
        samples: 2,
    });

    commands.spawn((PlayerRoot, Name::new("Player"), ChunkObserver));

    commands.spawn((SpawnTorch, Transform::from_xyz(-2.0, 1.0, 0.0)));

    commands.spawn((SpawnTorch, Transform::from_xyz(2.0, 1.0, 0.0)));

    commands.spawn((ParticleEffect::new(assets.void.clone()),));
}

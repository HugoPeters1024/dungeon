use std::f32::consts::PI;

use avian3d::prelude::*;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::post_process::bloom::Bloom;
use bevy::post_process::motion_blur::MotionBlur;
use bevy::window::CursorOptions;
use bevy::{math::Affine2, prelude::*};
use bevy_hanabi::prelude::*;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_tnua::prelude::*;
use bevy_tnua_avian3d::prelude::*;

use crate::assets::*;
use crate::player::controller::PlayerRoot;
use crate::spawners::*;

pub struct GamePlugin;

#[derive(Component)]
pub struct PlayerCamera {
    pub pitch: f32,
    pub yaw: f32,
    pub distance: f32,
    pub height: f32,
}

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(avian3d::prelude::PhysicsPlugins::default());
        app.insert_resource(avian3d::prelude::Gravity(Vec3::NEG_Y * 9.0));
        //app.add_plugins(avian3d::prelude::PhysicsDebugPlugin::default());
        app.add_plugins(TnuaControllerPlugin::new(FixedUpdate));
        app.add_plugins(TnuaAvian3dPlugin::new(FixedUpdate));
        app.add_plugins(EguiPlugin::default());

        #[cfg(not(target_arch = "wasm32"))]
        app.add_plugins(WorldInspectorPlugin::new());

        app.add_plugins(HanabiPlugin);
        app.add_plugins(crate::assets::AssetPlugin);
        app.add_plugins(crate::spawners::SpawnPlugin);
        app.add_plugins(crate::player::PlayerPlugin);
        app.insert_resource(ClearColor(Color::srgb(0.0, 0.0, 0.0))); // Very dark black background
        app.add_systems(OnEnter(MyStates::Next), setup);
        app.add_systems(
            Update,
            (handle_mouse_look, update_camera_position).run_if(in_state(MyStates::Next)),
        );
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

    // base
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(12.0, 0.1, 12.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(assets.mossy_stones.clone()),
            uv_transform: Affine2::from_scale(Vec2::new(3.0, 3.0)),
            perceptual_roughness: 1.0,
            ..default()
        })),
        RigidBody::Static,
        Collider::cuboid(12.0, 0.1, 12.0),
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
            ColliderConstructor::TrimeshFromMesh,
        ));
    }

    // Player-following camera
    let mut camera_entity = commands.spawn((
        Camera3d::default(),
        PlayerCamera {
            pitch: -0.5, // Look slightly down
            yaw: 0.0,
            distance: 5.0,
            height: 2.5,
        },
        Transform::from_xyz(0.0, 3.0, 5.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
        Bloom::NATURAL,
    ));

    camera_entity.insert(MotionBlur {
        shutter_angle: 1.25,
        samples: 2,
    });

    commands.spawn((PlayerRoot, Name::new("Player")));

    commands.spawn((SpawnTorch, Transform::from_xyz(-2.0, 1.0, 0.0)));

    commands.spawn((SpawnTorch, Transform::from_xyz(2.0, 1.0, 0.0)));

    commands.spawn((ParticleEffect::new(assets.void.clone()),));
}

fn handle_mouse_look(
    mut cursor_options: Single<&mut CursorOptions>,
    mut camera_query: Query<&mut PlayerCamera>,
    mut cursor_events: MessageReader<bevy::input::mouse::MouseMotion>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
) {
    let Ok(mut camera) = camera_query.single_mut() else {
        return;
    };

    const MOUSE_SENSITIVITY_HORIZONTAL: f32 = 0.003;
    const MOUSE_SENSITIVITY_VERTICAL: f32 = 0.003;

    // Use cursor delta from mouse motion events
    let mut delta = Vec2::ZERO;
    for event in cursor_events.read() {
        delta += event.delta;
    }

    // Lock cursor for better camera control
    if mouse.just_pressed(MouseButton::Middle) {
        cursor_options.grab_mode = bevy::window::CursorGrabMode::Locked;
        cursor_options.visible = false;
    }

    if keyboard.just_pressed(KeyCode::Escape) {
        cursor_options.grab_mode = bevy::window::CursorGrabMode::None;
        cursor_options.visible = true;
    }

    if cursor_options.grab_mode == bevy::window::CursorGrabMode::Locked {
        // Update camera rotation with different sensitivities
        camera.yaw -= delta.x * MOUSE_SENSITIVITY_HORIZONTAL;
        camera.pitch += delta.y * MOUSE_SENSITIVITY_VERTICAL;

        // Clamp pitch to prevent flipping
        camera.pitch = camera.pitch.clamp(
            -std::f32::consts::FRAC_PI_2 + 0.1,
            std::f32::consts::FRAC_PI_2 - 0.1,
        );
    }
}

fn update_camera_position(
    mut camera_query: Query<(&mut Transform, &PlayerCamera)>,
    player_query: Query<
        &Transform,
        (
            With<bevy_tnua::prelude::TnuaController>,
            Without<PlayerCamera>,
        ),
    >,
) {
    let Ok((mut camera_transform, camera)) = camera_query.single_mut() else {
        return;
    };

    let Ok(player_transform) = player_query.single() else {
        return;
    };

    // Calculate camera position behind player based on yaw and pitch
    let player_pos = player_transform.translation;

    // Horizontal distance component (reduced when looking up/down)
    let horizontal_distance = camera.distance * camera.pitch.cos();

    // Camera offset in spherical coordinates
    let camera_offset = Vec3::new(
        camera.yaw.sin() * horizontal_distance,
        camera.height + camera.distance * camera.pitch.sin(), // Adjust height based on pitch
        camera.yaw.cos() * horizontal_distance,
    );

    camera_transform.translation = player_pos + camera_offset;

    // Calculate look direction - always look at player's head height
    let look_target = player_pos + Vec3::Y * 1.0;
    camera_transform.look_at(look_target, Vec3::Y);
}

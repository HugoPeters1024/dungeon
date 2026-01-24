use bevy::prelude::*;
use bevy_editor::{EditorCamera, EditorPlugin, PrefabId, Prefabs};
use bevy_panorbit_camera::PanOrbitCamera;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_plugins(EditorPlugin::new());
    app.add_systems(Startup, setup);
    app.run();
}

fn setup(mut commands: Commands, mut prefabs: ResMut<Prefabs>) {
    // camera
    commands.spawn((
        Camera3d::default(),
        EditorCamera,
        PanOrbitCamera {
            button_orbit: MouseButton::Middle,
            button_pan: MouseButton::Middle,
            modifier_pan: Some(KeyCode::ShiftLeft),
            ..default()
        },
        Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        MeshPickingCamera::default(),
        // PickRaycastSource,
    ));

    // Spawn a directional light
    commands.spawn((
        DirectionalLight {
            illuminance: 3000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, -0.5, 0.0)),
    ));

    prefabs.register_prefab(
        &mut commands,
        "Red Cube",
        |mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>| {
            (
                Name::new("Red Cube"),
                Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 0.0, 0.0),
                    ..default()
                })),
            )
        },
    );
    prefabs.register_prefab(
        &mut commands,
        "Blue Sphere",
        |mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>| {
            (
                Name::new("Blue Sphere"),
                Mesh3d(meshes.add(Sphere::new(0.5))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.0, 0.0, 1.0),
                    ..default()
                })),
            )
        },
    );

    prefabs.register_prefab(&mut commands, "Both", || {
        (
            Transform::default(),
            InheritedVisibility::default(),
            children![
                (
                    PrefabId::new("Red Cube"),
                    Transform::from_translation(Vec3::Y),
                ),
                (PrefabId::new("Blue Sphere"),)
            ],
        )
    });
}

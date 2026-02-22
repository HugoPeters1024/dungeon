use bevy::prelude::*;
use bevy_editor::{
    AssetRef, AssetRefRegistry, EditorCamera, EditorPlugin, PanOrbitCamera, PrefabId, Prefabs,
};

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_plugins(EditorPlugin::new());
    app.add_systems(Startup, setup);
    app.run();
}

fn setup(
    mut commands: Commands,
    mut prefabs: ResMut<Prefabs>,
    mut asset_refs: ResMut<AssetRefRegistry>,
) {
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
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 3000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, -0.5, 0.0)),
    ));

    // Register asset hydration factories — these resolve AssetRefs into actual handles
    asset_refs.register(
        &mut commands,
        "red_cube_assets",
        |mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>| {
            (
                Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 0.0, 0.0),
                    ..default()
                })),
            )
        },
    );

    asset_refs.register(
        &mut commands,
        "blue_sphere_assets",
        |mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>| {
            (
                Mesh3d(meshes.add(Sphere::new(0.5))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.0, 0.0, 1.0),
                    ..default()
                })),
            )
        },
    );

    // Prefabs describe structure + AssetRefs — no direct handle usage
    prefabs.register_prefab(&mut commands, "Red Cube", || {
        (Name::new("Red Cube"), AssetRef::new("red_cube_assets"))
    });

    prefabs.register_prefab(&mut commands, "Blue Sphere", || {
        (
            Name::new("Blue Sphere"),
            AssetRef::new("blue_sphere_assets"),
        )
    });

    prefabs.register_prefab(&mut commands, "Both", || {
        (
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

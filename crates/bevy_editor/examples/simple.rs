use bevy::ecs::{entity::EntitySetIterator, reflect::AppTypeRegistry};
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::scene::DynamicSceneBuilder;
use bevy_editor::{EditorCamera, EditorPlugin, PrefabId, Prefabs};
use bevy_panorbit_camera::PanOrbitCamera;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_plugins(EditorPlugin::new());
    app.add_systems(Startup, setup);
    app.add_systems(Update, (save_scene, load_scene));
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
}

fn do_the_save(world: &mut World) {
    let entities: HashSet<Entity> = world
        .query_filtered::<Entity, With<PrefabId>>()
        .iter(world)
        .collect_set();

    let scene = DynamicSceneBuilder::from_world(world)
        .deny_all_components()
        .allow_component::<PrefabId>()
        .allow_component::<Transform>()
        .allow_component::<Children>()
        .extract_entities(entities.iter().cloned())
        .build();

    let type_registry = world.resource::<AppTypeRegistry>();
    let type_registry = type_registry.read();

    match scene.serialize(&type_registry) {
        Ok(serialized) => {
            std::fs::write("crates/bevy_editor/assets/scene.scn.ron", &serialized)
                .expect("Failed to write scene file");
            info!("Scene saved to scene.scn.ron");
        }
        Err(e) => {
            error!("Failed to serialize scene: {}", e);
        }
    }
}

fn save_scene(mut commands: Commands, keyboard: Res<ButtonInput<KeyCode>>) {
    if keyboard.just_pressed(KeyCode::KeyS) {
        commands.run_system_cached(do_the_save);
    }
}

fn load_scene(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    asset_server: Res<AssetServer>,
) {
    if keyboard.just_pressed(KeyCode::KeyL) {
        commands.spawn(DynamicSceneRoot(asset_server.load("scene.scn.ron")));
        info!("Loading scene from scene.scn.ron");
    }
}

use crate::{
    assets::{GameAssets, MyStates},
    game::Pickupable,
    player::controller::ControllerCamera,
};
use avian3d::prelude::*;
use bevy::{
    gltf::{GltfMesh, GltfNode},
    prelude::*,
};
use bevy_editor::{bevy_panorbit_camera::PanOrbitCamera, EditorCamera, Prefabs};
use bevy_tnua::TnuaNotPlatform;

pub struct EditorPlugin;

/// Marker component for the dedicated editor camera (separate from player camera)
#[derive(Component)]
pub struct DedicatedEditorCamera;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::picking::prelude::MeshPickingPlugin);
        app.add_plugins(bevy_editor::EditorPlugin::new().run_if(in_state(MyStates::Editor)));
        app.add_systems(OnEnter(MyStates::Editor), (setup_prefabs, enable_editor_camera));
        app.add_systems(OnExit(MyStates::Editor), disable_editor_camera);
    }
}

/// Spawns or enables the dedicated editor camera when entering editor mode
fn enable_editor_camera(
    mut commands: Commands,
    mut player_cam: Single<&mut Camera, With<ControllerCamera>>,
    editor_cam: Option<Single<(Entity, &mut Camera), (With<DedicatedEditorCamera>, Without<ControllerCamera>)>>,
) {
    // Disable the player camera
    player_cam.is_active = false;

    if let Some(mut editor_cam) = editor_cam {
        // Re-enable existing editor camera
        editor_cam.1.is_active = true;
    } else {
        // Spawn the dedicated editor camera with PanOrbitCamera for Blender-like controls
        commands.spawn((
            Name::new("Editor Camera"),
            DedicatedEditorCamera,
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
    }
}

/// Disables the editor camera and re-enables the player camera when exiting editor mode
fn disable_editor_camera(
    mut player_cam: Single<&mut Camera, With<ControllerCamera>>,
    editor_cam: Option<Single<&mut Camera, (With<DedicatedEditorCamera>, Without<ControllerCamera>)>>,
) {
    // Re-enable the player camera and reset its viewport
    player_cam.is_active = true;
    player_cam.viewport = None;

    // Disable the editor camera
    if let Some(mut editor_cam) = editor_cam {
        editor_cam.is_active = false;
    }
}

fn setup_prefabs(
    mut commands: Commands,
    mut prefabs: ResMut<Prefabs>,
    assets: Res<GameAssets>,
    gltfs: Res<Assets<Gltf>>,
) {

    prefabs.add(
        "The test",
        commands.register_system(|mut commands: Commands, assets: Res<GameAssets>| {
            commands.spawn((
                Mesh3d(assets.bong.clone()),
                MeshMaterial3d(assets.bong_material.clone()),
                Transform::from_xyz(2.0, 14.0, 4.0).with_scale(Vec3::splat(0.3)),
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
        }),
    );

    fn f(node: Handle<GltfNode>) -> impl Fn(Commands) {
        move |commands: Commands| {
            g(commands, node.clone());
        }
    }

    fn g(mut commands: Commands, node: Handle<GltfNode>) {
        commands.run_system_cached_with(spawn_gltf_node, node);
    }

    let gltf = gltfs.get(assets.castle_test.id()).unwrap();
    for (name, node) in gltf.named_nodes.iter() {
        prefabs.add(name.clone(), commands.register_system(f(node.clone())));
    }
}

fn spawn_gltf_node(
    In(node_handle): In<Handle<GltfNode>>,
    mut commands: Commands,
    nodes: Res<Assets<GltfNode>>,
    meshes: Res<Assets<GltfMesh>>,
) {
    let Some(node) = nodes.get(node_handle.id()) else {
        return;
    };
    let mut builder = commands.spawn((
        Name::new(node.name.clone()),
        InheritedVisibility::default(),
        Transform::default(),
    ));
    if let Some(mesh_handle) = node.mesh.as_ref() {
        if let Some(mesh) = meshes.get(mesh_handle.id()).as_ref() {
            builder.with_children(|parent| {
                for primitive in mesh.primitives.iter() {
                    let mut child = parent.spawn((
                        Mesh3d(primitive.mesh.clone()),
                        Name::new(primitive.name.clone()),
                    ));

                    if let Some(material) = primitive.material.as_ref() {
                        child.insert(MeshMaterial3d(material.clone()));
                    }
                }
            });

            for node in node.children.iter() {
                commands.run_system_cached_with(spawn_gltf_node, node.clone());
            }
        }
    }
}

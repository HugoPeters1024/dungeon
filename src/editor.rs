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
use bevy_editor::{EditorCamera, Prefabs, SpawnPosition, bevy_panorbit_camera::PanOrbitCamera};
use bevy_tnua::TnuaNotPlatform;

pub struct EditorPlugin;

/// Marker component for the dedicated editor camera (separate from player camera)
#[derive(Component)]
pub struct DedicatedEditorCamera;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::picking::prelude::MeshPickingPlugin);
        app.add_plugins(bevy_editor::EditorPlugin::new().run_if(in_state(MyStates::Editor)));
        app.add_systems(
            OnEnter(MyStates::Editor),
            (setup_prefabs, enable_editor_camera),
        );
        app.add_systems(OnExit(MyStates::Editor), disable_editor_camera);
    }
}

/// Spawns or enables the dedicated editor camera when entering editor mode
fn enable_editor_camera(
    mut commands: Commands,
    mut player_cam: Single<&mut Camera, With<ControllerCamera>>,
    editor_cam: Option<
        Single<(Entity, &mut Camera), (With<DedicatedEditorCamera>, Without<ControllerCamera>)>,
    >,
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
            MeshPickingCamera,
        ));
    }
}

/// Disables the editor camera and re-enables the player camera when exiting editor mode
fn disable_editor_camera(
    mut player_cam: Single<&mut Camera, With<ControllerCamera>>,
    editor_cam: Option<
        Single<&mut Camera, (With<DedicatedEditorCamera>, Without<ControllerCamera>)>,
    >,
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
    prefabs.register_prefab(
        "Bong",
        commands.register_system(
            |mut commands: Commands, assets: Res<GameAssets>, spawn_pos: Res<SpawnPosition>| {
                commands.spawn((
                    Mesh3d(assets.bong.clone()),
                    MeshMaterial3d(assets.bong_material.clone()),
                    Transform::from_translation(spawn_pos.0).with_scale(Vec3::splat(0.3)),
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
            },
        ),
    );

    let gltf = gltfs.get(assets.castle_test.id()).unwrap();
    for (name, node) in gltf.named_nodes.iter() {
        prefabs.register_prefab(
            name.clone(),
            commands.register_system(spawn_gltf_node_system(node.clone())),
        );
    }
}

fn spawn_gltf_node_system(
    node: Handle<GltfNode>,
) -> impl Fn(Commands, Res<SpawnPosition>, Res<Assets<GltfNode>>, Res<Assets<GltfMesh>>) {
    move |mut commands: Commands,
          spawn_pos: Res<SpawnPosition>,
          nodes: Res<Assets<GltfNode>>,
          meshes: Res<Assets<GltfMesh>>| {
        spawn_gltf_node(&mut commands, &spawn_pos, &nodes, &meshes, node.clone());
    }
}

fn spawn_gltf_node(
    commands: &mut Commands,
    spawn_pos: &Res<SpawnPosition>,
    nodes: &Res<Assets<GltfNode>>,
    meshes: &Res<Assets<GltfMesh>>,
    node_handle: Handle<GltfNode>,
) {
    let Some(node) = nodes.get(node_handle.id()) else {
        return;
    };
    let mut builder = commands.spawn((
        Name::new(node.name.clone()),
        InheritedVisibility::default(),
        Transform::from_translation(spawn_pos.0),
    ));
    if let Some(mesh_handle) = node.mesh.as_ref()
        && let Some(mesh) = meshes.get(mesh_handle.id()).as_ref()
    {
        builder.with_children(|parent| {
            for primitive in mesh.primitives.iter() {
                let mut child = parent.spawn((
                    Mesh3d(primitive.mesh.clone()),
                    Name::new(primitive.name.clone()),
                    RigidBody::Static,
                    ColliderConstructor::TrimeshFromMesh,
                ));

                if let Some(material) = primitive.material.as_ref() {
                    child.insert(MeshMaterial3d(material.clone()));
                }
            }
        });

        for child_node in node.children.iter() {
            // Child nodes spawn at origin relative to parent, not at spawn_pos
            spawn_gltf_node_child(commands, nodes, meshes, child_node.clone());
        }
    }
}

fn spawn_gltf_node_child(
    commands: &mut Commands,
    nodes: &Res<Assets<GltfNode>>,
    meshes: &Res<Assets<GltfMesh>>,
    node_handle: Handle<GltfNode>,
) {
    let Some(node) = nodes.get(node_handle.id()) else {
        return;
    };
    let mut builder = commands.spawn((
        Name::new(node.name.clone()),
        InheritedVisibility::default(),
        Transform::default(),
    ));
    if let Some(mesh_handle) = node.mesh.as_ref()
        && let Some(mesh) = meshes.get(mesh_handle.id()).as_ref()
    {
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

        for child_node in node.children.iter() {
            spawn_gltf_node_child(commands, nodes, meshes, child_node.clone());
        }
    }
}

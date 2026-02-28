use crate::{
    assets::{GameAssets, MyStates},
    game::Pickupable,
    grid_wall::GridWall,
    player::controller::ControllerCamera,
};
use avian3d::prelude::*;
use bevy::{
    gltf::{GltfMesh, GltfNode},
    prelude::*,
};
use bevy_editor::{
    AssetRef, AssetRefRegistry, EditorCamera, Prefabs, bevy_panorbit_camera::PanOrbitCamera,
};
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
    mut asset_refs: ResMut<AssetRefRegistry>,
    assets: Res<GameAssets>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
) {
    asset_refs.register(&mut commands, "bong_assets", |assets: Res<GameAssets>| {
        (
            Mesh3d(assets.bong.clone()),
            MeshMaterial3d(assets.bong_material.clone()),
        )
    });

    prefabs.register_prefab(&mut commands, "Bong", || {
        (
            AssetRef::new("bong_assets"),
            Transform::from_scale(Vec3::splat(0.3)),
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
        )
    });

    prefabs.register_prefab(&mut commands, "GridWall", || (GridWall::default(),));

    let gltf = gltfs.get(assets.castle_test.id()).unwrap();

    // Register an AssetRef for every primitive in every named mesh
    for (_, mesh_handle) in &gltf.named_meshes {
        let Some(gltf_mesh) = gltf_meshes.get(mesh_handle) else {
            continue;
        };
        for primitive in gltf_mesh.primitives.iter() {
            let mesh_h = primitive.mesh.clone();
            if let Some(mat_h) = primitive.material.clone() {
                asset_refs.register(&mut commands, &primitive.name, move || {
                    (Mesh3d(mesh_h.clone()), MeshMaterial3d(mat_h.clone()))
                });
            } else {
                asset_refs.register(&mut commands, &primitive.name, move || {
                    Mesh3d(mesh_h.clone())
                });
            }
        }
    }

    // Register a prefab spawner for every top-level named node
    for (name, node_handle) in gltf.named_nodes.iter() {
        let h = node_handle.clone();
        prefabs.register_prefab_spawner(
            &mut commands,
            name.clone(),
            move |In(entity): In<Entity>,
                  mut commands: Commands,
                  gltf_meshes: Res<Assets<GltfMesh>>,
                  gltf_nodes: Res<Assets<GltfNode>>| {
                let mut parent = commands.entity(entity);
                spawn_node_hierarchy(&mut parent, &gltf_nodes, &gltf_meshes, &h);
            },
        );
    }
}

fn spawn_node_hierarchy(
    parent: &mut EntityCommands,
    nodes: &Res<Assets<GltfNode>>,
    meshes: &Res<Assets<GltfMesh>>,
    node: &Handle<GltfNode>,
) {
    let node = nodes.get(node).unwrap();

    parent.insert(Name::new(node.name.clone()));

    if let Some(mesh) = node.mesh.as_ref() {
        let mesh = meshes.get(mesh).unwrap();
        parent.with_children(|parent| {
            for primitive in mesh.primitives.iter() {
                parent.spawn((AssetRef::new(&primitive.name),RigidBody::Static, ColliderConstructor::ConvexHullFromMesh ));
            }
        });
    }

    parent.with_children(|parent| {
        for child in node.children.iter() {
            let mut child_builder = parent.spawn_empty();
            spawn_node_hierarchy(&mut child_builder, nodes, meshes, child);
        }
    });
}

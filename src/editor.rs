use crate::{
    assets::{GameAssets, MyStates},
    game::Pickupable,
    player::controller::ControllerCamera,
};
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_editor::Prefabs;
use bevy_tnua::TnuaNotPlatform;

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::picking::prelude::MeshPickingPlugin);
        app.add_plugins(bevy_editor::EditorPlugin::new().run_if(in_state(MyStates::Editor)));
        app.add_systems(OnEnter(MyStates::Editor), setup_prefabs);
        app.add_systems(
            OnExit(MyStates::Editor),
            |mut cam: Single<&mut Camera, With<ControllerCamera>>| {
                dbg!("Reset the viewport to fullscreen");
                cam.viewport = None;
            },
        );
    }
}

fn setup_prefabs(mut commands: Commands) {
    let mut prefabs = Prefabs::default();
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
    commands.insert_resource(prefabs);
}

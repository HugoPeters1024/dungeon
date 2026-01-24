use bevy::{ecs::entity::EntitySetIterator, platform::collections::HashSet, prelude::*};

use crate::PrefabId;

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (save_scene, load_scene));
    }
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

use bevy::{
    camera::primitives::Aabb, ecs::entity::EntitySetIterator, platform::collections::HashSet,
    prelude::*,
};

use crate::{PrefabId, actions::TrashRoot};

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (save_scene, load_scene));
    }
}

fn do_the_save(In(trashed): In<Vec<Entity>>, world: &mut World) {
    let mut entities: HashSet<Entity> = world
        .query_filtered::<Entity, With<PrefabId>>()
        .iter(world)
        .collect_set();

    for t in trashed.iter() {
        entities.remove(t);
    }

    // Only keep root PrefabId entities: exclude any whose ancestor also has PrefabId
    entities.retain(|&entity| {
        let mut current = entity;
        while let Some(child_of) = world.get::<ChildOf>(current) {
            let parent = child_of.parent();
            if world.get::<PrefabId>(parent).is_some() {
                return false;
            }
            current = parent;
        }
        true
    });

    let scene = DynamicSceneBuilder::from_world(world)
        .deny_all_components()
        .allow_component::<PrefabId>()
        .allow_component::<Transform>()
        .allow_component::<InheritedVisibility>()
        .allow_component::<Visibility>()
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

fn save_scene(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    trash: Res<TrashRoot>,
    children: Query<&Children>,
) {
    if keyboard.just_pressed(KeyCode::KeyS) {
        commands.run_system_cached_with(do_the_save, children.iter_descendants(trash.0).collect());
    }
}

fn load_scene(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    asset_server: Res<AssetServer>,
) {
    if keyboard.just_pressed(KeyCode::KeyL) {
        commands.spawn((
            DynamicSceneRoot(asset_server.load("scene.scn.ron")),
            Visibility::default(),
            Transform::default(),
        ));
        info!("Loading scene from scene.scn.ron");
    }
}

use bevy::{platform::collections::HashSet, prelude::*};

use crate::{actions::TrashRoot, asset_ref::AssetRef};

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (save_scene, load_scene));
    }
}

fn do_the_save(In(trashed): In<Vec<Entity>>, world: &mut World) {
    let mut roots: HashSet<Entity> = world.query::<Entity>().iter(world).collect();

    for t in trashed.iter() {
        roots.remove(t);
    }

    // Collect all entities: roots + their entire descendant hierarchies
    let mut all_entities = roots.clone();
    for root in roots.iter() {
        collect_descendants(world, *root, &mut all_entities);
    }

    let scene = DynamicSceneBuilder::from_world(world)
        .deny_all_components()
        .allow_component::<AssetRef>()
        .allow_component::<Transform>()
        .allow_component::<Name>()
        .allow_component::<Visibility>()
        .allow_component::<InheritedVisibility>()
        .allow_component::<ChildOf>()
        .extract_entities(all_entities.iter().cloned())
        .remove_empty_entities()
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

fn collect_descendants(world: &World, entity: Entity, out: &mut HashSet<Entity>) {
    if let Some(children) = world.get::<Children>(entity) {
        for child in children.iter() {
            out.insert(child);
            collect_descendants(world, child, out);
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
        commands.run_system_cached_with(
            do_the_save,
            std::iter::once(trash.0)
                .chain(children.iter_descendants(trash.0))
                .collect(),
        );
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

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
    if keyboard.just_pressed(KeyCode::KeyP) {
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
            Name::new("Loaded Scene"),
        ));
        info!("Loading scene from scene.scn.ron");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::entity::EntityHashMap;

    fn minimal_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<AssetRef>();
        app
    }

    fn build_save_scene(world: &mut World) -> DynamicScene {
        let all_entities: Vec<Entity> = world.query::<Entity>().iter(world).collect();
        DynamicSceneBuilder::from_world(world)
            .deny_all_components()
            .allow_component::<AssetRef>()
            .allow_component::<Transform>()
            .allow_component::<Name>()
            .allow_component::<Visibility>()
            .allow_component::<ChildOf>()
            .extract_entities(all_entities.into_iter())
            .remove_empty_entities()
            .build()
    }

    #[test]
    fn scene_round_trip_preserves_names() {
        // === SAVE ===
        let mut save_app = minimal_app();
        let world = save_app.world_mut();

        let parent = world
            .spawn((
                Transform::from_xyz(-0.99, 1.24, 2.47),
                Visibility::Inherited,
            ))
            .id();

        world.spawn((
            Name::new("Red Cube"),
            AssetRef::new("red_cube_assets"),
            Transform::from_xyz(0.0, 1.0, 0.0),
            Visibility::Inherited,
            ChildOf(parent),
        ));

        world.spawn((
            Name::new("Blue Sphere"),
            AssetRef::new("blue_sphere_assets"),
            Transform::default(),
            Visibility::Inherited,
            ChildOf(parent),
        ));

        let scene = build_save_scene(world);

        // === LOAD ===
        let mut load_app = minimal_app();
        let load_world = load_app.world_mut();

        let mut entity_map = EntityHashMap::default();
        scene
            .write_to_world(load_world, &mut entity_map)
            .expect("Failed to load scene");

        let names: Vec<String> = load_world
            .query::<&Name>()
            .iter(load_world)
            .map(|n| n.to_string())
            .collect();

        assert!(
            names.contains(&"Red Cube".to_string()),
            "Expected 'Red Cube', found: {names:?}"
        );
        assert!(
            names.contains(&"Blue Sphere".to_string()),
            "Expected 'Blue Sphere', found: {names:?}"
        );

        let refs: Vec<String> = load_world
            .query::<&AssetRef>()
            .iter(load_world)
            .map(|a| a.key().to_string())
            .collect();

        assert!(refs.contains(&"red_cube_assets".to_string()));
        assert!(refs.contains(&"blue_sphere_assets".to_string()));

        let child_count = load_world.query::<&ChildOf>().iter(load_world).count();
        assert_eq!(child_count, 2, "Expected 2 ChildOf relationships");
    }
}

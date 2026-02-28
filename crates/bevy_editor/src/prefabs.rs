use bevy::{
    ecs::{component::ComponentId, system::SystemId},
    platform::collections::{HashMap, HashSet},
    prelude::*,
};

use crate::asset_ref::AssetRefRegistry;

pub struct PrefabPlugin;

impl Plugin for PrefabPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<PrefabId>();
        app.init_resource::<Prefabs>();
        app.add_observer(on_prefab_id_spawn);
    }
}

/// Marker component for prefabs that haven't been spawned yet
#[derive(Component, Clone, Debug, Hash, PartialEq, Eq, Reflect)]
#[reflect(Component)]
#[require(Visibility, Transform)]
pub struct PrefabId(String);

impl PrefabId {
    pub fn new(x: impl Into<String>) -> Self {
        PrefabId(x.into())
    }
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl From<String> for PrefabId {
    fn from(value: String) -> Self {
        PrefabId(value)
    }
}

#[derive(Resource, Default)]
pub struct Prefabs {
    prefabs: HashMap<PrefabId, SystemId<In<Entity>, ()>>,
}

impl Prefabs {
    pub fn get_prefab_ids(&self) -> impl Iterator<Item = &PrefabId> {
        self.prefabs.keys()
    }

    pub fn register_prefab<M, B: Bundle + 'static, I: IntoSystem<(), B, M> + 'static>(
        &mut self,
        commands: &mut Commands,
        name: impl Into<String>,
        factory: I,
    ) {
        let get_bundle = commands.register_system(factory);
        let prefab_name = name.into();
        let prefab_name_for_warn = prefab_name.clone();
        let wrapper = move |In(target): In<Entity>, world: &mut World| {
            let before: HashSet<ComponentId> = world
                .entity(target)
                .archetype()
                .components()
                .iter()
                .copied()
                .collect();

            let bundle = world.run_system(get_bundle).unwrap();
            world.entity_mut(target).insert(bundle);

            warn_if_asset_components(world, target, &before, &prefab_name_for_warn);
        };

        let factory_id = commands.register_system(wrapper);
        self.prefabs.insert(PrefabId(prefab_name), factory_id);
    }

    /// Register a prefab that uses an `In<Entity>` system for full control over
    /// the spawned entity (adding children, running queries, etc.).
    pub fn register_prefab_spawner<M, I: IntoSystem<In<Entity>, (), M> + 'static>(
        &mut self,
        commands: &mut Commands,
        name: impl Into<String>,
        system: I,
    ) {
        let system_id = commands.register_system(system);
        self.prefabs.insert(PrefabId(name.into()), system_id);
    }
}

fn on_prefab_id_spawn(
    on: On<Add, PrefabId>,
    mut commands: Commands,
    prefab_ids: Query<&PrefabId>,
    prefabs: Res<Prefabs>,
) {
    let entity = on.event_target();

    let Ok(prefab_id) = prefab_ids.get(entity) else {
        return;
    };

    if let Some(factory) = prefabs.prefabs.get(prefab_id) {
        commands.entity(entity).remove::<PrefabId>();
        commands.run_system_with(*factory, entity);
    } else {
        warn!("No prefab factory registered for '{}'", prefab_id.name());
    };
}

fn warn_if_asset_components(
    world: &World,
    entity: Entity,
    before: &HashSet<ComponentId>,
    prefab_name: &str,
) {
    let Some(registry) = world.get_resource::<AssetRefRegistry>() else {
        return;
    };
    let asset_type_ids = registry.asset_type_ids();
    if asset_type_ids.is_empty() {
        return;
    }

    let newly_added: Vec<ComponentId> = world
        .entity(entity)
        .archetype()
        .components()
        .iter()
        .copied()
        .filter(|id| !before.contains(id))
        .collect();

    // If the prefab added an AssetRef, any asset handle components came from
    // hydration (the observer chain resolves synchronously), so don't warn.
    let added_asset_ref = newly_added.iter().any(|&id| {
        world
            .components()
            .get_info(id)
            .and_then(|info| info.type_id())
            .is_some_and(|tid| tid == std::any::TypeId::of::<crate::asset_ref::AssetRef>())
    });
    if added_asset_ref {
        return;
    }

    for &component_id in &newly_added {
        if let Some(info) = world.components().get_info(component_id) {
            if let Some(type_id) = info.type_id() {
                if asset_type_ids.contains(&type_id) {
                    warn!(
                        "Prefab '{}' inserted AssetComponent `{}` directly — use AssetRef instead",
                        prefab_name,
                        info.name()
                    );
                }
            }
        }
    }
}

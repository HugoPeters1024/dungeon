use bevy::{ecs::system::SystemId, platform::collections::HashMap, prelude::*};

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
    /// Legacy spawner systems that spawn their own entities
    spawners: HashMap<PrefabId, SystemId>,
}

impl Prefabs {
    pub fn get_prefab_ids(&self) -> impl Iterator<Item = &PrefabId> {
        self.prefabs.keys().chain(self.spawners.keys())
    }

    pub fn register_prefab<M, B: Bundle + 'static, I: IntoSystem<(), B, M> + 'static>(
        &mut self,
        commands: &mut Commands,
        name: impl Into<String>,
        factory: I,
    ) {
        let get_bundle = commands.register_system(factory);
        let wrapper = move |In(target): In<Entity>, world: &mut World| {
            let bundle = world.run_system(get_bundle).unwrap();
            world.entity_mut(target).insert(bundle);
        };

        let factory_id = commands.register_system(wrapper);
        self.prefabs.insert(PrefabId(name.into()), factory_id);
    }

    pub fn register_prefab_spawner<M>(
        &mut self,
        commands: &mut Commands,
        name: impl Into<String>,
        spawner: impl IntoSystem<(), (), M> + 'static,
    ) {
        let system_id = commands.register_system(spawner);
        self.spawners.insert(PrefabId(name.into()), system_id);
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
        commands.run_system_with(*factory, entity);
    };
}

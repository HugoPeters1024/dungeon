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
#[derive(Component, Clone, Hash, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct PrefabId(String);

impl PrefabId {
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl From<String> for PrefabId {
    fn from(value: String) -> Self {
        PrefabId(value)
    }
}

trait PrefabFactory: Send + Sync {
    fn spawn(&self, world: &mut World, entity: Entity);
}

struct PrefabFactoryImpl<B: Bundle> {
    factory_id: SystemId<(), B>,
}

impl<B: Bundle> PrefabFactory for PrefabFactoryImpl<B> {
    fn spawn(&self, world: &mut World, entity: Entity) {
        let bundle = world.run_system(self.factory_id).unwrap();
        world.entity_mut(entity).insert(bundle);
    }
}

#[derive(Resource, Default)]
pub struct Prefabs {
    prefabs: HashMap<PrefabId, Box<dyn PrefabFactory>>,
    /// Legacy spawner systems that spawn their own entities
    spawners: HashMap<PrefabId, SystemId>,
}

impl Prefabs {
    pub fn get_prefab_ids(&self) -> impl Iterator<Item = &PrefabId> {
        self.prefabs.keys().chain(self.spawners.keys())
    }

    pub fn register_prefab<M, B: Bundle + 'static>(
        &mut self,
        commands: &mut Commands,
        name: impl Into<String>,
        factory: impl IntoSystem<(), B, M> + 'static,
    ) {
        let factory_id: SystemId<(), B> = commands.register_system(factory);
        self.prefabs.insert(
            PrefabId(name.into()),
            Box::new(PrefabFactoryImpl { factory_id }),
        );
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

struct InsertPrefabCommand {
    entity: Entity,
    prefab_id: PrefabId,
}

impl Command for InsertPrefabCommand {
    fn apply(self, world: &mut World) {
        world.resource_scope(|world, prefabs: Mut<Prefabs>| {
            if let Some(factory) = prefabs.prefabs.get(&self.prefab_id) {
                factory.spawn(world, self.entity);
            } else if let Some(&spawner_id) = prefabs.spawners.get(&self.prefab_id) {
                world.run_system(spawner_id).unwrap();
            } else {
                warn!("Spawned unregistered prefab with id: {}", self.prefab_id.0);
            }
        });
    }
}

fn on_prefab_id_spawn(on: On<Add, PrefabId>, mut commands: Commands, prefab_ids: Query<&PrefabId>) {
    let entity = on.event_target();

    let Ok(prefab_id) = prefab_ids.get(entity) else {
        return;
    };

    commands.queue(InsertPrefabCommand {
        entity,
        prefab_id: prefab_id.clone(),
    });
}

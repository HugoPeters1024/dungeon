use bevy::{ecs::system::SystemId, platform::collections::HashMap, prelude::*};

pub struct PrefabPlugin;

impl Plugin for PrefabPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Prefabs>();
        app.add_observer(on_prefab_id_spawn);
    }
}

/// Marker component for prefabs that haven't been spawned yet
#[derive(Component, Clone, Hash, PartialEq, Eq)]
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

#[derive(Resource, Default)]
pub struct Prefabs {
    prefabs: HashMap<PrefabId, SystemId>,
}

impl std::ops::Deref for Prefabs {
    type Target = HashMap<PrefabId, SystemId>;
    fn deref(&self) -> &Self::Target {
        &self.prefabs
    }
}

struct SpawnPrefabCommand<B: Bundle> {
    system_id: SystemId<(), B>,
}

impl<B: Bundle> Command for SpawnPrefabCommand<B> {
    fn apply(self, world: &mut World) {
        let bundle = world.run_system(self.system_id).unwrap();
        world.spawn(bundle);
    }
}

impl Prefabs {
    pub fn register_prefab<M, B: Bundle + 'static>(
        &mut self,
        commands: &mut Commands,
        name: impl Into<String>,
        factory: impl IntoSystem<(), B, M> + 'static,
    ) {
        let factory_id: SystemId<(), B> = commands.register_system(factory);
        let spawn_system = commands.register_system(move |mut commands: Commands| {
            commands.queue(SpawnPrefabCommand {
                system_id: factory_id,
            });
        });
        self.prefabs.insert(PrefabId(name.into()), spawn_system);
    }

    pub fn register_prefab_spawner<M>(
        &mut self,
        commands: &mut Commands,
        name: impl Into<String>,
        spawner: impl IntoSystem<(), (), M> + 'static,
    ) {
        let system_id = commands.register_system(spawner);
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

    let Some(on_spawn) = prefabs.get(prefab_id).copied() else {
        warn!("Spawned unregistered prefab with id: {}", prefab_id.0);
        return;
    };

    commands.run_system(on_spawn);
}

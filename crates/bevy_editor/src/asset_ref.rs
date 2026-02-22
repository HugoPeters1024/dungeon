use std::any::TypeId;

use bevy::{ecs::system::SystemId, platform::collections::HashMap, prelude::*};

/// Marker trait for components that wrap asset handles (e.g., `Mesh3d`, `MeshMaterial3d`).
///
/// These should only be inserted via [`AssetRef`] hydration, not directly in prefab factories.
/// Implementing this trait opts a component into:
/// - Compile-time enforcement on [`AssetRefRegistry::register`]
/// - Runtime warnings if inserted directly by a prefab
pub trait AssetComponent: Component + 'static {}

impl AssetComponent for Mesh3d {}
impl<M: Material> AssetComponent for MeshMaterial3d<M> {}

/// Marker trait for bundles composed entirely of [`AssetComponent`] types.
/// Used to constrain what [`AssetRefRegistry::register`] accepts at compile time.
pub trait AssetBundle: Bundle {
    fn collect_type_ids(ids: &mut Vec<TypeId>);
}

impl<T: AssetComponent> AssetBundle for T {
    fn collect_type_ids(ids: &mut Vec<TypeId>) {
        ids.push(TypeId::of::<T>());
    }
}

macro_rules! impl_asset_bundle_tuple {
    ($($T:ident),+) => {
        impl<$($T: AssetBundle),+> AssetBundle for ($($T,)+) {
            fn collect_type_ids(ids: &mut Vec<TypeId>) {
                $($T::collect_type_ids(ids);)+
            }
        }
    };
}

impl_asset_bundle_tuple!(A, B);
impl_asset_bundle_tuple!(A, B, C);
impl_asset_bundle_tuple!(A, B, C, D);
impl_asset_bundle_tuple!(A, B, C, D, E);
impl_asset_bundle_tuple!(A, B, C, D, E, F);

/// A serializable reference to asset handles that gets hydrated at runtime.
///
/// When this component is added to an entity (via prefab spawn or scene load),
/// the [`AssetRefRegistry`] observer runs the registered factory for this key,
/// inserting the actual handle-based components (`Mesh3d`, `MeshMaterial3d`, etc.).
#[derive(Component, Clone, Debug, Hash, PartialEq, Eq, Reflect)]
#[reflect(Component)]
#[require(Visibility, Transform)]
pub struct AssetRef(String);

impl AssetRef {
    pub fn new(key: impl Into<String>) -> Self {
        AssetRef(key.into())
    }

    pub fn key(&self) -> &str {
        &self.0
    }
}

#[derive(Resource, Default)]
pub struct AssetRefRegistry {
    factories: HashMap<String, SystemId<In<Entity>, ()>>,
    asset_type_ids: Vec<TypeId>,
}

impl AssetRefRegistry {
    /// Register a hydration factory that returns an [`AssetBundle`] for the given key.
    ///
    /// The factory runs automatically whenever an entity with a matching [`AssetRef`]
    /// is spawned or loaded from a scene.
    pub fn register<M, B: AssetBundle + 'static, I: IntoSystem<(), B, M> + 'static>(
        &mut self,
        commands: &mut Commands,
        key: impl Into<String>,
        factory: I,
    ) {
        B::collect_type_ids(&mut self.asset_type_ids);

        let get_bundle = commands.register_system(factory);
        let wrapper = move |In(target): In<Entity>, world: &mut World| {
            let bundle = world.run_system(get_bundle).unwrap();
            world.entity_mut(target).insert(bundle);
        };

        let system_id = commands.register_system(wrapper);
        self.factories.insert(key.into(), system_id);
    }

    pub fn get(&self, key: &str) -> Option<&SystemId<In<Entity>, ()>> {
        self.factories.get(key)
    }

    /// Returns the `TypeId`s of all component types registered through asset bundles.
    /// Used by the prefab system to warn when these are inserted directly.
    pub fn asset_type_ids(&self) -> &[TypeId] {
        &self.asset_type_ids
    }
}

pub(crate) struct AssetRefPlugin;

impl Plugin for AssetRefPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<AssetRef>();
        app.init_resource::<AssetRefRegistry>();
        app.add_observer(on_asset_ref_added);
    }
}

fn on_asset_ref_added(
    on: On<Add, AssetRef>,
    mut commands: Commands,
    asset_refs: Query<&AssetRef>,
    registry: Res<AssetRefRegistry>,
) {
    let entity = on.event_target();
    let Ok(asset_ref) = asset_refs.get(entity) else {
        return;
    };

    if let Some(factory) = registry.get(asset_ref.key()) {
        info!("Running AssetRef {} for entity {}", asset_ref.key(), entity);
        commands.run_system_with(*factory, entity);
    } else {
        warn!("No factory registered for AssetRef '{}'", asset_ref.key());
    }
}

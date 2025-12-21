use std::marker::PhantomData;

use bevy::prelude::*;

#[derive(Component, Reflect)]
#[relationship(relationship_target = HasAnimationPlayer)]
pub struct AnimationPlayerOf(pub Entity);

#[derive(Component, Reflect)]
#[relationship_target(relationship = AnimationPlayerOf, linked_spawn)]
pub struct HasAnimationPlayer(Entity);

impl HasAnimationPlayer {
    pub fn target_entity(&self) -> Entity {
        self.0
    }
}

#[derive(Default)]
pub struct LinkAnimationPlayerPluginFor<T: Component>(PhantomData<T>);

impl<T: Component> Plugin for LinkAnimationPlayerPluginFor<T> {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, link_animation_player_for::<T>);
    }
}

fn link_animation_player_for<T: Component>(
    mut commands: Commands,
    on: Query<Entity, Added<AnimationPlayer>>,
    roots: Query<Entity, With<T>>,
    parents: Query<&ChildOf>,
) {
    for target in on.iter() {
        let Some(root) = parents
            .iter_ancestors(target)
            .find_map(|e| roots.get(e).ok())
        else {
            continue;
        };

        commands.entity(target).insert(AnimationPlayerOf(root));
    }
}

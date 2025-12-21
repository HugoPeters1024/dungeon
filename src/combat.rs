use bevy::prelude::*;

use crate::assets::MyStates;

#[derive(Component, Debug, Clone, Copy)]
pub struct Damageable {
    pub hp: f32,
}

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            cleanup_dead_damageables.run_if(in_state(MyStates::Next)),
        );
    }
}

fn cleanup_dead_damageables(mut commands: Commands, q: Query<(Entity, &Damageable)>) {
    for (e, d) in q.iter() {
        if d.hp <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

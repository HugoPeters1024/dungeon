use bevy::prelude::{Update, *};

use self::systems::move_enemies;

mod systems;

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, move_enemies);
    }
}

#[derive(Component)]
pub struct Enemy;

#[derive(Component)]
pub struct Patrol {
    pub points: Vec<Vec3>,
    pub target: usize,
}

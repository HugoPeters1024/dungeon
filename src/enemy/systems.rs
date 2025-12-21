use bevy::prelude::*;

use super::{Enemy, Patrol};

const PATROL_SPEED: f32 = 2.5;
const TARGET_REACHED_DISTANCE: f32 = 0.1;

pub fn move_enemies(
    mut enemies: Query<(&mut Transform, &mut Patrol), With<Enemy>>,
    time: Res<Time>,
) {
    for (mut transform, mut patrol) in &mut enemies {
        if patrol.points.is_empty() {
            continue;
        }

        let target_point = patrol.points[patrol.target];
        let direction = target_point - transform.translation;

        if direction.length() < TARGET_REACHED_DISTANCE {
            patrol.target = (patrol.target + 1) % patrol.points.len();
        } else {
            transform.translation += direction.normalize() * PATROL_SPEED * time.delta_secs();
        }
    }
}

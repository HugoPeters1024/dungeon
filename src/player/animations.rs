use std::ops::Deref;

use bevy::prelude::*;

use crate::{
    animations_utils::AnimationPlayerOf,
    assets::GameAssets,
    player::controller::{ControllerSensors, ControllerState},
};

#[derive(Debug, Default, Component)]
pub struct AnimationsT<T> {
    defeated: T,
    running: T,
    right_strafe: T,
    left_strafe: T,
    turn_around: T,
    jump: T,
    landing: T,
    walking: T,
}

impl<T> AnimationsT<T> {
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        [
            &self.defeated,
            &self.running,
            &self.right_strafe,
            &self.left_strafe,
            &self.turn_around,
            &self.jump,
            &self.landing,
            &self.walking,
        ]
        .into_iter()
    }
}

type AnimationClips = AnimationsT<AnimationNodeIndex>;
type AnimationWeights = AnimationsT<f32>;

#[derive(Debug, Clone)]
pub enum MovementLock {
    Full,
}

pub fn on_animation_player_loaded(
    on: On<Add, AnimationPlayerOf>,
    assets: Res<GameAssets>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut commands: Commands,
) {
    let mut graph = AnimationGraph::new();
    let clips = AnimationClips {
        defeated: graph.add_clip(assets.player_clips[0].clone(), 1.0, graph.root),
        running: graph.add_clip(assets.player_clips[1].clone(), 1.0, graph.root),
        right_strafe: graph.add_clip(assets.player_clips[2].clone(), 1.0, graph.root),
        left_strafe: graph.add_clip(assets.player_clips[3].clone(), 1.0, graph.root),
        turn_around: graph.add_clip(assets.player_clips[4].clone(), 1.0, graph.root),
        jump: graph.add_clip(assets.player_clips[5].clone(), 1.0, graph.root),
        landing: graph.add_clip(assets.player_clips[6].clone(), 1.0, graph.root),
        walking: graph.add_clip(assets.player_clips[7].clone(), 1.0, graph.root),
    };

    commands
        .entity(on.event_target())
        .insert(AnimationGraphHandle(graphs.add(graph)))
        .insert(clips)
        .insert(AnimationWeights::default());
}

pub fn animations_from_controller(
    mut q: Query<(
        &mut AnimationPlayer,
        &AnimationClips,
        &mut AnimationWeights,
        &AnimationPlayerOf,
    )>,
    c: Query<(&ControllerState, &ControllerSensors)>,
    mut prev_state: Local<ControllerState>,
) {
    for (mut player, clips, mut weights, AnimationPlayerOf(controller_entity)) in q.iter_mut() {
        let Ok((state, sensors)) = c.get(*controller_entity) else {
            continue;
        };

        let state_transioned =
            std::mem::discriminant(state) != std::mem::discriminant(prev_state.deref());

        use ControllerState::*;
        match state {
            Idle => {
                *weights = AnimationWeights {
                    defeated: 1.0,
                    ..default()
                };
            }
            Moving => {
                let forward = sensors
                    .actual_velocity
                    .dot(sensors.facing_direction)
                    .max(0.0);
                let left = sensors
                    .actual_velocity
                    .dot(sensors.facing_direction.cross(Vec3::NEG_Y))
                    .max(0.0);
                let right = sensors
                    .actual_velocity
                    .dot(sensors.facing_direction.cross(Vec3::Y))
                    .max(0.0);

                let mut w = AnimationWeights::default();
                if forward > 2.2 {
                    w.running = forward
                } else {
                    w.walking = forward
                };
                w.left_strafe = left;
                w.right_strafe = right;
                *weights = w;
            }
            PreparingJump(_) => {
                if state_transioned {
                    player.play(clips.jump).set_seek_time(0.26);
                }

                *weights = AnimationWeights {
                    jump: 1.0,
                    running: sensors.actual_velocity.length().max(1.0),
                    ..default()
                }
            }
            Jumping(_) => {
                *weights = AnimationWeights {
                    jump: 1.0,
                    ..default()
                }
            }
            Falling => {
                if state_transioned {
                    player.play(clips.landing).set_seek_time(0.0).set_speed(0.6);
                }
                *weights = AnimationWeights {
                    landing: 1.0,
                    ..default()
                }
            }
        }

        *prev_state = state.clone();
    }
}

pub fn apply_animation_weights(
    mut q: Query<(&AnimationWeights, &AnimationClips, &mut AnimationPlayer)>,
    time: Res<Time>,
) {
    for (weights, clips, mut player) in q.iter_mut() {
        for (&weight, &clip) in weights.iter().zip(clips.iter()) {
            let current_weight = player.animation(clip).map(|a| a.weight()).unwrap_or(0.0);
            let target_weight = weight;
            let interpolation_speed = 5.0;
            let new_weight = current_weight
                + (target_weight - current_weight) * interpolation_speed * time.delta_secs();
            player.play(clip).repeat().set_weight(new_weight);
        }
    }
}

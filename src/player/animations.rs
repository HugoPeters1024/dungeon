use std::ops::Deref;

use bevy::{
    animation::{AnimationTarget, AnimationTargetId},
    prelude::*,
};
use bevy_inspector_egui::egui::ahash::HashMap;

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
    slash: T,
    drop_kick: T,
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
            &self.slash,
            &self.drop_kick,
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
    mut players: Query<&mut AnimationPlayer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut commands: Commands,
    bones: Query<(&Name, &AnimationTarget)>,
    children: Query<&Children>,
) -> Result {
    let mut graph = AnimationGraph::new();

    let bone_lookup: HashMap<&str, (Entity, AnimationTargetId)> = children
        .iter_descendants(on.event_target())
        .flat_map(|e| {
            bones
                .get(e)
                .map(|(name, target)| (name.as_str(), (e, target.id)))
        })
        .collect();

    graph.add_target_to_mask_group(bone_lookup["mixamorigSpine"].1, 3);

    let clips = AnimationClips {
        defeated: graph.add_clip(assets.player_clips[0].clone(), 1.0, graph.root),
        running: graph.add_clip(assets.player_clips[1].clone(), 1.0, graph.root),
        right_strafe: graph.add_clip(assets.player_clips[2].clone(), 1.0, graph.root),
        left_strafe: graph.add_clip(assets.player_clips[3].clone(), 1.0, graph.root),
        turn_around: graph.add_clip(assets.player_clips[4].clone(), 1.0, graph.root),
        jump: graph.add_clip(assets.player_clips[5].clone(), 1.0, graph.root),
        landing: graph.add_clip(assets.player_clips[6].clone(), 1.0, graph.root),
        walking: graph.add_clip(assets.player_clips[7].clone(), 1.0, graph.root),
        slash: graph.add_clip_with_mask(assets.player_clips[8].clone(), 0b1000, 1.0, graph.root),
        drop_kick: graph.add_clip(assets.player_clips[9].clone(), 1.0, graph.root),
    };

    let mut player = players.get_mut(on.event_target())?;

    // Play all the loop continious animations
    player.play(clips.defeated).repeat();
    player.play(clips.running).repeat();
    player.play(clips.left_strafe).repeat();
    player.play(clips.right_strafe).repeat();
    player.play(clips.walking).repeat();

    commands
        .entity(on.event_target())
        .insert(AnimationGraphHandle(graphs.add(graph)))
        .insert(clips)
        .insert(AnimationWeights::default());

    Ok(())
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
                    .running_velocity
                    .dot(sensors.facing_direction)
                    .max(0.0);
                let left = sensors
                    .running_velocity
                    .dot(sensors.facing_direction.cross(Vec3::NEG_Y))
                    .max(0.0);
                let right = sensors
                    .running_velocity
                    .dot(sensors.facing_direction.cross(Vec3::Y))
                    .max(0.0);

                player
                    .animation_mut(clips.walking)
                    .map(|a| a.set_speed(sensors.running_velocity.length().sqrt().min(1.0)));

                player
                    .animation_mut(clips.running)
                    .map(|a| a.set_speed(sensors.running_velocity.length().sqrt().min(1.0)));

                let mut w = AnimationWeights::default();
                if forward > 3.0 {
                    w.running = forward
                } else {
                    w.walking = forward
                };
                w.left_strafe = left;
                w.right_strafe = right;
                *weights = w;
            }
            Jumping(_) => {
                if state_transioned {
                    player.start(clips.jump).set_seek_time(0.66);
                }

                *weights = AnimationWeights {
                    jump: 1.0,
                    ..default()
                }
            }
            Falling => {
                if state_transioned {
                    player
                        .start(clips.landing)
                        .set_seek_time(0.0)
                        .set_speed(0.3);
                }
                *weights = AnimationWeights {
                    landing: 1.0,
                    ..default()
                }
            }
            DropKicking(..) => {
                if state_transioned {
                    player
                        .start(clips.drop_kick)
                        .set_seek_time(0.0)
                        .set_speed(1.0);
                }
                *weights = AnimationWeights {
                    drop_kick: 1.0,
                    ..default()
                }
            }
            Attacking(_) => {
                if state_transioned {
                    player.start(clips.slash).set_seek_time(0.0).set_speed(1.8);
                }
                *weights = AnimationWeights {
                    slash: 1.0,
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
            if let Some(clip) = player.animation_mut(clip) {
                let current_weight = clip.weight();
                let target_weight = weight;
                let interpolation_speed = 5.0;
                let new_weight = current_weight
                    + (target_weight - current_weight) * interpolation_speed * time.delta_secs();

                clip.set_weight(new_weight);
            }
        }
    }
}

use std::ops::DerefMut;

use bevy::prelude::*;
use bevy_tnua::prelude::TnuaBuiltinJump;

use crate::{
    animations_utils::AnimationPlayerOf, assets::GameAssets, player::controller::ControllerSnapshot,
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

#[derive(Component, Debug, Clone)]
pub enum AnimationState {
    Moving { forward: f32, left: f32, right: f32 },
    Idle,
    PreparingJump(Timer),
    Jumping,
}

impl AnimationState {
    pub fn is_valid_transition(&self, other: &AnimationState, s: &ControllerSnapshot) -> bool {
        use AnimationState::*;
        match (self, other) {
            (Moving { .. } | Idle, Moving { .. }) => true,
            (Moving { .. }, Idle { .. }) => true,
            (Jumping { .. }, Idle { .. } | Moving { .. }) => s.standing_on_ground,
            (Moving { .. } | Idle, PreparingJump { .. }) => true,
            (PreparingJump { .. }, Jumping { .. }) => true,
            _ => false,
        }
    }
}

#[derive(Component)]
pub struct OldAnimationState(AnimationState);

#[derive(Debug, Clone)]
pub enum MovementLock {
    Full,
}

#[derive(Component, Default)]
pub struct AnimationInfluence {
    pub movement_lock: Option<MovementLock>,
    pub jump_action: Option<TnuaBuiltinJump>,
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
        .insert(AnimationState::Idle)
        .insert(AnimationInfluence::default());
}

pub fn save_animation_state(mut commands: Commands, q: Query<(Entity, &AnimationState)>) {
    for (entity, state) in q.iter() {
        dbg!(&state);
        commands
            .entity(entity)
            .insert(OldAnimationState(state.clone()));
    }
}

pub fn tick_animation_state(
    mut commands: Commands,
    mut q: Query<(Entity, &mut AnimationState)>,
    time: Res<Time>,
) {
    for (entity, mut state) in q.iter_mut() {
        let next_state = match state.deref_mut() {
            AnimationState::PreparingJump(timer) => {
                if timer.tick(time.delta()).is_finished() {
                    Some(AnimationState::Jumping)
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(next_state) = next_state {
            commands.entity(entity).insert(next_state);
        }
    }
}

pub fn update_animation_state(
    mut commands: Commands,
    q: Query<(Entity, &AnimationState, &AnimationPlayerOf)>,
    c: Query<&ControllerSnapshot>,
) {
    for (anim_entity, anim_state, AnimationPlayerOf(c_entity)) in q.iter() {
        let Ok(c_snapshot) = c.get(*c_entity) else {
            continue;
        };

        let mut new_state = AnimationState::Idle;
        if c_snapshot.desired_velocity.length() > 0.01 {
            new_state = AnimationState::Moving {
                forward: c_snapshot
                    .actual_velocity
                    .dot(c_snapshot.facing_direction)
                    .max(0.0),
                left: c_snapshot
                    .actual_velocity
                    .dot(c_snapshot.facing_direction.cross(Vec3::NEG_Y))
                    .max(0.0),
                right: c_snapshot
                    .actual_velocity
                    .dot(c_snapshot.facing_direction.cross(Vec3::Y))
                    .max(0.0),
            }
        }

        if c_snapshot.wants_to_jump {
            new_state = AnimationState::PreparingJump(Timer::from_seconds(0.5, TimerMode::Once));
        }

        if anim_state.is_valid_transition(&new_state, &c_snapshot) {
            commands.entity(anim_entity).insert(new_state);
        }
    }
}

pub fn update_animation_weights(mut commands: Commands, mut q: Query<(Entity, &AnimationState)>) {
    for (entity, anim_state) in q.iter_mut() {
        let mut weights = AnimationWeights::default();
        match anim_state {
            AnimationState::Moving {
                forward,
                left,
                right,
            } => {
                if *forward > 2.22 {
                    weights.running = 1.0;
                } else if *forward > 0.01 {
                    weights.walking = 1.0;
                }

                weights.left_strafe = *left;
                weights.right_strafe = *right;
            }
            AnimationState::Idle => {
                weights.defeated = 1.0;
            }
            AnimationState::PreparingJump(_timer) => {
                weights.jump = 1.0;
            }
            AnimationState::Jumping => {
                weights.jump = 1.0;
            }
        };

        commands.entity(entity).insert(weights);
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

pub fn on_animation_state_transitions(
    mut q: Query<(
        &OldAnimationState,
        &AnimationState,
        &mut AnimationPlayer,
        &AnimationClips,
        &mut AnimationInfluence,
    )>,
) {
    for (OldAnimationState(old_state), new_state, mut player, clips, mut influence) in q.iter_mut()
    {
        if std::mem::discriminant(new_state) == std::mem::discriminant(old_state) {
            continue;
        }
        influence.jump_action = None;

        match new_state {
            AnimationState::PreparingJump(_) => {
                player.play(clips.jump).set_seek_time(0.0);
            }
            AnimationState::Jumping => {
                influence.jump_action = Some(TnuaBuiltinJump {
                    // The height is the only mandatory field of the jump button.
                    height: 2.5,
                    fall_extra_gravity: 10.5,
                    // `TnuaBuiltinJump` also has customization fields with sensible defaults.
                    ..Default::default()
                });
            }
            _ => {}
        }
    }
}

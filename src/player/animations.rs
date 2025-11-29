use bevy::prelude::*;

use crate::{
    animations_utils::AnimationPlayerOf, assets::GameAssets, player::controller::ControllerSnapshot,
};

/// Animation Clip State
#[derive(Debug)]
struct ACS {
    idx: AnimationNodeIndex,
    target_weight: f32,
}

impl ACS {
    fn new(idx: AnimationNodeIndex) -> Self {
        Self {
            idx,
            target_weight: 0.0,
        }
    }
}

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

type AnimationClips = AnimationsT<AnimationNodeIndex>;
type AnimationWeights = AnimationsT<f32>;

#[derive(Component, Debug)]
pub enum AnimationState {
    Moving { forward: f32, left: f32, right: f32 },
    Idle,
    PreparingJump(Timer),
    Jumping,
}

#[derive(Debug, Clone)]
pub enum MovementLock {
    Full,
}

#[derive(Component, Default)]
pub struct AnimationInfluence {
    pub movement_lock: Option<MovementLock>,
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
        .insert(AnimationState::Idle);
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
                forward: c_snapshot.desired_velocity.length(),
                left: 0.0,
                right: 0.0,
            }
        }

        dbg!(&new_state);
        commands.entity(anim_entity).insert(new_state);
    }
}

pub fn update_animation_weights(
    mut commands: Commands,
    mut q: Query<(Entity, &AnimationState, &mut AnimationPlayer)>,
) {
    for (entity, anim_state, mut player) in q.iter_mut() {
        let mut weights = AnimationWeights::default();
        match anim_state {
            AnimationState::Moving {
                forward,
                left,
                right,
            } => {
                if *forward > 0.22 {
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
) {
    for (weights, clips, mut player) in q.iter_mut() {
        dbg!(&weights);
        player
            .play(clips.defeated)
            .repeat()
            .set_weight(weights.defeated);
        player
            .play(clips.running)
            .repeat()
            .set_weight(weights.running);
        player
            .play(clips.right_strafe)
            .repeat()
            .set_weight(weights.right_strafe);
        player
            .play(clips.left_strafe)
            .repeat()
            .set_weight(weights.left_strafe);
        player
            .play(clips.turn_around)
            .repeat()
            .set_weight(weights.turn_around);
        player.play(clips.jump).repeat().set_weight(weights.jump);
        player
            .play(clips.landing)
            .repeat()
            .set_weight(weights.landing);
        player
            .play(clips.walking)
            .repeat()
            .set_weight(weights.walking);
    }
}

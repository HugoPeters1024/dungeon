use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::{TnuaAnimatingState, builtins::TnuaBuiltinJumpState, prelude::*};
use bevy_tnua_avian3d::prelude::*;

use crate::{
    animations_utils::{HasAnimationPlayer},
    assets::{GameAssets},
};

use crate::game::PlayerCamera;

#[derive(Component, Default)]
#[require(Transform, InheritedVisibility)]
pub struct PlayerRoot;

#[derive(Debug)]
pub enum AnimationState {
    Standing,
    Running(Vec3),
    Jumping,
    Falling,
    Landing,
    Walking(Vec3),
}

#[derive(Component, Default, Debug)]
pub struct PlayerAnimations<T> {
    pub running: T,
    pub defeated: T,
    pub right_strafe: T,
    pub left_strafe: T,
    pub a180: T,
    pub jump: T,
    pub falling_landing: T,
    pub walking: T,
}

impl PlayerAnimations<f32> {
    fn new() -> Self {
        Self {
            running: 0.0,
            defeated: 1.0,
            right_strafe: 0.0,
            left_strafe: 0.0,
            a180: 0.0,
            jump: 0.0,
            falling_landing: 0.0,
            walking: 0.0,
        }
    }
}

impl<T> PlayerAnimations<T> {
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        [
            &self.running,
            &self.defeated,
            &self.right_strafe,
            &self.left_strafe,
            &self.a180,
            &self.jump,
            &self.falling_landing,
            &self.walking,
        ]
        .into_iter()
    }
}

pub fn update_animation_weights(
    mut q: Query<(
        &PlayerAnimations<AnimationNodeIndex>,
        &PlayerAnimations<f32>,
        &mut AnimationPlayer,
    )>,
    time: Res<Time>,
) {
    const WEIGHT_INTERPOLATION_SPEED: f32 = 6.0;

    for (nodes, weights, mut player) in q.iter_mut() {
        for (&node, &target_weight) in nodes.iter().zip(weights.iter()) {
            let animation = player.play(node).repeat();
            let current_weight = animation.weight();
            let new_weight = current_weight
                + (target_weight - current_weight) * WEIGHT_INTERPOLATION_SPEED * time.delta_secs();
            animation.set_weight(new_weight);
        }
    }
}

pub fn on_player_spawn(on: On<Add, PlayerRoot>, mut commands: Commands, assets: Res<GameAssets>) {
    commands.entity(on.event_target()).insert((
        children![(
            SceneRoot(assets.player.clone()),
            Transform::from_scale(Vec3::splat(0.008)),
        )],
        // Spawn at appropriate height: ground is at Y=0.05 (top of 0.1 thick floor)
        // Capsule bottom should be at ground level, so center at 0.05 + 0.8 = 0.85
        Transform::from_xyz(0.0, 0.85, 0.0),
        InheritedVisibility::default(),
        RigidBody::Dynamic,
        Collider::capsule(0.3, 1.0),
        TnuaController::default(),
        TnuaAvian3dSensorShape(Collider::cylinder(0.29, 0.0)),
        TnuaAnimatingState::<AnimationState>::default(),
        RayCaster::new(Vec3::new(0.0, 0.0, 0.05), Dir3::NEG_Y),
    ));
}

pub fn setup_animation(
    mut on: Query<(&HasAnimationPlayer, &PlayerRoot), Added<HasAnimationPlayer>>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    for (player_ref, _) in on.iter_mut() {
        let mut graph = AnimationGraph::new();
        let animations = PlayerAnimations {
            running: graph.add_clip(assets.player_animations[1].clone(), 1.0, graph.root),
            defeated: graph.add_clip(assets.player_animations[0].clone(), 1.0, graph.root),
            right_strafe: graph.add_clip(assets.player_animations[2].clone(), 1.0, graph.root),
            left_strafe: graph.add_clip(assets.player_animations[3].clone(), 1.0, graph.root),
            a180: graph.add_clip(assets.player_animations[4].clone(), 1.0, graph.root),
            jump: graph.add_clip(assets.player_animations[5].clone(), 1.0, graph.root),
            falling_landing: graph.add_clip(assets.player_animations[6].clone(), 1.0, graph.root),
            walking: graph.add_clip(assets.player_animations[7].clone(), 1.0, graph.root),
        };

        commands
            .entity(player_ref.target_entity())
            .insert(AnimationGraphHandle(graphs.add(graph)))
            .insert(animations)
            .insert(PlayerAnimations::<f32>::new());
    }
}

pub fn update_animation_state(
    mut q: Query<(
        &mut TnuaAnimatingState<AnimationState>,
        &TnuaController,
        &HasAnimationPlayer,
        &RayHits,
        &Transform,
    )>,
    mut animation_players: Query<(
        &mut AnimationPlayer,
        &PlayerAnimations<AnimationNodeIndex>,
        &mut PlayerAnimations<f32>,
    )>,
) {
    for (mut state, controller, has_player, hits, player_transform) in q.iter_mut() {
        let Ok((mut animation_player, character_animations, mut animation_weights)) =
            animation_players.get_mut(has_player.target_entity())
        else {
            continue;
        };

        let new_state = match controller.action_name() {
            Some(TnuaBuiltinJump::NAME) => {
                // In case of jump, we want to cast it so that we can get the concrete jump
                // state.
                let (_, jump_state) = controller
                    .concrete_action::<TnuaBuiltinJump>()
                    .expect("action name mismatch");
                // Depending on the state of the jump, we need to decide if we want to play the
                // jump animation or the fall animation.
                match jump_state {
                    TnuaBuiltinJumpState::NoJump => continue,
                    TnuaBuiltinJumpState::StartingJump { .. } => AnimationState::Jumping,
                    TnuaBuiltinJumpState::SlowDownTooFastSlopeJump { .. } => {
                        AnimationState::Jumping
                    }
                    TnuaBuiltinJumpState::MaintainingJump { .. } => AnimationState::Jumping,
                    TnuaBuiltinJumpState::StoppedMaintainingJump => AnimationState::Jumping,
                    TnuaBuiltinJumpState::FallSection => {
                        if let Some(hit) = hits.iter_sorted().next()
                            && hit.distance < 1.4
                        {
                            AnimationState::Landing
                        } else {
                            AnimationState::Falling
                        }
                    }
                }
            }
            Some(other) => {
                warn!("Unknown action: {other}");
                AnimationState::Standing
            }
            None => {
                // If there is no action going on, we'll base the animation on the state of the
                // basis.
                let Some((_, basis_state)) = controller.concrete_basis::<TnuaBuiltinWalk>() else {
                    continue;
                };
                if basis_state.standing_on_entity().is_none() {
                    AnimationState::Falling
                } else {
                    let speed = basis_state.running_velocity.length();
                    if speed > 2.5 {
                        AnimationState::Running(basis_state.running_velocity)
                    } else if speed > 0.01 {
                        AnimationState::Walking(basis_state.running_velocity)
                    } else {
                        AnimationState::Standing
                    }
                }
            }
        };

        match state.update_by_discriminant(new_state) {
            bevy_tnua::TnuaAnimatingStateDirective::Maintain { state } => {
                match state {
                    AnimationState::Running(velocity) | AnimationState::Walking(velocity) => {
                        // Get the forward and right directions in world space
                        let forward = player_transform.rotation * Vec3::Z;
                        let right = player_transform.rotation * Vec3::NEG_X;

                        // Normalize velocity to get direction
                        let velocity_dir = velocity.normalize_or_zero();

                        // Calculate how much we're moving forward vs sideways
                        let forward_amount = velocity_dir.dot(forward).max(0.0);
                        let right_amount = velocity_dir.dot(right);

                        // Determine strafe weights (only one should be active at a time)
                        let left_strafe = (-right_amount).max(0.0);
                        let right_strafe = right_amount.max(0.0);

                        *animation_weights = PlayerAnimations {
                            running: if matches!(state, AnimationState::Running(_)) {
                                forward_amount
                            } else {
                                0.0
                            },
                            walking: if matches!(state, AnimationState::Walking(_)) {
                                forward_amount
                            } else {
                                0.0
                            },
                            left_strafe,
                            right_strafe,
                            ..default()
                        };
                    }
                    _ => {}
                };
            }
            bevy_tnua::TnuaAnimatingStateDirective::Alter { old_state: _, state } => {
                let weights = match state {
                    AnimationState::Standing => PlayerAnimations {
                        defeated: 1.0,
                        ..default()
                    },
                    AnimationState::Running(_) => PlayerAnimations {
                        running: 1.0,
                        ..default()
                    },
                    AnimationState::Jumping => {
                        animation_player
                            .play(character_animations.jump)
                            .set_seek_time(0.5)
                            .set_speed(1.6);

                        PlayerAnimations {
                            jump: 1.0,
                            ..default()
                        }
                    }
                    AnimationState::Falling => {
                        animation_player
                            .play(character_animations.falling_landing)
                            .set_seek_time(0.0)
                            .set_speed(0.1);
                        PlayerAnimations {
                            falling_landing: 1.0,
                            ..default()
                        }
                    }
                    AnimationState::Landing => {
                        animation_player
                            .play(character_animations.falling_landing)
                            .set_speed(1.3);
                        PlayerAnimations {
                            falling_landing: 1.0,
                            ..default()
                        }
                    }
                    AnimationState::Walking(_) => PlayerAnimations {
                        walking: 1.0,
                        ..default()
                    },
                };

                *animation_weights = weights;
            }
        };
    }
}

pub fn apply_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut controller_query: Query<(&mut TnuaController, &Transform)>,
) {
    let Ok((mut controller, transform)) = controller_query.single_mut() else {
        return;
    };

    // Get the character's forward direction from its rotation
    // In Bevy, -Z is forward, so we rotate Vec3::NEG_Z by the character's rotation
    let forward = transform.rotation * Vec3::Z;
    let sideways = transform.rotation * Vec3::X;
    const FORWARD_SPEED: f32 = 2.0;
    const SIDEWAYS_SPEED: f32 = 2.0;

    let sprint_factor = if keyboard.pressed(KeyCode::ShiftLeft) {
        1.8
    } else {
        1.0
    };

    // W/S move forward/backward relative to character's rotation
    let mut direction = Vec3::ZERO;
    if keyboard.pressed(KeyCode::KeyW) {
        direction += forward * FORWARD_SPEED * sprint_factor;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        direction -= forward * FORWARD_SPEED;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        direction += sideways * SIDEWAYS_SPEED;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        direction -= sideways * SIDEWAYS_SPEED;
    }

    // Feed the basis every frame. Even if the player doesn't move - just use `desired_velocity:
    // Vec3::ZERO`. `TnuaController` starts without a basis, which will make the character collider
    // just fall.
    controller.basis(TnuaBuiltinWalk {
        // The `desired_velocity` determines how the character will move.
        desired_velocity: direction,
        // The `float_height` must be greater (even if by little) from the distance between the
        // character's center and the lowest point of its collider.
        // Capsule: radius 0.3, height 1.0 -> total height 1.6, center to bottom = 0.8
        // Using 0.85 to be slightly above the bottom point
        float_height: 0.85,
        // `TnuaBuiltinWalk` has many other fields for customizing the movement - but they have
        // sensible defaults. Refer to the `TnuaBuiltinWalk`'s documentation to learn what they do.
        ..Default::default()
    });

    // Feed the jump action every frame as long as the player holds the jump button. If the player
    // stops holding the jump button, simply stop feeding the action.
    if keyboard.pressed(KeyCode::Space) {
        controller.action(TnuaBuiltinJump {
            // The height is the only mandatory field of the jump button.
            height: 2.5,
            fall_extra_gravity: 10.5,
            // `TnuaBuiltinJump` also has customization fields with sensible defaults.
            ..Default::default()
        });
    }
}

/// Rotates the character to always face away from the camera (like Elden Ring)
pub fn rotate_character_to_camera(
    mut query: Query<&mut Transform, With<TnuaController>>,
    camera_query: Query<&PlayerCamera>,
    time: Res<Time>,
) {
    let Ok(mut transform) = query.single_mut() else {
        return;
    };

    let Ok(camera) = camera_query.single() else {
        return;
    };

    // Character should face away from camera (opposite direction)
    // Camera yaw is the direction camera is looking, so character faces camera_yaw + PI
    let target_yaw = camera.yaw + std::f32::consts::PI;

    let target_rotation = Quat::from_rotation_y(target_yaw);

    // Smoothly rotate character to match target
    const ROTATION_SPEED: f32 = 4.0; // radians per second
    transform.rotation = transform
        .rotation
        .slerp(target_rotation, ROTATION_SPEED * time.delta_secs());
}

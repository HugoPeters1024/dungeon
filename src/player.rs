use std::time::Duration;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::{TnuaAnimatingState, builtins::TnuaBuiltinJumpState, prelude::*};
use bevy_tnua_avian3d::prelude::*;

use crate::{
    animations_utils::{HasAnimationPlayer, LinkAnimationsPluginFor},
    assets::{CharacterAnimations, GameAssets, MyStates},
};

use super::PlayerCamera;

#[derive(Component, Default)]
#[require(Transform, InheritedVisibility)]
pub struct PlayerRoot;

#[derive(Debug)]
pub enum AnimationState {
    Standing,
    Running(f32),
    Jumping,
    Falling,
    Landing,
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LinkAnimationsPluginFor::<PlayerRoot>::default());
        app.add_observer(on_player_spawn);
        app.add_systems(
            Update,
            (
                print_hits,
                setup_animation,
                //update_animation_weights,
                update_animation_state,
                apply_controls,
                rotate_character_to_camera,
            )
                .run_if(in_state(MyStates::Next)),
        );
    }
}

fn on_player_spawn(on: On<Add, PlayerRoot>, mut commands: Commands, assets: Res<GameAssets>) {
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
        // Lock all rotation - we'll manually control Y rotation for facing direction
        //LockedAxes::ROTATION_LOCKED,
    ));
}

fn setup_animation(
    mut on: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
    mut commands: Commands,
    character_animations: Res<CharacterAnimations>,
    graphs: Res<Assets<AnimationGraph>>,
) {
    let Some((target, mut animation_player)) = on.iter_mut().next() else {
        return;
    };

    let graph = graphs.get(&character_animations.graph).unwrap();
    //for idx in graph.nodes() {
    //    animation_player.play(idx).repeat();
    //}

    commands
        .entity(target)
        .insert(AnimationGraphHandle(character_animations.graph.clone()))
        .insert(AnimationTransitions::new());

    //animation_player.play(character_animations.running).repeat();
    //animation_player
    //    .play(character_animations.defeated)
    //    .repeat();
}

fn print_hits(query: Query<(&RayCaster, &RayHits)>) {
    for (ray, hits) in &query {
        // For the faster iterator that isn't sorted, use `.iter()`
        for hit in hits.iter_sorted() {}
    }
}

fn update_animation_state(
    mut q: Query<(
        &mut TnuaAnimatingState<AnimationState>,
        &TnuaController,
        &HasAnimationPlayer,
        &RayCaster,
        &RayHits,
    )>,
    mut animation_players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    character_animations: Res<CharacterAnimations>,
) {
    for (mut state, controller, has_player, ray, hits) in q.iter_mut() {
        let Ok((mut animation_player, mut animation_transitions)) =
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
                            && hit.distance < 1.2
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
                    if 0.01 < speed {
                        AnimationState::Running(0.1 * speed)
                    } else {
                        AnimationState::Standing
                    }
                }
            }
        };

        match state.update_by_discriminant(new_state) {
            bevy_tnua::TnuaAnimatingStateDirective::Maintain { state } => {}
            bevy_tnua::TnuaAnimatingStateDirective::Alter { old_state, state } => {
                match state {
                    AnimationState::Standing => animation_transitions
                        .play(
                            &mut animation_player,
                            character_animations.defeated,
                            Duration::from_millis(300),
                        )
                        .repeat(),
                    AnimationState::Running(_) => animation_transitions
                        .play(
                            &mut animation_player,
                            character_animations.running,
                            Duration::from_millis(300),
                        )
                        .repeat(),
                    AnimationState::Jumping => animation_transitions
                        .play(
                            &mut animation_player,
                            character_animations.jump,
                            Duration::from_millis(200),
                        )
                        .set_seek_time(0.6),
                    AnimationState::Falling => animation_transitions
                        .play(
                            &mut animation_player,
                            character_animations.falling_landing,
                            Duration::from_millis(300),
                        )
                        .set_speed(0.05),
                    AnimationState::Landing => animation_transitions
                        .play(
                            &mut animation_player,
                            character_animations.falling_landing,
                            Duration::from_millis(100),
                        )
                        .set_speed(1.0),
                };
            }
        };
    }
}

fn apply_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut controller_query: Query<(&mut TnuaController, &Transform)>,
) {
    let Ok((mut controller, transform)) = controller_query.single_mut() else {
        return;
    };

    // Get the character's forward direction from its rotation
    // In Bevy, -Z is forward, so we rotate Vec3::NEG_Z by the character's rotation
    let forward = transform.rotation * Vec3::Z;

    // W/S move forward/backward relative to character's rotation
    let mut direction = Vec3::ZERO;
    if keyboard.pressed(KeyCode::KeyW) {
        direction += forward;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        direction -= forward;
    }

    const MOVEMENT_SPEED: f32 = 6.0;

    // Feed the basis every frame. Even if the player doesn't move - just use `desired_velocity:
    // Vec3::ZERO`. `TnuaController` starts without a basis, which will make the character collider
    // just fall.
    controller.basis(TnuaBuiltinWalk {
        // The `desired_velocity` determines how the character will move.
        desired_velocity: direction.normalize_or_zero() * MOVEMENT_SPEED,
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
            fall_extra_gravity: 0.0,
            // `TnuaBuiltinJump` also has customization fields with sensible defaults.
            ..Default::default()
        });
    }
}

/// Rotates the character to always face away from the camera (like Elden Ring)
fn rotate_character_to_camera(
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

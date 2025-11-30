use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::{builtins::TnuaBuiltinJumpState, prelude::*};
use bevy_tnua_avian3d::prelude::*;

use crate::animations_utils::HasAnimationPlayer;
use crate::assets::GameAssets;

use crate::game::PlayerCamera;
use crate::player::animations::AnimationInfluence;

#[derive(Component, Default)]
#[require(Transform, InheritedVisibility)]
pub struct PlayerRoot;

#[derive(Component, Default, Debug)]
pub struct ControllerSnapshot {
    pub desired_velocity: Vec3,
    pub actual_velocity: Vec3,
    pub facing_direction: Vec3,
    pub standing_on_ground: bool,
    pub distance_to_ground: f32,
    pub jump_state: Option<TnuaBuiltinJumpState>,
    pub wants_to_jump: bool,
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
        RayCaster::new(Vec3::new(0.0, 0.0, 0.05), Dir3::NEG_Y),
    ));
}

pub fn take_controller_snapshot(
    mut commands: Commands,
    mut q: Query<(
        Entity,
        &mut TnuaController,
        &RayHits,
        &Transform,
        &LinearVelocity,
        &HasAnimationPlayer
    )>,
    a: Query<&AnimationInfluence>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    for (entity, mut controller, hits, transform, velocity, has_player) in q.iter_mut() {
        let Ok(influence) = a.get(has_player.target_entity()) else {
            warn!("player has not animation player");
            continue;
        };
        // Initialize all fields as local bindings
        let distance_to_ground = hits.iter_sorted().next().map_or(0.0, |h| h.distance);
        let mut desired_velocity = Vec3::ZERO;
        let actual_velocity = velocity.0;
        let facing_direction = transform.rotation * Vec3::Z;
        let mut standing_on_ground = false;
        let mut jump_state = None;
        let mut wants_to_jump = false;

        match controller.action_name() {
            Some(TnuaBuiltinJump::NAME) => {
                // In case of jump, we want to cast it so that we can get the concrete jump
                // state.
                let (_, jump_state_inner) = controller
                    .concrete_action::<TnuaBuiltinJump>()
                    .expect("action name mismatch");
                // Depending on the state of the jump, we need to decide if we want to play the
                // jump animation or the fall animation.
                jump_state = Some(jump_state_inner.clone());
            }
            Some(other) => {
                warn!("Unknown action: {other}");
            }
            None => {
                // If there is no action going on, we'll base the animation on the state of the
                // basis.
                if let Some((_, basis_state)) = controller.concrete_basis::<TnuaBuiltinWalk>() {
                    standing_on_ground = basis_state.standing_on_entity().is_some();
                    desired_velocity = basis_state.running_velocity;
                }
            }
        };

        if keyboard.pressed(KeyCode::Space) {
            wants_to_jump = true;
        }

        if let Some(jump) = &influence.jump_action {
            controller.action(jump.clone());
        }

        // Construct the struct at the end - this will error if any field is missing
        let snapshot = ControllerSnapshot {
            desired_velocity,
            actual_velocity,
            facing_direction,
            standing_on_ground,
            distance_to_ground,
            jump_state,
            wants_to_jump,
        };

        commands.entity(entity).insert(snapshot);
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

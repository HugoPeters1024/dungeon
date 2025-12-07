use std::ops::DerefMut;

use avian3d::math::PI;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::{builtins::TnuaBuiltinJumpState, prelude::*};
use bevy_tnua_avian3d::prelude::*;

use crate::assets::GameAssets;

use crate::game::PlayerCamera;

#[derive(Component, Default)]
#[require(Transform, InheritedVisibility)]
pub struct PlayerRoot;

#[derive(Component, Default, Debug)]
pub struct ControllerSensors {
    pub desired_velocity: Vec3,
    pub actual_velocity: Vec3,
    pub facing_direction: Vec3,
    pub standing_on_ground: bool,
    pub distance_to_ground: f32,
    pub jump_state: Option<TnuaBuiltinJumpState>,
}

#[derive(Component, Debug, Default, Clone)]
pub enum ControllerState {
    #[default]
    Idle,
    Moving,
    Jumping(TnuaBuiltinJump),
    Falling,
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
        Collider::cylinder(0.25, 1.25),
        Mass(400.0),
        Friction::new(0.0),
        TnuaController::default(),
        TnuaAvian3dSensorShape(Collider::cylinder(0.24, 0.1)),
        RayCaster::new(Vec3::new(0.0, 0.0, 0.05), Dir3::NEG_Y),
        ControllerSensors::default(),
        ControllerState::Idle,
        LockedAxes::ROTATION_LOCKED,
    ));
}

pub fn put_in_hand(
    on: On<Add, Name>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    names: Query<&Name>,
) {
    let Ok(name) = names.get(on.entity) else {
        return;
    };

    if name.as_str() != "mixamorigRightHand" {
        return;
    }

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 0.9, 0.9))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.8, 0.1, 0.1),
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(30.0)).with_translation(Vec3::new(0.0, 40.0, 0.0)),
        ChildOf(on.entity),
        Name::new("Sword"),
    ));
}

pub fn controller_update_sensors(
    mut commands: Commands,
    q: Query<(
        Entity,
        &TnuaController,
        &RayHits,
        &Transform,
        &LinearVelocity,
    )>,
) {
    for (entity, controller, hits, transform, velocity) in q.iter() {
        let distance_to_ground = hits.iter_sorted().next().map_or(0.0, |h| h.distance);
        let mut desired_velocity = Vec3::ZERO;
        let actual_velocity = velocity.0;
        let facing_direction = transform.rotation * Vec3::Z;
        let mut standing_on_ground = false;
        let mut jump_state = None;

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

        // Construct the struct at the end - this will error if any field is missing
        let snapshot = ControllerSensors {
            desired_velocity,
            actual_velocity,
            facing_direction,
            standing_on_ground,
            distance_to_ground,
            jump_state,
        };

        commands.entity(entity).insert(snapshot);
    }
}

pub fn update_controller_state(
    mut q: Query<(&mut ControllerState, &ControllerSensors)>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    let jump_action = TnuaBuiltinJump {
        height: 2.5,
        fall_extra_gravity: 10.5,
        ..default()
    };

    for (mut state, sensors) in q.iter_mut() {
        use ControllerState::*;
        match state.deref_mut() {
            Moving => {
                if !sensors.standing_on_ground {
                    *state = Falling;
                }
                if sensors.actual_velocity.length() < 0.1 {
                    *state = Idle;
                }

                if keyboard.just_pressed(KeyCode::Space) {
                    *state = Jumping(jump_action.clone());
                }
            }
            Idle => {
                if sensors.actual_velocity.xz().length() > 0.1 {
                    *state = Moving;
                }

                if !sensors.standing_on_ground {
                    *state = Falling;
                }

                if keyboard.just_pressed(KeyCode::Space) {
                    *state = Jumping(jump_action.clone());
                }
            }
            Jumping(_) => {
                match sensors.jump_state {
                    Some(
                        TnuaBuiltinJumpState::FallSection
                        | TnuaBuiltinJumpState::StoppedMaintainingJump,
                    ) => {
                        *state = Falling;
                    }
                    Some(TnuaBuiltinJumpState::NoJump) => {
                        *state = Idle;
                    }
                    _ => {}
                };
            }
            Falling => {
                if sensors.standing_on_ground {
                    *state = Idle;
                }
            }
        };
    }
}

pub fn apply_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut controller_query: Query<(&mut TnuaController, &ControllerState, &Transform)>,
) {
    let Ok((mut controller, state, transform)) = controller_query.single_mut() else {
        return;
    };

    let forward = transform.rotation * Vec3::Z;
    let sideways = transform.rotation * Vec3::X;
    const FORWARD_SPEED: f32 = 2.0;
    const SIDEWAYS_SPEED: f32 = 2.0;

    let sprint_factor = if keyboard.pressed(KeyCode::ShiftLeft) {
        1.8
    } else {
        1.0
    };

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
        float_height: 0.76,
        max_slope: PI / 3.0,
        acceleration: 30.0,
        spring_strength: 2700.0,
        ..Default::default()
    });

    if let ControllerState::Jumping(jump) = state
        && keyboard.pressed(KeyCode::Space)
    {
        controller.action(jump.clone());
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

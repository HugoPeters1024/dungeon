use std::ops::DerefMut;

use avian3d::math::PI;
use avian3d::prelude::*;
use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use bevy_tnua::{builtins::TnuaBuiltinJumpState, prelude::*};
use bevy_tnua_avian3d::prelude::*;

use crate::assets::GameAssets;
use bevy_hanabi::prelude::*;

use crate::game::{Pickupable, PlayerCamera};

#[derive(Component, Default)]
#[require(Transform, InheritedVisibility)]
pub struct PlayerRoot;

#[derive(PhysicsLayer, Default)]
enum GameLayer {
    #[default]
    Default,
    Player,
}

fn ALL_EXCEPT_PLAYER() -> LayerMask {
    let mut x = LayerMask::ALL;
    x &= !GameLayer::Player.to_bits();
    x
}

#[derive(Component, Default, Debug)]
pub struct ControllerSensors {
    pub actual_velocity: Vec3,
    pub running_velocity: Vec3,
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
    DropKicking(Timer),
}

pub fn on_player_spawn(on: On<Add, PlayerRoot>, mut commands: Commands, assets: Res<GameAssets>) {
    commands.entity(on.event_target()).insert((
        // Spawn at appropriate height: ground is at Y=0.05 (top of 0.1 thick floor)
        // Capsule bottom should be at ground level, so center at 0.05 + 0.8 = 0.85
        Transform::from_xyz(0.0, 0.85, 0.0),
        InheritedVisibility::default(),
        RigidBody::Dynamic,
        //Collider::cuboid(0.1, 0.1, 0.1),
        Friction::new(0.1),
        TnuaController::default(),
        TnuaAvian3dSensorShape(Collider::cylinder(0.20, 0.1)),
        RayCaster::new(Vec3::new(0.0, 0.0, 0.05), Dir3::NEG_Y),
        ControllerSensors::default(),
        ControllerState::Idle,
        LockedAxes::ROTATION_LOCKED,
        children![(
            SceneRoot(assets.player.clone()),
            Transform::from_scale(Vec3::splat(0.008)),
        )],
    ));
}

#[derive(Component)]
pub struct PickupParticleEffect {
    pub spawn_time: f32,
}

pub fn pickup_stuff(
    mut commands: Commands,
    players: Query<Entity, With<PlayerRoot>>,
    children: Query<&Children>,
    colliders: Query<(&CollidingEntities, &Transform)>,
    pickups: Query<(Entity, &Transform), With<Pickupable>>,
    assets: Res<GameAssets>,
    time: Res<Time>,
) {
    for player in players.iter() {
        let mut seen: HashSet<Entity> = HashSet::new();
        for (colliding_entities, _) in children
            .iter_descendants(player)
            .filter_map(|e| colliders.get(e).ok())
        {
            for other in colliding_entities.iter() {
                if let Ok((picked_up, picked_up_transform)) = pickups.get(*other) {
                    // Spawn golden particle effect relative to player position
                    commands.spawn((
                        ParticleEffect {
                            handle: assets.golden_pickup.clone(),
                            prng_seed: Some(time.elapsed().as_micros() as u32),
                        },
                        Transform::from_translation(picked_up_transform.translation),
                        PickupParticleEffect {
                            spawn_time: time.elapsed_secs(),
                        },
                    ));

                    // Despawn the picked up item
                    if !seen.contains(&picked_up) {
                        commands.entity(picked_up).despawn();
                        seen.insert(picked_up);
                    }
                }
            }
        }
    }
}

pub fn cleanup_pickup_particles(
    mut commands: Commands,
    query: Query<(Entity, &PickupParticleEffect)>,
    time: Res<Time>,
) {
    const DURATION: f32 = 2.5; // Despawn after 2.5 seconds (longer for slow fade)

    for (entity, effect) in query.iter() {
        if time.elapsed_secs() - effect.spawn_time > DURATION {
            commands.entity(entity).despawn();
        }
    }
}

pub fn add_mixamo_colliders(on: Query<(Entity, &Name), Added<Name>>, mut commands: Commands) {
    let index: HashMap<&str, (Collider, Transform)> = HashMap::from_iter([
        (
            "mixamorigLeftUpLeg",
            (
                Collider::capsule(15.0, 30.0),
                Transform::from_xyz(0.0, 15.0, 0.0),
            ),
        ),
        (
            "mixamorigRightUpLeg",
            (
                Collider::capsule(15.0, 30.0),
                Transform::from_xyz(0.0, 15.0, 0.0),
            ),
        ),
        (
            "mixamorigLeftLeg",
            (
                Collider::capsule(13.0, 30.0),
                Transform::from_xyz(0.0, 15.0, 0.0),
            ),
        ),
        (
            "mixamorigRightLeg",
            (
                Collider::capsule(13.0, 30.0),
                Transform::from_xyz(0.0, 15.0, 0.0),
            ),
        ),
        (
            "mixamorigHips",
            (Collider::cylinder(30.25, 30.25), Transform::default()),
        ),
        (
            "mixamorigHead",
            (
                Collider::cuboid(30.0, 30.0, 30.0),
                Transform::from_xyz(0.0, 15.0, 0.0),
            ),
        ),
        (
            "mixamorigSpine",
            (
                Collider::cylinder(30.25, 50.25),
                Transform::default(),
            ),
        ),
    ]);

    for (entity, name) in on.iter() {
        if name.as_str().contains("mixamo") {
            warn!("{}", name.as_str());
        }

        if let Some(collider) = index.get(name.as_str()) {
            dbg!(name.as_str());
            commands.entity(entity).with_child((
                collider.clone(),
                CollisionLayers::new(GameLayer::Player, ALL_EXCEPT_PLAYER()),
                CollidingEntities::default(),
            ));
        }
    }
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
        let actual_velocity = velocity.0;
        let facing_direction = transform.rotation * Vec3::Z;
        let mut running_velocity = Vec3::default();
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
                    running_velocity = basis_state.running_velocity;
                }
            }
        };

        // Construct the struct at the end - this will error if any field is missing
        let snapshot = ControllerSensors {
            actual_velocity,
            facing_direction,
            standing_on_ground,
            distance_to_ground,
            jump_state,
            running_velocity,
        };

        commands.entity(entity).insert(snapshot);
    }
}

pub fn update_controller_state(
    mut q: Query<(&mut ControllerState, &ControllerSensors)>,
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    let jump_action = TnuaBuiltinJump {
        height: 4.5,
        fall_extra_gravity: 8.5,
        ..default()
    };

    for (mut state, sensors) in q.iter_mut() {
        use ControllerState::*;
        match state.deref_mut() {
            Moving => {
                if !sensors.standing_on_ground {
                    *state = Falling;
                }
                if sensors.running_velocity.length() < 0.1 {
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

                if keyboard.just_pressed(KeyCode::KeyO) {
                    *state = DropKicking(Timer::from_seconds(2.0, TimerMode::Once));
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
            DropKicking(timer) => {
                timer.tick(time.delta());
                if timer.is_finished() {
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
        float_height: 0.80,
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

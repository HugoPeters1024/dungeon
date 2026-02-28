use std::ops::DerefMut;

use avian3d::math::PI;
use avian3d::prelude::*;
use bevy::{platform::collections::HashSet, prelude::*};
use bevy_tnua::builtins::{TnuaBuiltinJumpConfig, TnuaBuiltinJumpMemory, TnuaBuiltinWalkConfig};
use bevy_tnua::prelude::*;
use bevy_tnua_avian3d::prelude::*;

use crate::assets::GameAssets;
use bevy_hanabi::prelude::*;

use crate::game::Pickupable;

#[derive(TnuaScheme)]
#[scheme(basis = TnuaBuiltinWalk)]
pub enum PlayerControlScheme {
    Jump(TnuaBuiltinJump),
}

#[derive(Component, Default)]
#[require(Transform, InheritedVisibility)]
pub struct PlayerRoot;

#[derive(Component, Default)]
pub struct ControllerCamera;

#[derive(PhysicsLayer, Default)]
enum GameLayer {
    #[default]
    Default,
    Player,
}

fn all_except_player() -> LayerMask {
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
}

#[derive(Component, Debug, Default, Clone)]
pub enum ControllerState {
    #[default]
    Idle,
    Moving,
    Jumping,
    Falling,
    DropKicking(Timer, Timer),
    Attacking(Timer),
}

#[derive(Component)]
pub struct FootRayCaster;

pub fn on_player_spawn(
    on: On<Add, PlayerRoot>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut control_scheme_configs: ResMut<Assets<PlayerControlSchemeConfig>>,
) {
    let control_scheme_config = control_scheme_configs.add(PlayerControlSchemeConfig {
        basis: TnuaBuiltinWalkConfig {
            speed: 2.7,
            float_height: 0.85,
            max_slope: PI / 3.0,
            acceleration: 20.0,
            spring_strength: 700.0,
            ..default()
        },
        jump: TnuaBuiltinJumpConfig {
            height: 2.5,
            fall_extra_gravity: 7.5,
            ..default()
        },
    });

    commands.entity(on.event_target()).insert((
        // Spawn at appropriate height: ground is at Y=0.05 (top of 0.1 thick floor)
        // Capsule bottom should be at ground level, so center at 0.05 + 0.8 = 0.85
        Transform::from_xyz(0.0, 0.85, 0.0),
        InheritedVisibility::default(),
        MassPropertiesBundle::default(),
        RigidBody::Dynamic,
        Friction::new(0.1),
        //Collider::cuboid(0.1, 0.1, 0.1),
        TnuaController::<PlayerControlScheme>::default(),
        TnuaConfig::<PlayerControlScheme>(control_scheme_config),
        TnuaAvian3dSensorShape(Collider::cylinder(0.20, 0.1)),
        RayCaster::new(Vec3::new(0.0, 0.0, 0.05), Dir3::NEG_Y),
        ControllerSensors::default(),
        ControllerState::Idle,
        LockedAxes::ROTATION_LOCKED,
        children![(
            SceneRoot(assets.player.clone()),
            Transform::from_scale(Vec3::splat(0.008)).with_rotation(Quat::from_rotation_y(PI)),
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

pub fn add_mixamo_colliders(
    on: Query<(Entity, &Name), Added<Name>>,
    mut commands: Commands,
    assets: Res<GameAssets>,
) {
    #[rustfmt::skip]
    let index = |name: &str| -> Option<(Collider, Transform)> {
        match name {
            "mixamorigLeftUpLeg" | "mixamorigRightUpLeg" => Some((Collider::capsule(15.0, 30.0), Transform::from_xyz(0.0, 15.0, 0.0))),
            "mixamorigLeftLeg" | "mixamorigRightLeg" => Some((Collider::capsule(13.0, 30.0), Transform::from_xyz(0.0, 15.0, 0.0))),
            "mixamorigHips" => Some((Collider::cylinder(27.25, 30.25), Transform::default())),
            "mixamorigHead" => Some((Collider::sphere(20.0), Transform::from_xyz(0.0, 15.0, 0.0))),
            "mixamorigSpine" => Some((Collider::cylinder(24.25, 50.25), Transform::default())),
            "mixamorigLeftArm" | "mixamorigRightArm" => Some((Collider::capsule(13.0, 30.0), Transform::from_xyz(0.0, 10.0, 0.0))),
            "mixamorigLeftForeArm" | "mixamorigRightForeArm" => Some((Collider::capsule(13.0, 30.0), Transform::from_xyz(0.0, 10.0, 0.0))),
            _ => None,
        }
    };

    for (entity, name) in on.iter() {
        if name.as_str().contains("mixamo") {
            //warn!("{}", name.as_str());
        }

        if let Some(collider) = index(name.as_str()) {
            commands.entity(entity).with_child((
                collider.clone(),
                CollisionLayers::new(GameLayer::Player, all_except_player()),
                CollidingEntities::default(),
            ));
        }

        if name.as_str() == "mixamorigLeftFoot" {
            commands.entity(entity).with_child((
                RayCaster::new(Vec3::new(0.0, 0.0, 0.0), Dir3::Y)
                    .with_max_distance(0.4)
                    .with_query_filter(SpatialQueryFilter::from_mask(all_except_player())),
                FootRayCaster,
            ));
        }

        if name.as_str() == "mixamorigRightHand" {
            commands.entity(entity).with_child((
                SceneRoot(assets.sword.clone()),
                Transform::from_translation(Vec3::new(88.3, 26.9, 0.0))
                    .with_scale(Vec3::splat(40.0))
                    .with_rotation(Quat::from_rotation_z(8.0)),
                Name::new("Sword"),
            ));
        }
    }
}

pub fn controller_update_sensors(
    mut commands: Commands,
    q: Query<(
        Entity,
        &TnuaController<PlayerControlScheme>,
        &RayHits,
        &Transform,
        &LinearVelocity,
    )>,
) {
    for (entity, controller, hits, transform, velocity) in q.iter() {
        let distance_to_ground = hits.iter_sorted().next().map_or(0.0, |h| h.distance);
        let actual_velocity = velocity.0;
        let facing_direction = transform.rotation * Vec3::NEG_Z;
        let standing_on_ground = controller.basis_memory.standing_on_entity().is_some();
        let running_velocity = controller.basis_memory.running_velocity;

        // Construct the struct at the end - this will error if any field is missing
        let snapshot = ControllerSensors {
            actual_velocity,
            facing_direction,
            standing_on_ground,
            distance_to_ground,
            running_velocity,
        };

        commands.entity(entity).insert(snapshot);
    }
}

pub fn update_controller_state(
    mut q: Query<(
        &mut ControllerState,
        &ControllerSensors,
        Forces,
        &TnuaController<PlayerControlScheme>,
    )>,
    caster_and_hit: Single<(&RayCaster, &RayHits), With<FootRayCaster>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    for (mut state, sensors, mut forces, controller) in q.iter_mut() {
        use ControllerState::*;

        if keyboard.just_pressed(KeyCode::KeyO) {
            *state = DropKicking(
                Timer::from_seconds(1.2, TimerMode::Once),
                Timer::from_seconds(2.0, TimerMode::Once),
            );
            continue;
        }

        if keyboard.just_pressed(KeyCode::KeyV) {
            *state = Attacking(Timer::from_seconds(0.9, TimerMode::Once));
            continue;
        }

        let jump_memory = match controller.current_action.as_ref() {
            Some(PlayerControlSchemeActionState::Jump(state)) => Some(&state.memory),
            None => None,
        };
        // A jump action can still be active while falling if jump is held.
        let jump_is_fall_section = matches!(jump_memory, Some(TnuaBuiltinJumpMemory::FallSection));
        let jumping = jump_memory.is_some() && !jump_is_fall_section;
        let grounded = sensors.standing_on_ground;
        let moving = sensors.running_velocity.length() > 0.1;

        match state.deref_mut() {
            Idle => {
                if jumping {
                    *state = Jumping;
                } else if jump_is_fall_section || !grounded {
                    *state = Falling;
                } else if moving {
                    *state = Moving;
                }
            }
            Moving => {
                if jumping {
                    *state = Jumping;
                } else if jump_is_fall_section || !grounded {
                    *state = Falling;
                } else if !moving {
                    *state = Idle;
                }
            }
            Jumping => {
                if jumping {
                    // Stay in Jumping while Tnua jump action is active.
                } else if jump_is_fall_section || !grounded {
                    *state = Falling;
                } else if moving {
                    *state = Moving;
                }
            }
            Falling => {
                if jumping {
                    *state = Jumping;
                } else if grounded && moving {
                    *state = Moving;
                } else if grounded {
                    *state = Idle;
                }
            }
            DropKicking(time_to_force, time_to_complete) => {
                time_to_force.tick(time.delta());
                time_to_complete.tick(time.delta());

                if time_to_force.just_finished() && !caster_and_hit.1.is_empty() {
                    forces.apply_force(200.0 * -caster_and_hit.0.global_direction().as_vec3());
                }

                if time_to_complete.is_finished() {
                    *state = Idle;
                }
            }
            Attacking(timer) => {
                timer.tick(time.delta());
                if timer.just_finished() {
                    *state = Idle;
                }
            }
        }
    }
}

pub fn apply_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut controller_query: Query<(&mut TnuaController<PlayerControlScheme>, &ControllerState)>,
    camera: Single<&Transform, With<ControllerCamera>>,
) {
    let Ok((mut controller, state)) = controller_query.single_mut() else {
        return;
    };

    let forward = (camera.rotation * Vec3::NEG_Z).xz().normalize_or_zero();
    let forward = Vec3::new(forward.x, 0.0, forward.y);
    let sideways = (camera.rotation * Vec3::NEG_X).xz().normalize_or_zero();
    let sideways = Vec3::new(sideways.x, 0.0, sideways.y);

    let sprint_factor = if keyboard.pressed(KeyCode::ShiftLeft) {
        2.0
    } else {
        1.0
    };

    let mut direction = Vec3::ZERO;
    if keyboard.pressed(KeyCode::KeyW) {
        direction += forward;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        direction -= forward;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        direction += sideways;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        direction -= sideways;
    }

    if !matches!(
        state,
        ControllerState::Idle
            | ControllerState::Moving
            | ControllerState::Jumping
            | ControllerState::Falling,
    ) {
        direction = Vec3::ZERO;
    }

    controller.initiate_action_feeding();
    controller.basis = TnuaBuiltinWalk {
        desired_motion: direction.normalize_or_zero() * sprint_factor,
        desired_forward: Dir3::new(direction).ok(),
    };

    if keyboard.pressed(KeyCode::Space)
        && matches!(
            state,
            ControllerState::Idle
                | ControllerState::Moving
                | ControllerState::Jumping
                | ControllerState::Falling
        )
    {
        controller.action(PlayerControlScheme::Jump(TnuaBuiltinJump::default()));
    }
}

/// Rotates the character to always face away from the camera (like Elden Ring)
pub fn rotate_character_to_movement(
    mut query: Query<
        (&mut Transform, &mut ControllerSensors),
        With<TnuaController<PlayerControlScheme>>,
    >,
    time: Res<Time>,
) {
    for (mut transform, sensors) in query.iter_mut() {
        let movement = sensors.running_velocity.normalize_or_zero();
        if movement.length_squared() > 0.1 {
            let target_rotation = Transform::IDENTITY.looking_to(movement, Vec3::Y).rotation;

            // Smoothly rotate character to match target
            const ROTATION_SPEED: f32 = 4.0; // radians per second
            transform.rotation = transform
                .rotation
                .slerp(target_rotation, ROTATION_SPEED * time.delta_secs());
        }
    }
}

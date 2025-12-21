use std::ops::DerefMut;

use avian3d::math::PI;
use avian3d::prelude::*;
use bevy::{platform::collections::HashSet, prelude::*};
use bevy_tnua::{builtins::TnuaBuiltinJumpState, prelude::*};
use bevy_tnua_avian3d::prelude::*;

use crate::assets::GameAssets;
use crate::hud::{GameOver, Vitals};
use crate::talents::{
    ClassSelectUiState, EscapeMenuUiState, SelectedTalentClass, TalentBonuses, TalentClass,
    TalentUiState,
};
use bevy_hanabi::prelude::*;
use bevy_kira_audio::prelude::*;

use crate::game::Pickupable;

#[derive(Component, Default)]
#[require(Transform, InheritedVisibility)]
pub struct PlayerRoot;

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
    pub jump_state: Option<TnuaBuiltinJumpState>,
}

#[derive(Component, Debug, Default, Clone)]
pub enum ControllerState {
    #[default]
    Idle,
    Moving,
    Jumping(TnuaBuiltinJump),
    Falling {
        max_speed: f32,
    },
    DropKicking(Timer, Timer),
}

#[derive(Component)]
pub struct FootRayCaster;

#[derive(Component, Default, Debug, Clone, Copy)]
pub struct AirJumpState {
    pub used: bool,
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
        AirJumpState::default(),
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
    mut vitals: ResMut<Vitals>,
    audio: Res<Audio>,
    class: Res<SelectedTalentClass>,
) {
    for player in players.iter() {
        let mut seen: HashSet<Entity> = HashSet::new();
        for (colliding_entities, _) in children
            .iter_descendants(player)
            .filter_map(|e| colliders.get(e).ok())
        {
            for other in colliding_entities.iter() {
                if let Ok((picked_up, picked_up_transform)) = pickups.get(*other) {
                    // Play pickup sound
                    audio.play(assets.pickup.clone());

                    // Heal based on class
                    let heal_amount = match class.0 {
                        Some(TalentClass::Cleric) => 10.0,
                        Some(TalentClass::Paladin) => 5.0,
                        Some(TalentClass::Bard) => 3.0,
                        None => 2.0,
                    };
                    vitals.health = (vitals.health + heal_amount).min(vitals.max_health);

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
            }
        };

        if let Some((_, basis_state)) = controller.concrete_basis::<TnuaBuiltinWalk>() {
            standing_on_ground = basis_state.standing_on_entity().is_some();
            running_velocity = basis_state.running_velocity;
        }

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

#[allow(clippy::too_many_arguments)]
pub fn update_controller_state(
    mut q: Query<(
        &mut ControllerState,
        &ControllerSensors,
        &mut AirJumpState,
        Forces,
    )>,
    caster_and_hit: Single<(&RayCaster, &RayHits), With<FootRayCaster>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    ui_state: Res<TalentUiState>,
    escape_ui: Res<EscapeMenuUiState>,
    class_select_ui: Res<ClassSelectUiState>,
    bonuses: Res<TalentBonuses>,
    time: Res<Time>,
    mut vitals: ResMut<Vitals>,
    assets: Res<GameAssets>,
    audio: Res<Audio>,
    game_over: Res<GameOver>,
) {
    let jump_action = TnuaBuiltinJump {
        height: 2.5 * bonuses.jump_height_mult,
        fall_extra_gravity: 3.5 * bonuses.fall_extra_gravity_mult,
        ..default()
    };

    let blocked = ui_state.open || escape_ui.open || class_select_ui.open || game_over.0;

    for (mut state, sensors, mut air_jump, mut forces) in q.iter_mut() {
        use ControllerState::*;

        // Reset air-jump when we touch ground.
        if sensors.standing_on_ground {
            air_jump.used = false;
        }

        // Mid-air jump (double jump) from talent.
        if !blocked
            && !sensors.standing_on_ground
            && bonuses.extra_air_jumps > 0
            && !air_jump.used
            && keyboard.just_pressed(KeyCode::Space)
        {
            air_jump.used = true;

            // Apply an instant upward impulse so the jump always happens even if Tnua jump
            // action refuses to trigger while airborne.
            //
            // Tune: this gives a nice, snappy mid-air jump without being a full ground jump.
            const AIR_JUMP_IMPULSE: f32 = 3.6;
            forces.apply_linear_impulse(Vec3::Y * AIR_JUMP_IMPULSE);

            *state = Jumping(jump_action.clone());
        }

        match state.deref_mut() {
            Moving => {
                if !sensors.standing_on_ground {
                    *state = Falling { max_speed: 0.0 };
                }
                if sensors.running_velocity.length() < 0.1 {
                    *state = Idle;
                }

                if !blocked && keyboard.just_pressed(KeyCode::Space) {
                    *state = Jumping(jump_action.clone());
                }

                if !blocked && keyboard.just_pressed(KeyCode::KeyO) {
                    *state = DropKicking(
                        Timer::from_seconds(1.2, TimerMode::Once),
                        Timer::from_seconds(2.0, TimerMode::Once),
                    );
                }
            }
            Idle => {
                if sensors.actual_velocity.xz().length() > 0.1 {
                    *state = Moving;
                }

                if !sensors.standing_on_ground {
                    *state = Falling { max_speed: 0.0 };
                }

                if !blocked && keyboard.just_pressed(KeyCode::Space) {
                    *state = Jumping(jump_action.clone());
                }

                if !blocked && keyboard.just_pressed(KeyCode::KeyO) {
                    *state = DropKicking(
                        Timer::from_seconds(1.2, TimerMode::Once),
                        Timer::from_seconds(2.0, TimerMode::Once),
                    );
                }
            }
            Jumping(_) => {
                match sensors.jump_state {
                    Some(
                        TnuaBuiltinJumpState::FallSection
                        | TnuaBuiltinJumpState::StoppedMaintainingJump,
                    ) => {
                        *state = Falling { max_speed: 0.0 };
                    }
                    Some(TnuaBuiltinJumpState::NoJump) => {
                        *state = Idle;
                    }
                    _ => {}
                };
            }
            Falling { max_speed } => {
                *max_speed = max_speed.max(sensors.actual_velocity.y.abs());

                if sensors.standing_on_ground {
                    if *max_speed > 10.0 {
                        let damage = (*max_speed - 10.0) * 5.0 * bonuses.fall_damage_mult;
                        vitals.health = (vitals.health - damage).max(0.0);
                        audio.play(assets.fall.clone());
                    } else if *max_speed > 2.0 {
                        // Play sound even for small falls, but no damage
                        audio.play(assets.fall.clone());
                    }
                    *state = Idle;
                }
            }
            DropKicking(time_to_force, time_to_complete) => {
                time_to_force.tick(time.delta());
                time_to_complete.tick(time.delta());

                if time_to_force.just_finished() && !caster_and_hit.1.is_empty() {
                    dbg!(-caster_and_hit.0.global_direction());
                    forces.apply_force(200.0 * -caster_and_hit.0.global_direction().as_vec3());
                }

                if time_to_complete.is_finished() {
                    *state = Idle;
                }
            }
        };
    }
}

pub fn apply_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut controller_query: Query<(&mut TnuaController, &ControllerState)>,
    camera: Single<&Transform, With<Camera>>,
    ui_state: Res<TalentUiState>,
    escape_ui: Res<EscapeMenuUiState>,
    class_select_ui: Res<ClassSelectUiState>,
    bonuses: Res<TalentBonuses>,
    game_over: Res<GameOver>,
) {
    let Ok((mut controller, state)) = controller_query.single_mut() else {
        return;
    };

    let forward = (camera.rotation * Vec3::NEG_Z).xz().normalize_or_zero();
    let forward = Vec3::new(forward.x, 0.0, forward.y);
    let sideways = (camera.rotation * Vec3::NEG_X).xz().normalize_or_zero();
    let sideways = Vec3::new(sideways.x, 0.0, sideways.y);
    const BASE_SPEED: f32 = 2.0;

    let sprint_factor = if keyboard.pressed(KeyCode::ShiftLeft) {
        2.0
    } else {
        1.0
    };
    let sprint_factor = sprint_factor * bonuses.sprint_mult;

    let blocked = ui_state.open || escape_ui.open || class_select_ui.open || game_over.0;

    let mut direction = Vec3::ZERO;
    if !blocked && keyboard.pressed(KeyCode::KeyW) {
        direction += forward;
    }
    if !blocked && keyboard.pressed(KeyCode::KeyS) {
        direction -= forward;
    }
    if !blocked && keyboard.pressed(KeyCode::KeyA) {
        direction += sideways;
    }
    if !blocked && keyboard.pressed(KeyCode::KeyD) {
        direction -= sideways;
    }

    // Feed the basis every frame. Even if the player doesn't move - just use `desired_velocity:
    // Vec3::ZERO`. `TnuaController` starts without a basis, which will make the character collider
    // just fall.
    controller.basis(TnuaBuiltinWalk {
        // The `desired_velocity` determines how the character will move.
        desired_velocity: direction.normalize_or_zero()
            * BASE_SPEED
            * bonuses.move_speed_mult
            * sprint_factor,
        // The `float_height` must be greater (even if by little) from the distance between the
        // character's center and the lowest point of its collider.
        float_height: 0.85,
        max_slope: PI / 3.0,
        acceleration: 30.0,
        spring_strength: 2700.0,
        ..Default::default()
    });

    if !blocked
        && let ControllerState::Jumping(jump) = state
        && keyboard.pressed(KeyCode::Space)
    {
        controller.action(jump.clone());
    }
}

/// Rotates the character to always face away from the camera (like Elden Ring)
pub fn rotate_character_to_movement(
    mut query: Query<(&mut Transform, &mut ControllerSensors), With<TnuaController>>,
    time: Res<Time>,
) {
    for (mut transform, sensors) in query.iter_mut() {
        if sensors.running_velocity.length() > 0.1 {
            let target_rotation = Quat::from_rotation_y(
                PI - sensors
                    .running_velocity
                    .x
                    .atan2(-sensors.running_velocity.z),
            );

            // Smoothly rotate character to match target
            const ROTATION_SPEED: f32 = 4.0; // radians per second
            transform.rotation = transform
                .rotation
                .slerp(target_rotation, ROTATION_SPEED * time.delta_secs());
        }
    }
}

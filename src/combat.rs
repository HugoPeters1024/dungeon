use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::HashMap;

use crate::assets::MyStates;
use crate::camera::ThirdPersonCamera;

#[derive(Component, Debug, Clone, Copy)]
pub struct Damageable {
    pub hp: f32,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct DamageDealtEvent {
    pub target: Entity,
    pub pos: Vec3,
    pub amount: f32,
}

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DamageDealtEvent>()
            .init_resource::<DamageNumberBuckets>()
            .add_systems(OnEnter(MyStates::Next), spawn_damage_numbers_root)
            .add_systems(
                Update,
                (
                    handle_damage_numbers,
                    tick_damage_numbers,
                    cleanup_dead_damageables,
                )
                    .run_if(in_state(MyStates::Next)),
            );
    }
}

fn cleanup_dead_damageables(mut commands: Commands, q: Query<(Entity, &Damageable)>) {
    for (e, d) in q.iter() {
        if d.hp <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

#[derive(Component)]
struct DamageNumbersRoot;

#[derive(Component)]
struct DamageNumber {
    t: f32,
    lifetime: f32,
    vel: Vec3,
    world_pos: Vec3,
    text_entity: Entity,
}

#[derive(Default, Resource)]
struct DamageNumberBuckets {
    by_target: HashMap<Entity, DamageBucket>,
}

#[derive(Clone, Copy)]
struct DamageBucket {
    pos: Vec3,
    accum: f32,
    since_last: f32,
}

fn handle_damage_numbers(
    mut commands: Commands,
    time: Res<Time>,
    mut buckets: ResMut<DamageNumberBuckets>,
    mut ev: MessageReader<DamageDealtEvent>,
) {
    let dt = time.delta_secs();

    // Tick bucket timers.
    for b in buckets.by_target.values_mut() {
        b.since_last += dt;
    }

    for e in ev.read() {
        // Big hits: show immediately.
        if e.amount >= 5.0 {
            spawn_damage_number(&mut commands, e.pos, e.amount, true);
            continue;
        }

        // Small hits (DOT): accumulate and show periodically.
        buckets
            .by_target
            .entry(e.target)
            .and_modify(|b| {
                b.pos = e.pos;
                b.accum += e.amount;
            })
            .or_insert(DamageBucket {
                pos: e.pos,
                accum: e.amount,
                since_last: 0.0,
            });
    }

    // Flush DOT buckets.
    // - show at most 4 times/sec per target
    // - only show if the rounded amount would be >= 1
    const FLUSH_INTERVAL: f32 = 0.25;
    let mut to_clear: Vec<Entity> = Vec::new();
    for (&target, b) in buckets.by_target.iter_mut() {
        if b.since_last >= FLUSH_INTERVAL {
            let shown = b.accum.round();
            if shown >= 1.0 {
                spawn_damage_number(&mut commands, b.pos, shown, false);
            }
            b.accum = 0.0;
            b.since_last = 0.0;
        }

        // Keep map small.
        if b.accum <= 0.0 && b.since_last > 1.0 {
            to_clear.push(target);
        }
    }
    for t in to_clear {
        buckets.by_target.remove(&t);
    }
}

fn spawn_damage_number(commands: &mut Commands, pos: Vec3, amount: f32, big: bool) {
    let text = format!("{}", amount.round() as i32);
    let base = if big { 26.0 } else { 20.0 };
    let color = if big {
        Color::srgba(1.0, 0.85, 0.25, 1.0)
    } else {
        Color::srgba(0.95, 0.95, 0.95, 1.0)
    };

    // Nudge up above the target.
    let p = pos + Vec3::Y * 1.6;

    // Spawn under the UI root so this always renders (independent of 3D/2D camera rendering quirks).
    let root = commands
        .spawn((
            Name::new("Damage Number"),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            GlobalZIndex(5000),
        ))
        .id();

    let text_entity = commands
        .spawn((
            Name::new("Damage Number Text"),
            Text::new(text),
            TextFont {
                font_size: base,
                ..default()
            },
            TextColor(color),
            TextShadow::default(),
        ))
        .id();

    commands.entity(root).add_child(text_entity);

    commands.entity(root).insert(DamageNumber {
        t: 0.0,
        lifetime: 0.85,
        vel: Vec3::new(0.0, 1.7, 0.0),
        world_pos: p,
        text_entity,
    });

    // Note: we intentionally spawn these as top-level UI nodes. They're cheap, and this avoids
    // fighting UI parenting while still rendering correctly.
}

#[allow(clippy::type_complexity)]
fn tick_damage_numbers(
    mut commands: Commands,
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), (With<Camera3d>, With<ThirdPersonCamera>)>,
    mut nodes: Query<&mut Node>,
    mut colors: Query<&mut TextColor>,
    mut q: Query<(Entity, &mut DamageNumber)>,
) {
    let dt = time.delta_secs();
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, cam_gt)) = camera_q.single() else {
        return;
    };

    let scale = window.scale_factor();
    let win_h = window.physical_height() as f32 / scale;

    for (e, mut n) in q.iter_mut() {
        n.t += dt;
        let vel = n.vel;
        n.world_pos += vel * dt;

        let a = (1.0 - (n.t / n.lifetime)).clamp(0.0, 1.0);
        if let Ok(mut color) = colors.get_mut(n.text_entity) {
            color.0 = color.0.with_alpha(a);
        }

        // Project to screen space and place the UI node.
        if let Ok(p) = camera.world_to_viewport(cam_gt, n.world_pos) {
            let x = p.x / scale;
            let y_from_top = (win_h - (p.y / scale)).max(0.0);

            if let Ok(mut node) = nodes.get_mut(e) {
                node.left = Val::Px(x);
                node.top = Val::Px(y_from_top);
            }
        }

        if n.t >= n.lifetime {
            commands.entity(e).despawn();
        }
    }
}

fn spawn_damage_numbers_root(mut commands: Commands) {
    // A fixed overlay container for combat text.
    commands.spawn((
        DamageNumbersRoot,
        Name::new("Damage Numbers Root"),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        GlobalZIndex(5000),
    ));
}

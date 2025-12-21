use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::HashMap;

use crate::assets::{GameAssets, MyStates};
use crate::camera::ThirdPersonCamera;
use crate::combat::{DamageDealtEvent, Damageable};
use crate::player::controller::{ControllerSensors, PlayerRoot};
use crate::spells::{
    DamageElement, SPELL_SLOTS, SpellBar, SpellDef, SpellEffect, spellbar_for_class,
};
use crate::talents::{InfiniteMana, SelectedTalentClass, TalentBonuses, TalentClass};
use avian3d::prelude::{Forces, RigidBodyForces, SpatialQuery, SpatialQueryFilter};
use bevy_hanabi::prelude::ParticleEffect;

pub struct HudPlugin;

const ACTION_KEYS: [&str; SPELL_SLOTS] = ["1", "2", "3", "4", "5", "Q", "E", "R"];
const ACTION_SLOTS: usize = SPELL_SLOTS;
const ACTION_BINDS: [KeyCode; 8] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::KeyQ,
    KeyCode::KeyE,
    KeyCode::KeyR,
];
const ICON_ATLAS_PATH: &str = "icons.png";

fn slot_for_bind(bind: KeyCode) -> Option<usize> {
    ACTION_BINDS.iter().position(|&b| b == bind)
}

#[derive(Resource, Clone, Copy)]
struct ActiveSpellBar(SpellBar);

impl Default for ActiveSpellBar {
    fn default() -> Self {
        Self(spellbar_for_class(TalentClass::Paladin))
    }
}

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Vitals>()
            .init_resource::<SkillCastRng>()
            .init_resource::<ActiveSpellBar>()
            .add_systems(OnEnter(MyStates::Next), spawn_hud)
            .add_systems(
                Update,
                (
                    regen_mana,
                    sync_spellbar_from_class,
                    handle_action_bar_casts,
                    tick_elemental_orbs,
                    update_damage_pools_surface,
                    tick_damage_pools,
                    animate_action_cast_fx,
                    update_hud_from_vitals,
                    animate_action_bar_slots,
                    swap_action_icons_from_atlas,
                )
                    .chain()
                    .run_if(in_state(MyStates::Next)),
            );
    }
}

#[derive(Component)]
struct DamagePoolFx {
    dps: f32,
    radius: f32,
    remaining: f32,
    element: DamageElement,
}

fn sync_spellbar_from_class(
    selected: Option<Res<SelectedTalentClass>>,
    mut bar: ResMut<ActiveSpellBar>,
) {
    if selected.as_ref().is_some_and(|s| !s.is_changed()) {
        return;
    }
    let class = selected
        .as_ref()
        .and_then(|s| s.0)
        .unwrap_or(TalentClass::Paladin);
    bar.0 = spellbar_for_class(class);
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct Vitals {
    pub health: f32,
    pub max_health: f32,
    pub mana: f32,
    pub max_mana: f32,
}

impl Default for Vitals {
    fn default() -> Self {
        Self {
            health: 85.0,
            max_health: 100.0,
            mana: 42.0,
            max_mana: 100.0,
        }
    }
}

fn regen_mana(
    time: Res<Time>,
    bonuses: Res<TalentBonuses>,
    infinite: Option<Res<InfiniteMana>>,
    mut vitals: ResMut<Vitals>,
) {
    if infinite.as_ref().is_some_and(|i| i.0) {
        vitals.mana = vitals.max_mana;
        return;
    }
    // Placeholder base regen until spells/abilities exist.
    const BASE_MANA_REGEN_PER_SEC: f32 = 2.0;
    let regen = BASE_MANA_REGEN_PER_SEC * bonuses.mana_regen_mult;
    vitals.mana = (vitals.mana + regen * time.delta_secs()).min(vitals.max_mana);
}

#[derive(Component)]
struct HudRoot;

#[derive(Component, Clone, Copy)]
enum OrbKind {
    Health,
    Mana,
}

#[derive(Component)]
struct OrbFill(OrbKind);

#[derive(Component)]
struct OrbText(OrbKind);

#[derive(Component)]
struct OrbFrame;

#[derive(Component)]
struct OrbGloss;

#[derive(Component)]
struct HudBar;

#[derive(Component)]
struct ActionBarRoot;

#[derive(Component)]
struct ActionSlot;

#[derive(Component, Clone, Copy)]
struct ActionSlotBind(KeyCode);

#[derive(Component)]
struct ActionSlotIcon;

#[derive(Component, Clone, Copy)]
struct ActionSlotIconIndex(usize);

#[derive(Component)]
struct ActionSlotFrame;

#[derive(Component)]
struct ActionSlotGloss;

#[derive(Component)]
struct ActionSlotCooldown;

#[derive(Component)]
struct ActionSlotKeyText;

#[derive(Component)]
struct ActionCastFx {
    t: f32,
    success: bool,
}

#[derive(Resource, Default)]
struct HudImages {
    hp_fill: Handle<Image>,
    mp_fill: Handle<Image>,
    frame: Handle<Image>,
    gloss: Handle<Image>,
    bar: Handle<Image>,
    slot_frame: Handle<Image>,
    slot_gloss: Handle<Image>,
    spell_icons: Vec<Handle<Image>>,
}

#[derive(Resource, Default)]
struct IconAtlasState {
    source: Handle<Image>,
    built: bool,
    cols: Vec<(u32, u32)>,
    rows: Vec<(u32, u32)>,
    icons_by_class: HashMap<TalentClass, Vec<Handle<Image>>>,
    last_applied: Option<TalentClass>,
}

#[derive(Resource, Debug, Clone, Copy)]
struct SkillCastRng(u32);

impl Default for SkillCastRng {
    fn default() -> Self {
        Self(0xC0FFEE_u32)
    }
}

fn spawn_hud(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    // Higher-res textures look much nicer after UI scaling.
    const TEX: u32 = 512;
    let hud_images = HudImages {
        frame: images.add(make_orb_frame_image(TEX)),
        gloss: images.add(make_orb_gloss_image(TEX)),
        hp_fill: images.add(make_orb_fill_image(TEX, Color::srgb(0.78, 0.08, 0.12))),
        mp_fill: images.add(make_orb_fill_image(TEX, Color::srgb(0.10, 0.30, 0.86))),
        bar: images.add(make_hud_bar_image(1024, 160)),
        slot_frame: images.add(make_slot_frame_image(128)),
        slot_gloss: images.add(make_slot_gloss_image(128)),
        spell_icons: (0..ACTION_SLOTS)
            .map(|i| images.add(make_spell_icon_image(96, i as u32)))
            .collect(),
    };

    // Root overlay (non-interactive).
    let root = commands
        .spawn((
            HudRoot,
            Name::new("HUD Root"),
            GlobalZIndex(10),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
        ))
        .id();

    // Bottom bar behind the orbs (Diablo-ish anchor).
    commands.entity(root).with_child((
        HudBar,
        Name::new("HUD Bar"),
        GlobalZIndex(9),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            bottom: Val::Px(0.0),
            height: Val::Px(140.0),
            ..default()
        },
        ImageNode::default(),
    ));

    // Action bar (WoW-ish) — centered, on top of the bottom bar.
    // Note: this uses original/procedural icons (not WoW assets).
    let action_root = commands
        .spawn((
            ActionBarRoot,
            Name::new("Action Bar Root"),
            GlobalZIndex(12),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(18.0),
                height: Val::Px(72.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .id();
    commands.entity(root).add_child(action_root);

    let slots_row = commands
        .spawn((
            Name::new("Action Bar Slots Row"),
            Node {
                width: Val::Auto,
                height: Val::Px(72.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            },
        ))
        .id();
    commands.entity(action_root).add_child(slots_row);

    for (i, key_label) in ACTION_KEYS.iter().enumerate() {
        let slot = commands
            .spawn((
                ActionSlot,
                Button,
                ActionSlotBind(ACTION_BINDS[i]),
                Name::new(format!("Action Slot {i}")),
                Node {
                    width: Val::Px(56.0),
                    height: Val::Px(56.0),
                    position_type: PositionType::Relative,
                    ..default()
                },
                Transform::default(),
            ))
            .id();

        let icon = commands
            .spawn((
                ActionSlotIcon,
                ActionSlotIconIndex(i),
                Name::new("Slot Icon"),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(2.0),
                    top: Val::Px(2.0),
                    width: Val::Px(52.0),
                    height: Val::Px(52.0),
                    ..default()
                },
                ImageNode::new(hud_images.spell_icons[i].clone()),
            ))
            .id();

        let cd = commands
            .spawn((
                ActionSlotCooldown,
                Name::new("Slot Cooldown Wipe"),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(2.0),
                    right: Val::Px(2.0),
                    top: Val::Px(2.0),
                    height: Val::Percent(0.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            ))
            .id();

        let frame = commands
            .spawn((
                ActionSlotFrame,
                Name::new("Slot Frame"),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ImageNode::new(hud_images.slot_frame.clone()),
            ))
            .id();

        let gloss = commands
            .spawn((
                ActionSlotGloss,
                Name::new("Slot Gloss"),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ImageNode::new(hud_images.slot_gloss.clone()),
            ))
            .id();

        let key = commands
            .spawn((
                ActionSlotKeyText,
                Name::new("Slot Key Text"),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(6.0),
                    bottom: Val::Px(4.0),
                    ..default()
                },
                children![(
                    Text::new(*key_label),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.95, 0.92, 0.86, 0.85)),
                )],
            ))
            .id();

        commands.entity(slot).add_child(icon);
        commands.entity(slot).add_child(cd);
        commands.entity(slot).add_child(frame);
        commands.entity(slot).add_child(gloss);
        commands.entity(slot).add_child(key);
        commands.entity(slots_row).add_child(slot);
    }

    let hp_orb = spawn_orb(&mut commands, OrbKind::Health, Some(22.0), None);
    let mp_orb = spawn_orb(&mut commands, OrbKind::Mana, None, Some(22.0));

    commands.entity(root).add_child(hp_orb);
    commands.entity(root).add_child(mp_orb);

    // Insert images resource after building HUD so we can still use the handles above.
    commands.insert_resource(hud_images);

    // Start loading the real icon atlas; we'll slice it once Bevy has decoded it.
    // We keep procedural icons as a fallback/placeholder until then.
    let source = asset_server.load::<Image>(ICON_ATLAS_PATH);
    commands.insert_resource(IconAtlasState {
        source,
        built: false,
        cols: Vec::new(),
        rows: Vec::new(),
        icons_by_class: HashMap::new(),
        last_applied: None,
    });
}

fn swap_action_icons_from_atlas(
    mut atlas: ResMut<IconAtlasState>,
    mut images: ResMut<Assets<Image>>,
    selected: Option<Res<SelectedTalentClass>>,
    bar: Res<ActiveSpellBar>,
    mut icon_nodes: Query<(&ActionSlotIconIndex, &mut ImageNode), With<ActionSlotIcon>>,
) {
    let class = selected
        .as_ref()
        .and_then(|s| s.0)
        .unwrap_or(TalentClass::Paladin);

    // Build icons once the atlas is loaded.
    if !atlas.built {
        let Some(src) = images.get(&atlas.source).cloned() else {
            return; // not loaded yet
        };
        let Some((cols, rows)) = detect_icon_grid(&src) else {
            return; // couldn't detect; keep procedural
        };
        let total = cols.len() * rows.len();
        if total == 0 {
            return;
        }

        atlas.cols = cols;
        atlas.rows = rows;
        atlas.icons_by_class.clear();

        for c in TalentClass::ALL {
            let spells = spellbar_for_class(c);
            let mut out: Vec<Handle<Image>> = Vec::with_capacity(ACTION_SLOTS);
            for s in spells {
                let idx = s.icon_index % total;
                if let Some(icon_img) = extract_icon(&src, &atlas.cols, &atlas.rows, idx) {
                    out.push(images.add(icon_img));
                }
            }
            if out.len() == ACTION_SLOTS {
                atlas.icons_by_class.insert(c, out);
            }
        }

        if atlas.icons_by_class.len() != TalentClass::ALL.len() {
            return;
        }
        atlas.built = true;
        atlas.last_applied = None;
    }

    // Only re-apply when the selected class changes.
    if atlas.last_applied == Some(class) && !selected.as_ref().is_some_and(|s| s.is_changed()) {
        return;
    }
    atlas.last_applied = Some(class);

    let Some(icon_list) = atlas.icons_by_class.get(&class) else {
        return;
    };
    for (idx, mut node) in icon_nodes.iter_mut() {
        if let Some(h) = icon_list.get(idx.0).cloned() {
            node.image = h;
        }
    }

    // Ensure we don't show stale icons if atlas isn't ready: if the atlas isn't built yet,
    // spellbar icons remain the procedural placeholders. Once built, icons match the class spellbar.
    let _ = bar;
}

#[allow(clippy::type_complexity)]
fn detect_icon_grid(image: &Image) -> Option<(Vec<(u32, u32)>, Vec<(u32, u32)>)> {
    // Use near-black separator lines to infer cell boundaries.
    let w = image.size().x;
    let h = image.size().y;
    let fmt = image.texture_descriptor.format;
    let bpp = match fmt {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => 4,
        _ => return None,
    };
    let data = image.data.as_ref()?;
    if data.len() < (w as usize * h as usize * bpp) {
        return None;
    }

    let mut col_black = vec![0u32; w as usize];
    let mut row_black = vec![0u32; h as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) as usize) * bpp;
            let r = data[i];
            let g = data[i + 1];
            let b = data[i + 2];
            if r < 8 && g < 8 && b < 8 {
                col_black[x as usize] += 1;
                row_black[y as usize] += 1;
            }
        }
    }

    let cols = find_separators(&col_black, h, 0.55);
    let rows = find_separators(&row_black, w, 0.55);
    if cols.len() < 2 || rows.len() < 2 {
        return None;
    }

    let col_cells = runs_to_cells(&cols)?;
    let row_cells = runs_to_cells(&rows)?;
    Some((col_cells, row_cells))
}

fn find_separators(counts: &[u32], len_other_axis: u32, threshold: f32) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    let mut in_run = false;
    let mut start = 0u32;
    for (i, &c) in counts.iter().enumerate() {
        let frac = c as f32 / len_other_axis as f32;
        let is_sep = frac >= threshold;
        if is_sep && !in_run {
            in_run = true;
            start = i as u32;
        } else if !is_sep && in_run {
            in_run = false;
            out.push((start, i as u32 - 1));
        }
    }
    if in_run {
        out.push((start, counts.len() as u32 - 1));
    }
    out
}

fn runs_to_cells(runs: &[(u32, u32)]) -> Option<Vec<(u32, u32)>> {
    let mut cells = Vec::new();
    for w in runs.windows(2) {
        let a = w[0];
        let b = w[1];
        let x0 = a.1.saturating_add(1);
        let x1 = b.0.saturating_sub(1);
        if x1 > x0 + 8 {
            cells.push((x0, x1));
        }
    }
    if cells.is_empty() { None } else { Some(cells) }
}

fn extract_icon(
    image: &Image,
    cols: &[(u32, u32)],
    rows: &[(u32, u32)],
    idx: usize,
) -> Option<Image> {
    let w = image.size().x;
    let fmt = image.texture_descriptor.format;
    let bpp = match fmt {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => 4,
        _ => return None,
    };
    let data = image.data.as_ref()?;
    let cols_n = cols.len();
    let rows_n = rows.len();
    if cols_n == 0 || rows_n == 0 {
        return None;
    }
    let row = idx / cols_n;
    let col = idx % cols_n;
    if row >= rows_n {
        return None;
    }
    let (x0, x1) = cols[col];
    let (y0, y1) = rows[row];
    let tw = x1 - x0 + 1;
    let th = y1 - y0 + 1;

    let mut out = vec![0u8; (tw * th * 4) as usize];
    for oy in 0..th {
        let sy = y0 + oy;
        for ox in 0..tw {
            let sx = x0 + ox;
            let si = ((sy * w + sx) as usize) * bpp;
            let di = ((oy * tw + ox) as usize) * 4;
            out[di] = data[si];
            out[di + 1] = data[si + 1];
            out[di + 2] = data[si + 2];
            out[di + 3] = data[si + 3];
        }
    }

    Some(Image::new(
        Extent3d {
            width: tw,
            height: th,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        out,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    ))
}

fn animate_action_bar_slots(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    interactions: Query<(&ActionSlotBind, &Interaction), With<ActionSlot>>,
    mut slots: Query<(&ActionSlotBind, &mut Transform), With<ActionSlot>>,
    mut anims: Local<HashMap<KeyCode, f32>>,
) {
    // Trigger animation on keyboard press.
    for (bind, _) in slots.iter() {
        if keyboard.just_pressed(bind.0) {
            anims.insert(bind.0, 0.0);
        }
    }

    // Trigger animation on mouse press.
    for (bind, interaction) in interactions.iter() {
        if *interaction == Interaction::Pressed {
            anims.insert(bind.0, 0.0);
        }
    }

    // Update animation: we do it purely from Transform so it always resolves back.
    const PRESS_DUR: f32 = 0.06;
    const RELEASE_DUR: f32 = 0.10;
    const TOTAL: f32 = PRESS_DUR + RELEASE_DUR;

    // Advance timers
    let dt = time.delta_secs();
    anims.retain(|_, t| {
        *t += dt;
        *t < TOTAL
    });

    for (bind, mut tf) in slots.iter_mut() {
        let Some(&t) = anims.get(&bind.0) else {
            tf.scale = Vec3::ONE;
            tf.translation.y = 0.0;
            continue;
        };

        let (scale, y) = if t < PRESS_DUR {
            let p = (t / PRESS_DUR).clamp(0.0, 1.0);
            // ease-out
            let e = 1.0 - (1.0 - p) * (1.0 - p);
            (1.0 - 0.08 * e, -2.0 * e)
        } else {
            let p = ((t - PRESS_DUR) / RELEASE_DUR).clamp(0.0, 1.0);
            // little overshoot on the way back
            let overshoot = (p * std::f32::consts::PI).sin() * (1.0 - p) * 0.06;
            let s = 0.92 + 0.08 * p + overshoot;
            let y = -2.0 * (1.0 - p);
            (s, y)
        };

        tf.scale = Vec3::splat(scale);
        tf.translation.y = y;
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
fn handle_action_bar_casts(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut rng: ResMut<SkillCastRng>,
    mut vitals: ResMut<Vitals>,
    mut player: Query<(Forces, Option<&ControllerSensors>), With<PlayerRoot>>,
    player_tf: Query<&GlobalTransform, With<PlayerRoot>>,
    camera_tf: Query<&GlobalTransform, (With<Camera3d>, With<ThirdPersonCamera>)>,
    spatial_query: SpatialQuery,
    mut damageables: Query<(Entity, &GlobalTransform, &mut Damageable)>,
    assets: Res<GameAssets>,
    infinite: Option<Res<InfiniteMana>>,
    bar: Res<ActiveSpellBar>,
    slots: Query<(Entity, &ActionSlotBind), With<ActionSlot>>,
    clicks: Query<
        (Entity, &Interaction, &ActionSlotBind),
        (With<ActionSlot>, Changed<Interaction>),
    >,
) {
    let spells = bar.0;
    let infinite = infinite.as_ref().is_some_and(|i| i.0);

    // Keyboard activations
    for (entity, bind) in slots.iter() {
        if keyboard.just_pressed(bind.0) {
            let Some(slot) = slot_for_bind(bind.0) else {
                continue;
            };
            try_cast(
                &mut commands,
                &mut rng,
                &mut vitals,
                &mut player,
                &player_tf,
                &camera_tf,
                &spatial_query,
                &mut damageables,
                &assets,
                entity,
                spells[slot],
                infinite,
            );
        }
    }

    // Mouse click activations
    for (entity, interaction, bind) in clicks.iter() {
        if *interaction == Interaction::Pressed {
            let Some(slot) = slot_for_bind(bind.0) else {
                continue;
            };
            try_cast(
                &mut commands,
                &mut rng,
                &mut vitals,
                &mut player,
                &player_tf,
                &camera_tf,
                &spatial_query,
                &mut damageables,
                &assets,
                entity,
                spells[slot],
                infinite,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn try_cast(
    commands: &mut Commands,
    rng: &mut SkillCastRng,
    vitals: &mut Vitals,
    player: &mut Query<(Forces, Option<&ControllerSensors>), With<PlayerRoot>>,
    player_tf: &Query<&GlobalTransform, With<PlayerRoot>>,
    camera_tf: &Query<&GlobalTransform, (With<Camera3d>, With<ThirdPersonCamera>)>,
    spatial_query: &SpatialQuery,
    damageables: &mut Query<(Entity, &GlobalTransform, &mut Damageable)>,
    assets: &GameAssets,
    slot_entity: Entity,
    spell: SpellDef,
    infinite: bool,
) {
    // Deterministic "randomness" reserved for later (crit/variation etc).
    rng.0 = rng.0.wrapping_mul(1664525).wrapping_add(1013904223);

    // Use floor(mana) for both display + gating so players never see "20" but can't cast 20.
    // If infinite mana is enabled, always succeed and don't spend.
    let available = vitals.mana.max(0.0).floor() as u32;
    let success = infinite || available >= spell.mana_cost;
    if success {
        if !infinite {
            vitals.mana = (vitals.mana - spell.mana_cost as f32).max(0.0);
        }
        apply_spell_effect(vitals, player, spell.effect);
        apply_damage_spell_effect(
            commands,
            player,
            player_tf,
            camera_tf,
            spatial_query,
            damageables,
            assets,
            spell.effect,
        );
    }
    spawn_cast_fx(commands, slot_entity, success);
}

#[allow(clippy::too_many_arguments)]
fn apply_damage_spell_effect(
    commands: &mut Commands,
    _player: &mut Query<(Forces, Option<&ControllerSensors>), With<PlayerRoot>>,
    player_tf: &Query<&GlobalTransform, With<PlayerRoot>>,
    camera_tf: &Query<&GlobalTransform, (With<Camera3d>, With<ThirdPersonCamera>)>,
    spatial_query: &SpatialQuery,
    _damageables: &mut Query<(Entity, &GlobalTransform, &mut Damageable)>,
    assets: &GameAssets,
    effect: SpellEffect,
) {
    let Ok(gt) = player_tf.single() else {
        return;
    };
    let origin = gt.translation();
    let cam = camera_tf.single().ok();

    match effect {
        SpellEffect::ElementalBlast {
            damage,
            radius,
            range: _,
            element,
        } => {
            // True projectile:
            // - Spawn from just in front of the player (so it never hits the player)
            // - Fly along camera aim
            // - Detonate on first contact
            const ORB_MAX_DISTANCE: f32 = 140.0;
            const ORB_SPAWN_FORWARD: f32 = 0.55;

            let (spawn_pos, dir, max_distance) = cam.map_or_else(
                || {
                    let dir = Vec3::Z;
                    let spawn_pos = origin + Vec3::Y * 1.1 + aim_dir_xz(dir) * ORB_SPAWN_FORWARD;
                    (spawn_pos, dir, ORB_MAX_DISTANCE)
                },
                |c| {
                    aim_projectile_from_camera(
                        spatial_query,
                        origin,
                        c,
                        ORB_MAX_DISTANCE,
                        ORB_SPAWN_FORWARD,
                    )
                },
            );

            spawn_elemental_orb(
                commands,
                assets,
                spawn_pos,
                dir,
                damage,
                radius,
                max_distance,
                Some(element),
            );
        }
        SpellEffect::DamagePool {
            dps,
            radius,
            duration,
            range,
            element,
        } => {
            let (_spawn_pos, p) = cam
                .map_or((origin + Vec3::Y * 1.2, origin + Vec3::Z * range), |c| {
                    aim_from_camera_world(spatial_query, origin, c, range)
                });
            spawn_pool(commands, assets, p, dps, radius, duration, element);
        }
        _ => {}
    }
}

fn aim_from_camera_world(
    spatial_query: &SpatialQuery,
    player_origin: Vec3,
    camera: &GlobalTransform,
    max_range: f32,
) -> (Vec3, Vec3) {
    // Prefer "real" world raycast from camera (terrain, dummies, props).
    // Then clamp the final target around the player so range is consistent.
    let ray_o = camera.translation();
    let ray_dir: Vec3 = *camera.forward();
    let dir3 = Dir3::new(ray_dir).unwrap_or(Dir3::Z);
    let filter = SpatialQueryFilter::default();
    let target = spatial_query
        .cast_ray(ray_o, dir3, max_range, true, &filter)
        .map_or_else(
            || {
                // Fallback: intersect with the player's height plane.
                let target_y = player_origin.y;
                let mut p = if ray_dir.y.abs() > 1e-3 {
                    let t = (target_y - ray_o.y) / ray_dir.y;
                    if t > 0.0 {
                        ray_o + ray_dir * t
                    } else {
                        player_origin + aim_dir_xz(ray_dir) * max_range
                    }
                } else {
                    player_origin + aim_dir_xz(ray_dir) * max_range
                };

                // Clamp to range around the player (not the camera), for consistent gameplay.
                let delta = p - player_origin;
                let delta_xz = Vec3::new(delta.x, 0.0, delta.z);
                let dist = delta_xz.length();
                if dist > max_range {
                    p = player_origin + delta_xz.normalize_or_zero() * max_range;
                }
                p.y = target_y;
                p
            },
            |hit| ray_o + ray_dir.normalize_or_zero() * hit.distance,
        );

    // Spawn the projectile slightly in front of camera so it doesn't instantly collide with nearby geometry.
    let spawn_pos = ray_o + ray_dir.normalize_or_zero() * 0.8;
    (spawn_pos, target)
}

fn aim_dir_xz(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z).normalize_or_zero()
}

#[derive(Component)]
struct ElementalOrb {
    dir: Vec3,
    speed: f32,
    damage: f32,
    radius: f32,
    remaining: f32,
    traveled: f32,
    max_distance: f32,
    element: Option<DamageElement>,
}

#[allow(clippy::too_many_arguments)]
fn spawn_elemental_orb(
    commands: &mut Commands,
    assets: &GameAssets,
    spawn_pos: Vec3,
    dir: Vec3,
    damage: f32,
    radius: f32,
    max_distance: f32,
    element: Option<DamageElement>,
) {
    commands.spawn((
        Name::new("Elemental Orb"),
        Transform::from_translation(spawn_pos),
        ParticleEffect {
            handle: match element.unwrap_or(DamageElement::Holy) {
                DamageElement::Darkness => assets.spell_orb_darkness.clone(),
                DamageElement::Sonic => assets.spell_orb_sonic.clone(),
                DamageElement::Holy => assets.spell_orb_holy.clone(),
                DamageElement::Fire => assets.spell_orb_fire.clone(),
                DamageElement::Frost => assets.spell_orb_frost.clone(),
            },
            prng_seed: None,
        },
        ElementalOrb {
            dir: dir.normalize_or_zero(),
            speed: 18.0,
            damage,
            radius,
            remaining: 8.0,
            traveled: 0.0,
            max_distance,
            element,
        },
    ));
}

fn aim_projectile_from_camera(
    spatial_query: &SpatialQuery,
    player_origin: Vec3,
    camera: &GlobalTransform,
    max_distance: f32,
    spawn_forward: f32,
) -> (Vec3, Vec3, f32) {
    // Spawn just in front of the character, but direction is based on the camera view.
    let ray_dir: Vec3 = *camera.forward();
    let dir = ray_dir.normalize_or_zero();

    // Keep spawn offset on XZ so pitch doesn't spawn into the player capsule.
    let forward_xz = aim_dir_xz(dir);
    let spawn_pos = player_origin + Vec3::Y * 1.1 + forward_xz * spawn_forward;

    // If there's something immediately in front, shrink max travel so it detonates right away.
    let filter = SpatialQueryFilter::default();
    let dir3 = Dir3::new(dir).unwrap_or(Dir3::Z);
    let max_dist = spatial_query
        .cast_ray(spawn_pos, dir3, 1.25, true, &filter)
        .map_or(max_distance, |hit| hit.distance.max(0.2));

    (spawn_pos, dir, max_dist)
}

fn tick_elemental_orbs(
    mut commands: Commands,
    time: Res<Time>,
    spatial_query: SpatialQuery,
    assets: Res<GameAssets>,
    mut orbs: Query<(Entity, &mut Transform, &mut ElementalOrb)>,
    mut damageables: Query<(Entity, &GlobalTransform, &mut Damageable)>,
    mut damage_events: MessageWriter<DamageDealtEvent>,
) {
    let dt = time.delta_secs();
    let filter = SpatialQueryFilter::default();

    for (e, mut tf, mut orb) in orbs.iter_mut() {
        orb.remaining -= dt;
        let pos = tf.translation;
        let dir = orb.dir.normalize_or_zero();
        let step = (orb.speed * dt).max(0.0);

        let mut explode_at: Option<Vec3> = None;

        if orb.remaining <= 0.0 || orb.traveled >= orb.max_distance {
            explode_at = Some(pos);
        } else if dir.length_squared() > 0.0 {
            let dir3 = Dir3::new(dir).unwrap_or(Dir3::Z);
            // Raycast ahead by one step to detect first contact with any collider (terrain, props, dummies).
            if let Some(hit) = spatial_query.cast_ray(pos, dir3, step, true, &filter) {
                explode_at = Some(pos + dir * hit.distance);
            }
        }

        if let Some(p) = explode_at {
            deal_damage_in_radius(
                &mut commands,
                &mut damageables,
                &mut damage_events,
                p,
                orb.radius,
                orb.damage,
                orb.element,
            );
            spawn_blast_vfx(&mut commands, &assets, p, orb.element);
            commands.entity(e).despawn();
        } else {
            tf.translation = pos + dir * step;
            orb.traveled += step;
        }
    }
}

fn deal_damage_in_radius(
    commands: &mut Commands,
    damageables: &mut Query<(Entity, &GlobalTransform, &mut Damageable)>,
    damage_events: &mut MessageWriter<DamageDealtEvent>,
    center: Vec3,
    radius: f32,
    damage: f32,
    element: Option<DamageElement>,
) {
    let r2 = radius * radius;
    for (e, gt, mut d) in damageables.iter_mut() {
        let p = gt.translation();
        if p.distance_squared(center) <= r2 {
            d.hp -= damage;
            damage_events.write(DamageDealtEvent {
                target: e,
                pos: p,
                amount: damage,
                element,
            });
            if d.hp <= 0.0 {
                commands.entity(e).despawn();
            }
        }
    }
}

fn spawn_blast_vfx(
    commands: &mut Commands,
    assets: &GameAssets,
    pos: Vec3,
    element: Option<DamageElement>,
) {
    let element = element.unwrap_or(DamageElement::Holy);
    let handle = match element {
        DamageElement::Darkness => assets.spell_blast_darkness.clone(),
        DamageElement::Sonic => assets.spell_blast_sonic.clone(),
        DamageElement::Holy => assets.spell_blast_holy.clone(),
        DamageElement::Fire => assets.spell_blast_fire.clone(),
        DamageElement::Frost => assets.spell_blast_frost.clone(),
    };
    commands.spawn((
        Name::new(format!("Elemental Blast VFX ({element:?})")),
        Transform::from_translation(pos),
        ParticleEffect {
            handle,
            prng_seed: None,
        },
        DespawnAfter { t: 0.30 },
    ));
}

#[derive(Component)]
struct DespawnAfter {
    t: f32,
}

fn spawn_pool(
    commands: &mut Commands,
    assets: &GameAssets,
    pos: Vec3,
    dps: f32,
    radius: f32,
    duration: f32,
    element: DamageElement,
) {
    let root = commands
        .spawn((
            Name::new("Damage Pool"),
            DamagePoolFx {
                dps,
                radius,
                remaining: duration,
                element,
            },
            // Root stays on XZ; children will be terrain-snapped in Y.
            Transform::from_translation(Vec3::new(pos.x, pos.y, pos.z)),
            GlobalTransform::default(),
        ))
        .id();

    // Multiple emitters approximates "conforming" to uneven terrain without needing true mesh decals.
    // Offsets are in the pool local space.
    let ring_r = radius * 0.35;
    let offsets = [
        Vec2::ZERO,
        Vec2::new(ring_r, 0.0),
        Vec2::new(-ring_r, 0.0),
        Vec2::new(0.0, ring_r),
        Vec2::new(0.0, -ring_r),
        Vec2::new(ring_r * 0.7, ring_r * 0.7),
        Vec2::new(-ring_r * 0.7, -ring_r * 0.7),
    ];

    let emitter_scale = Vec3::new(radius * 0.55, 1.0, radius * 0.55);
    let pool_handle = match element {
        DamageElement::Darkness => assets.spell_pool_darkness.clone(),
        DamageElement::Sonic => assets.spell_pool_sonic.clone(),
        DamageElement::Holy => assets.spell_pool_holy.clone(),
        DamageElement::Fire => assets.spell_pool_fire.clone(),
        DamageElement::Frost => assets.spell_pool_frost.clone(),
    };
    commands.entity(root).with_children(|c| {
        for (i, off) in offsets.into_iter().enumerate() {
            c.spawn((
                Name::new(format!("Damage Pool Emitter {i}")),
                PoolSurfaceSample { offset: off },
                Transform::from_translation(Vec3::new(off.x, 0.05, off.y))
                    .with_scale(emitter_scale),
                ParticleEffect {
                    handle: pool_handle.clone(),
                    prng_seed: None,
                },
            ));
        }
    });
}

#[derive(Component, Clone, Copy)]
struct PoolSurfaceSample {
    offset: Vec2,
}

fn update_damage_pools_surface(
    spatial_query: SpatialQuery,
    pools: Query<(&GlobalTransform, &DamagePoolFx, &Children)>,
    mut samples: Query<(&PoolSurfaceSample, &mut Transform)>,
) {
    // Snap each sample emitter onto the terrain beneath it (approximation, but looks very "ground-hugging").
    let filter = SpatialQueryFilter::default();
    let up = 40.0;
    let max_distance = 120.0;

    for (root_gt, _pool, children) in pools.iter() {
        let root = root_gt.translation();
        for child in children.iter() {
            let Ok((sample, mut tf)) = samples.get_mut(child) else {
                continue;
            };

            let world_x = root.x + sample.offset.x;
            let world_z = root.z + sample.offset.y;
            let origin = Vec3::new(world_x, root.y + up, world_z);
            if let Some(hit) =
                spatial_query.cast_ray(origin, Dir3::NEG_Y, max_distance, true, &filter)
            {
                let ground_y = origin.y - hit.distance;
                tf.translation.y = (ground_y + 0.05) - root.y;
            }
        }
    }
}

fn tick_damage_pools(
    mut commands: Commands,
    time: Res<Time>,
    mut pools: Query<(Entity, &GlobalTransform, &mut DamagePoolFx)>,
    mut damageables: Query<(Entity, &GlobalTransform, &mut Damageable)>,
    mut vfx: Query<(Entity, &mut DespawnAfter)>,
    mut damage_events: MessageWriter<DamageDealtEvent>,
) {
    // Tick pool damage
    let dt = time.delta_secs();
    for (e, gt, mut pool) in pools.iter_mut() {
        pool.remaining -= dt;
        let center = gt.translation();
        let dmg = pool.dps * dt;
        deal_damage_in_radius(
            &mut commands,
            &mut damageables,
            &mut damage_events,
            center,
            pool.radius,
            dmg,
            Some(pool.element),
        );
        if pool.remaining <= 0.0 {
            commands.entity(e).despawn();
        }
    }

    // Tick VFX despawns
    for (e, mut d) in vfx.iter_mut() {
        d.t -= dt;
        if d.t <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

fn apply_spell_effect(
    vitals: &mut Vitals,
    player: &mut Query<(Forces, Option<&ControllerSensors>), With<PlayerRoot>>,
    effect: SpellEffect,
) {
    match effect {
        SpellEffect::Heal(amount) => {
            vitals.health = (vitals.health + amount).min(vitals.max_health);
        }
        SpellEffect::ManaBurst(amount) => {
            vitals.mana = (vitals.mana + amount).min(vitals.max_mana);
        }
        SpellEffect::Dash(impulse) => {
            if let Ok((mut forces, sensors)) = player.single_mut() {
                let mut dir = sensors.map(|s| s.facing_direction).unwrap_or(Vec3::Z);
                dir.y = 0.0;
                let dir = dir.normalize_or_zero();
                if dir.length_squared() > 0.0 {
                    forces.apply_linear_impulse(dir * impulse);
                } else {
                    forces.apply_linear_impulse(Vec3::Z * impulse);
                }
            }
        }
        // Damage spells are handled in `handle_action_bar_casts` because they need world queries
        // (damageables, transforms, meshes/materials). Keeping this match exhaustive for clarity.
        SpellEffect::ElementalBlast { .. } | SpellEffect::DamagePool { .. } => {}
    }
}

fn spawn_cast_fx(commands: &mut Commands, slot: Entity, success: bool) {
    // Simple flash overlay on top of the skill icon.
    let (bg, border) = if success {
        (
            Color::srgba(1.0, 0.95, 0.55, 0.30),
            Color::srgba(1.0, 0.92, 0.35, 0.85),
        )
    } else {
        (
            Color::srgba(1.0, 0.20, 0.20, 0.28),
            Color::srgba(1.0, 0.30, 0.30, 0.90),
        )
    };

    let fx = commands
        .spawn((
            ActionCastFx { t: 0.0, success },
            Name::new("Action Cast FX"),
            GlobalZIndex(40),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(2.0),
                top: Val::Px(2.0),
                right: Val::Px(2.0),
                bottom: Val::Px(2.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(border),
            BorderRadius::all(Val::Px(6.0)),
            Transform::default(),
        ))
        .id();
    commands.entity(slot).add_child(fx);
}

fn animate_action_cast_fx(
    mut commands: Commands,
    time: Res<Time>,
    mut fx: Query<(
        Entity,
        &mut ActionCastFx,
        &mut BackgroundColor,
        &mut BorderColor,
        &mut Transform,
    )>,
) {
    const DUR: f32 = 0.22;
    for (e, mut a, mut bg, mut border, mut tf) in fx.iter_mut() {
        a.t += time.delta_secs();
        let p = (a.t / DUR).clamp(0.0, 1.0);
        let fade = 1.0 - p;

        let (base_bg, base_border) = if a.success {
            (
                Color::srgba(1.0, 0.95, 0.55, 0.30),
                Color::srgba(1.0, 0.92, 0.35, 0.85),
            )
        } else {
            (
                Color::srgba(1.0, 0.20, 0.20, 0.28),
                Color::srgba(1.0, 0.30, 0.30, 0.90),
            )
        };
        bg.0.set_alpha(base_bg.alpha() * fade);
        let mut b = base_border;
        b.set_alpha(base_border.alpha() * fade);
        *border = BorderColor::all(b);

        // subtle pop out
        tf.scale = Vec3::splat(1.0 + p * 0.10);

        if a.t >= DUR {
            commands.entity(e).despawn();
        }
    }
}

// Action bar is currently static; when we add real spells we can drive cooldown wipes here.

fn spawn_orb(
    commands: &mut Commands,
    kind: OrbKind,
    left: Option<f32>,
    right: Option<f32>,
) -> Entity {
    let orb_size = 148.0;
    let pad = 18.0;

    let mut node = Node {
        width: Val::Px(orb_size),
        height: Val::Px(orb_size),
        position_type: PositionType::Absolute,
        bottom: Val::Px(pad),
        ..default()
    };
    if let Some(x) = left {
        node.left = Val::Px(x);
    }
    if let Some(x) = right {
        node.right = Val::Px(x);
    }

    let outer = commands
        .spawn((
            Name::new(match kind {
                OrbKind::Health => "Health Orb",
                OrbKind::Mana => "Mana Orb",
            }),
            node,
        ))
        .id();

    // Clip container for fill image.
    let clip = commands
        .spawn((
            Name::new("Orb Clip"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderRadius::all(Val::Px(999.0)),
        ))
        .id();

    let fill = commands
        .spawn((
            OrbFill(kind),
            Name::new("Orb Fill"),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                height: Val::Percent(60.0),
                ..default()
            },
            ImageNode::default(),
        ))
        .id();

    // Frame and gloss are full-size images on top.
    let frame = commands
        .spawn((
            OrbFrame,
            Name::new("Orb Frame"),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ImageNode::default(),
        ))
        .id();

    let gloss = commands
        .spawn((
            OrbGloss,
            Name::new("Orb Gloss"),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ImageNode::default(),
        ))
        .id();

    // Clean number, no labels.
    let text = commands
        .spawn((
            OrbText(kind),
            Name::new("Orb Value Text"),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            children![
                // Shadow
                (
                    Text::new(""),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.0, 0.0, 0.0, 0.70)),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(2.0),
                        top: Val::Px(2.0),
                        ..default()
                    },
                ),
                // Foreground
                (
                    Text::new(""),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.97, 0.95, 0.90, 0.90)),
                ),
            ],
        ))
        .id();

    commands.entity(outer).add_child(clip);
    commands.entity(clip).add_child(fill);
    commands.entity(outer).add_child(frame);
    commands.entity(outer).add_child(gloss);
    commands.entity(outer).add_child(text);

    outer
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
fn update_hud_from_vitals(
    vitals: Res<Vitals>,
    hud_images: Res<HudImages>,
    texts: Query<(&OrbText, &Children)>,
    mut inner_text: Query<&mut Text>,
    mut images: ParamSet<(
        Query<(&OrbFill, &mut Node, &mut ImageNode)>,
        Query<&mut ImageNode, With<OrbFrame>>,
        Query<&mut ImageNode, With<OrbGloss>>,
        Query<&mut ImageNode, With<HudBar>>,
    )>,
) {
    let hp_frac = (vitals.health / vitals.max_health).clamp(0.0, 1.0);
    let mp_frac = (vitals.mana / vitals.max_mana).clamp(0.0, 1.0);

    // Apply textures once.
    for mut img in images.p1().iter_mut() {
        if img.image == Handle::<Image>::default() {
            img.image = hud_images.frame.clone();
        }
    }
    for mut img in images.p2().iter_mut() {
        if img.image == Handle::<Image>::default() {
            img.image = hud_images.gloss.clone();
        }
    }
    for mut img in images.p3().iter_mut() {
        if img.image == Handle::<Image>::default() {
            img.image = hud_images.bar.clone();
        }
    }

    for (fill, mut node, mut img) in images.p0().iter_mut() {
        let (frac, tex) = match fill.0 {
            OrbKind::Health => (hp_frac, hud_images.hp_fill.clone()),
            OrbKind::Mana => (mp_frac, hud_images.mp_fill.clone()),
        };
        node.height = Val::Percent(frac * 100.0);
        img.image = tex;
    }

    for (label, children) in texts.iter() {
        let value = match label.0 {
            OrbKind::Health => vitals.health,
            OrbKind::Mana => vitals.mana,
        };
        // Show what the player can actually spend (we gate costs on floor(mana)).
        let s = format!("{:.0}", value.max(0.0).floor());
        for child in children.iter() {
            if let Ok(mut t) = inner_text.get_mut(child) {
                *t = Text::new(s.clone());
            }
        }
    }
}

fn make_hud_bar_image(width: u32, height: u32) -> Image {
    let mut data = vec![0u8; (width * height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;

            // Clean WoW-ish bar: darker center with a warm bronze tint and soft top highlight.
            let u = x as f32 / (width - 1) as f32;
            let v = y as f32 / (height - 1) as f32;
            let vignette = (1.0 - (u - 0.5).abs() * 1.3).clamp(0.0, 1.0)
                * (1.0 - (v - 0.55).abs() * 1.6).clamp(0.0, 1.0);

            let hash = x
                .wrapping_mul(1664525)
                .wrapping_add(y.wrapping_mul(1013904223));
            let n = ((hash >> 24) & 0xff) as f32 / 255.0;
            let noise = (n - 0.5) * 0.04;

            let top_highlight = ((0.22 - v).max(0.0) / 0.22).clamp(0.0, 1.0) * 0.08;
            let base = (0.12 + vignette * 0.12 + top_highlight + noise).clamp(0.0, 1.0);
            let warm = (0.05 + vignette * 0.05).clamp(0.0, 1.0);

            data[idx] = ((base + warm) * 255.0) as u8;
            data[idx + 1] = ((base * 0.78 + warm * 0.65) * 255.0) as u8;
            data[idx + 2] = ((base * 0.52) * 255.0) as u8;

            // Fade out the bar vertically at the top for a clean blend into the world.
            let alpha = (v * 1.2).clamp(0.0, 1.0);
            data[idx + 3] = (alpha * 255.0) as u8;
        }
    }

    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    )
}

fn make_slot_frame_image(size: u32) -> Image {
    // Cleaner WoW-ish slot frame: warm bronze bevel + crisp inner edge.
    let mut data = vec![0u8; (size * size * 4) as usize];
    let c = size as f32 * 0.5;
    let inner = c - 10.0;
    let outer = c - 2.0;

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let dx = (fx - c).abs();
            let dy = (fy - c).abs();
            let d = dx.max(dy);
            let idx = ((y * size + x) * 4) as usize;

            if d > outer {
                continue;
            }

            let (col, a) = if d >= inner {
                let t = ((d - inner) / (outer - inner)).clamp(0.0, 1.0);
                let sheen = (((x as f32 / size as f32) * 6.0).sin() * 0.06 + 0.12).clamp(0.0, 0.20);
                let bevel = if t < 0.28 {
                    (0.28 - t) * 0.7
                } else if t > 0.86 {
                    -(t - 0.86) * 0.7
                } else {
                    0.0
                };
                let base = (0.10 + (1.0 - t) * 0.12 + sheen + bevel).clamp(0.0, 1.0);
                // push towards bronze/gold
                (Vec3::new(base * 1.10, base * 0.86, base * 0.48), 1.0)
            } else {
                let t = (d / inner).clamp(0.0, 1.0);
                let shadow = ((t - 0.80).max(0.0) / 0.20).clamp(0.0, 1.0);
                (Vec3::new(0.02, 0.02, 0.02), 0.45 * shadow)
            };

            data[idx] = (col.x * 255.0) as u8;
            data[idx + 1] = (col.y * 255.0) as u8;
            data[idx + 2] = (col.z * 255.0) as u8;
            data[idx + 3] = (a * 255.0) as u8;
        }
    }

    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    )
}

fn make_slot_gloss_image(size: u32) -> Image {
    // A simple top-left gloss.
    let mut data = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let u = x as f32 / (size - 1) as f32;
            let v = y as f32 / (size - 1) as f32;
            let h = (1.0 - ((u - 0.20) * (u - 0.20) + (v - 0.18) * (v - 0.18)).sqrt() * 2.4)
                .clamp(0.0, 1.0);
            let alpha = (h * 0.20).min(0.20);
            if alpha <= 0.0 {
                continue;
            }
            data[idx] = 255;
            data[idx + 1] = 255;
            data[idx + 2] = 255;
            data[idx + 3] = (alpha * 255.0) as u8;
        }
    }

    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    )
}

fn make_spell_icon_image(size: u32, seed: u32) -> Image {
    // Original “rune” icon: radial gradient + noisy bands, deterministic per seed.
    let mut data = vec![0u8; (size * size * 4) as usize];
    let c = size as f32 * 0.5;
    let r = c - 2.0;

    let palettes = [
        (Vec3::new(0.80, 0.25, 0.12), Vec3::new(0.18, 0.04, 0.02)),
        (Vec3::new(0.12, 0.52, 0.86), Vec3::new(0.02, 0.05, 0.10)),
        (Vec3::new(0.62, 0.54, 0.22), Vec3::new(0.08, 0.06, 0.02)),
        (Vec3::new(0.46, 0.20, 0.70), Vec3::new(0.05, 0.02, 0.08)),
    ];
    let (hi, lo) = palettes[(seed as usize) % palettes.len()];

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let dx = fx - c;
            let dy = fy - c;
            let d = (dx * dx + dy * dy).sqrt();
            let idx = ((y * size + x) * 4) as usize;
            if d > r {
                continue;
            }

            let ang = (dy / r).atan2(dx / r);
            let rad = (1.0 - (d / r)).clamp(0.0, 1.0);

            let h = seed
                .wrapping_mul(1664525)
                .wrapping_add((x ^ y).wrapping_mul(1013904223));
            let n = ((h >> 24) & 0xff) as f32 / 255.0;
            let band = ((ang * 3.0 + n * 1.8).sin() * 0.5 + 0.5) * 0.35;

            let glow = (rad * 0.7 + band).clamp(0.0, 1.0);
            let col = lo.lerp(hi, glow);

            data[idx] = (col.x * 255.0) as u8;
            data[idx + 1] = (col.y * 255.0) as u8;
            data[idx + 2] = (col.z * 255.0) as u8;
            data[idx + 3] = 255;
        }
    }

    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    )
}

fn make_orb_frame_image(size: u32) -> Image {
    let mut data = vec![0u8; (size * size * 4) as usize];
    let c = size as f32 * 0.5;
    let r_outer = c - 2.0;
    let r_inner = c - 18.0;

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let dx = fx - c;
            let dy = fy - c;
            let d = (dx * dx + dy * dy).sqrt();
            let idx = ((y * size + x) * 4) as usize;

            let in_ring = d <= r_outer && d >= r_inner;
            if !in_ring {
                continue;
            }

            // WoW-ish gold rim: warmer, cleaner, slightly brighter.
            let t = ((d - r_inner) / (r_outer - r_inner)).clamp(0.0, 1.0);
            let ang = dy.atan2(dx);
            let sheen = ((ang * 2.0).sin() * 0.18 + 0.16).clamp(0.0, 0.32);
            let bevel = if t < 0.22 {
                (0.22 - t) * 0.9
            } else if t > 0.82 {
                -(t - 0.82) * 0.8
            } else {
                0.0
            };
            let base = (0.10 + (1.0 - t) * 0.12 + sheen + bevel).clamp(0.0, 1.0);
            let r = ((base * 1.10) * 255.0) as u8;
            let g = ((base * 0.88) * 255.0) as u8;
            let b = ((base * 0.46) * 255.0) as u8;

            data[idx] = r;
            data[idx + 1] = g;
            data[idx + 2] = b;
            data[idx + 3] = 255;
        }
    }

    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    )
}

fn make_orb_gloss_image(size: u32) -> Image {
    let mut data = vec![0u8; (size * size * 4) as usize];
    let c = size as f32 * 0.5;
    let r = c - 20.0;

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let dx = fx - c;
            let dy = fy - c;
            let d = (dx * dx + dy * dy).sqrt();
            let idx = ((y * size + x) * 4) as usize;
            if d > r {
                continue;
            }

            // Gloss highlight biased to top-left.
            let hx = (fx / size as f32 - 0.20).clamp(0.0, 1.0);
            let hy = (fy / size as f32 - 0.10).clamp(0.0, 1.0);
            let h = (1.0 - (hx * hx + hy * hy).sqrt()).clamp(0.0, 1.0);
            // WoW-ish: a bit cleaner / less glossy than Diablo
            let alpha = (h * 0.10).min(0.10);

            data[idx] = 255;
            data[idx + 1] = 255;
            data[idx + 2] = 255;
            data[idx + 3] = (alpha * 255.0) as u8;
        }
    }

    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    )
}

fn make_orb_fill_image(size: u32, base: Color) -> Image {
    let mut data = vec![0u8; (size * size * 4) as usize];
    let c = size as f32 * 0.5;
    let r = c - 20.0;

    let [br, bg, bb, _] = base.to_srgba().to_f32_array();

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let dx = fx - c;
            let dy = fy - c;
            let d = (dx * dx + dy * dy).sqrt();
            let idx = ((y * size + x) * 4) as usize;
            if d > r {
                continue;
            }

            let edge = (1.0 - (d / r)).clamp(0.0, 1.0);
            let v = (fy / size as f32).clamp(0.0, 1.0);
            // Use wrapping math to avoid debug overflow panics.
            let hash = x
                .wrapping_mul(1103515245)
                .wrapping_add(y.wrapping_mul(12345));
            let swirl = (((hash & 0xff) as f32) / 255.0) * 0.06 - 0.03;
            // Cleaner orb fill: less noisy, slightly more glassy near the top.
            let depth = (0.40 + edge * 0.52 + (1.0 - v) * 0.10 + swirl * 0.7).clamp(0.0, 1.0);
            let top_glow = ((1.0 - (v * 1.3)).clamp(0.0, 1.0) * 0.14).clamp(0.0, 0.14);
            let bright = (depth + top_glow).clamp(0.0, 1.0);

            data[idx] = (br * bright * 255.0) as u8;
            data[idx + 1] = (bg * bright * 255.0) as u8;
            data[idx + 2] = (bb * bright * 255.0) as u8;
            data[idx + 3] = 255;
        }
    }

    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    )
}
